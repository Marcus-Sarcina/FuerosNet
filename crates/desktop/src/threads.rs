//! **An identity's conversations at rest** (`light-client-requirements.md`
//! §9, APP-015), as the phone keeps them (its `Threads.kt`): the shell's
//! own files, sealed through the kernel under a key the shell never holds.
//! One file per thread, each message appended as it accumulates rather
//! than the history resealed whole; an index naming the threads for the
//! list, rewritten when a thread is added or renamed and never on a
//! message; and a thread read **lazily**, backwards, a page at a time, so
//! nothing of a thread not opened is decrypted at start.  Nothing here has
//! a retention window (`network-design.md` §7.5.1).
//!
//! A record is `[len][sealed][len]`, big-endian, the trailing length so
//! the file is walked backwards; the label a record is sealed under names
//! the identity, the thread and the record's own offset, so a record cannot
//! be moved from one place to another.  A record's offset is the message's
//! id: unique in its thread, monotonic, and what a delivery change names.
//! **The records and labels are the phone's to the character**, so one
//! format serves both shells.

use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// The directory the threads live in, under the desktop's own.
pub const DIR: &str = "dm";
/// The index's name, within the desktop's directory.
pub const INDEX: &str = "dm/index";
/// **What this identity chose about the people it talks to, and each
/// thread's unread count** [author, 2026-11-23]: one sealed file beside the
/// index, so the index is still written only when a thread is added or
/// renamed.
pub const PEOPLE: &str = "dm/people";
/// How many messages a page holds.
pub const PAGE: usize = 50;

/// **The kernel's sealing service, as the shell reaches it**: the label
/// bound as associated data (`Participant::seal`).
pub trait Sealer: Send + Sync {
    /// `bytes` sealed under `label`.
    fn seal(&self, label: &str, bytes: &[u8]) -> Vec<u8>;
    /// What [`Sealer::seal`] sealed under `label`, or nothing.
    fn unseal(&self, label: &str, bytes: &[u8]) -> Option<Vec<u8>>;
}

/// Where a message stands with the kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// Handed to the kernel, which has not yet said.
    Sending,
    /// The kernel accepted it for carriage.
    Sent,
    /// The kernel refused it, and the reason says why.
    Unsent,
}

impl Delivery {
    fn name(self) -> &'static str {
        match self {
            Delivery::Sending => "SENDING",
            Delivery::Sent => "SENT",
            Delivery::Unsent => "UNSENT",
        }
    }

    fn of(name: &str) -> Option<Delivery> {
        match name {
            "SENDING" => Some(Delivery::Sending),
            "SENT" => Some(Delivery::Sent),
            "UNSENT" => Some(Delivery::Unsent),
            _ => None,
        }
    }
}

/// One record of a thread file: a message, or a delivery change to one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    /// Something said, by this identity or to it.
    Said {
        /// Whether this identity said it.
        mine: bool,
        /// Who, as the thread names them.
        who: String,
        /// What.
        text: String,
        /// Where it stands.
        delivery: Delivery,
        /// Why it was refused, where it was.
        reason: Option<String>,
    },
    /// A change to the message at offset `id`.
    Settled {
        /// The message's offset.
        id: u64,
        /// Where it stands now.
        delivery: Delivery,
        /// Why it was refused, where it was.
        reason: Option<String>,
    },
}

impl Record {
    /// The record as the phone writes it.
    pub fn encode(&self) -> String {
        match self {
            Record::Said {
                mine,
                who,
                text,
                delivery,
                reason,
            } => json!({"mine": mine, "who": who, "text": text,
                        "delivery": delivery.name(), "reason": reason})
            .to_string(),
            Record::Settled {
                id,
                delivery,
                reason,
            } => json!({"settle": id, "delivery": delivery.name(), "reason": reason}).to_string(),
        }
    }

    /// A record back, or none where it is not one.
    pub fn decode(text: &str) -> Option<Record> {
        let v: Value = serde_json::from_str(text).ok()?;
        let delivery = Delivery::of(v.get("delivery")?.as_str()?)?;
        let reason = v.get("reason").and_then(Value::as_str).map(String::from);
        if let Some(id) = v.get("settle").and_then(Value::as_u64) {
            return Some(Record::Settled {
                id,
                delivery,
                reason,
            });
        }
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Some(Record::Said {
            mine: v.get("mine").and_then(Value::as_bool).unwrap_or(false),
            who: s("who"),
            text: s("text"),
            delivery,
            reason,
        })
    }
}

/// A message as a thread shows it, with its delivery changes folded in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Its offset in the thread file.
    pub id: u64,
    /// Whether this identity said it.
    pub mine: bool,
    /// Who.
    pub who: String,
    /// What.
    pub text: String,
    /// Where it stands.
    pub delivery: Delivery,
    /// Why it was refused, where it was.
    pub reason: Option<String>,
}

/// A thread as the index lists it: enough for the list, nothing of what
/// was said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The other party's keyhash, in hex.
    pub peer: String,
    /// What the thread calls them.
    pub name: String,
    /// Whether it is a direct thread rather than a board's.
    pub direct: bool,
    /// **The identity it is held as**, in hex: read from the store it came
    /// out of and never written into it, since a conversation continues in
    /// the identity it started in (`light-client-requirements.md` §2).
    pub by: String,
}

/// **A page read backwards**: the messages in file order, where the page
/// before it ends (none: the file's start was reached), delivery changes
/// met whose message lies earlier still, and whether a record refused to
/// open — the walk stops there and what opened is returned.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Page {
    /// The messages, oldest first.
    pub messages: Vec<Message>,
    /// The offset to read the page before this one from.
    pub before: Option<u64>,
    /// Delivery changes for messages earlier than this page.
    pub pending: HashMap<u64, (Delivery, Option<String>)>,
    /// Whether a record did not open.
    pub broken: bool,
}

/// **What this person has chosen about one party** [author, 2026-11-23],
/// and the thread's unread count with it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Chosen {
    /// **The colour they are shown in**, by the palette's name; none for
    /// the one their key gives them.
    pub colour: Option<String>,
    /// **Muted**: arrivals are kept and counted on the thread's card, and
    /// do not raise the direct-message orb.
    pub muted: bool,
    /// **Notifications muted**: arrivals count as any do, and are not told.
    pub silent: bool,
    /// **Blocked**: arrivals are dropped as they come, never kept or shown.
    pub blocked: bool,
    /// What arrived since the thread was last on screen.
    pub unread: u32,
    /// **Its last arrival could not be attributed** [author, 2026-11-19]:
    /// the thread is shown under the alias it carried, marked unverified,
    /// and not answered from here [author, 2026-11-20], until a message
    /// from them is attributed. Not a choice; kept with the count.
    pub unverified: bool,
}

/// One identity's threads, under the desktop's directory.
pub struct Threads {
    dir: PathBuf,
    by: String,
    sealer: Box<dyn Sealer>,
    /// What is chosen is read, changed and written whole, from the window
    /// and from the kernel's arrivals both.
    people: std::sync::Mutex<()>,
}

impl Threads {
    /// The threads of the identity `by` (its keyhash in hex), kept under
    /// `dir`.
    pub fn new(dir: &Path, by: &str, sealer: Box<dyn Sealer>) -> Threads {
        Threads {
            dir: dir.to_path_buf(),
            by: by.to_string(),
            sealer,
            people: std::sync::Mutex::new(()),
        }
    }

    /// **What this identity chose about each party**, by their keyhash in
    /// hex; none where there is none or it does not open.
    pub fn people(&self) -> std::collections::BTreeMap<String, Chosen> {
        let Ok(bytes) = std::fs::read(self.path(PEOPLE)) else {
            return Default::default();
        };
        let Some(open) = self.sealer.unseal(&format!("{}/{PEOPLE}", self.by), &bytes) else {
            return Default::default();
        };
        let Ok(Value::Object(all)) = serde_json::from_slice::<Value>(&open) else {
            return Default::default();
        };
        all.into_iter()
            .map(|(peer, o)| {
                let flag = |k: &str| o.get(k).and_then(Value::as_bool).unwrap_or(false);
                let chosen = Chosen {
                    colour: o.get("colour").and_then(Value::as_str).map(String::from),
                    muted: flag("muted"),
                    silent: flag("silent"),
                    blocked: flag("blocked"),
                    unverified: flag("unverified"),
                    unread: o
                        .get("unread")
                        .and_then(Value::as_u64)
                        .and_then(|n| u32::try_from(n).ok())
                        .unwrap_or(0),
                };
                (peer, chosen)
            })
            .collect()
    }

    /// **Change what is chosen about `peer`**, and keep it; what it is now.
    pub fn choose(&self, peer: &str, change: impl FnOnce(&mut Chosen)) -> Result<Chosen, String> {
        let _held = self
            .people
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut all = self.people();
        let mut c = all.get(peer).cloned().unwrap_or_default();
        let was = c.clone();
        change(&mut c);
        if c == was {
            return Ok(c);
        }
        if c == Chosen::default() {
            all.remove(peer);
        } else {
            all.insert(peer.to_string(), c.clone());
        }
        let body: serde_json::Map<String, Value> = all
            .into_iter()
            .map(|(peer, c)| {
                let v = json!({"colour": c.colour, "muted": c.muted, "silent": c.silent,
                    "blocked": c.blocked, "unread": c.unread, "unverified": c.unverified});
                (peer, v)
            })
            .collect();
        let sealed = self.sealer.seal(
            &format!("{}/{PEOPLE}", self.by),
            Value::Object(body).to_string().as_bytes(),
        );
        crate::platform::create_private(&self.path(DIR))?;
        crate::platform::write_whole(&self.path(PEOPLE), &sealed).map_err(|e| e.to_string())?;
        Ok(c)
    }

    fn file_of(peer: &str) -> String {
        format!("{DIR}/{peer}")
    }

    fn label(&self, peer: &str, at: u64) -> String {
        format!("{}/{}/@{at}", self.by, Threads::file_of(peer))
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// The threads the index names; none where there is no index or it
    /// does not open.
    pub fn index(&self) -> Vec<Entry> {
        let Ok(bytes) = std::fs::read(self.path(INDEX)) else {
            return Vec::new();
        };
        let Some(open) = self.sealer.unseal(&format!("{}/{INDEX}", self.by), &bytes) else {
            return Vec::new();
        };
        let Ok(Value::Array(items)) = serde_json::from_slice::<Value>(&open) else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|e| {
                let peer = e.get("peer")?.as_str()?.to_string();
                Some(Entry {
                    name: e
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or(&peer)
                        .to_string(),
                    direct: e.get("direct").and_then(Value::as_bool).unwrap_or(false),
                    peer,
                    by: self.by.clone(),
                })
            })
            .collect()
    }

    /// The index rewritten whole.
    pub fn write_index(&self, entries: &[Entry]) -> Result<(), String> {
        let list: Vec<Value> = entries
            .iter()
            .map(|e| json!({"peer": e.peer, "name": e.name, "direct": e.direct}))
            .collect();
        let sealed = self.sealer.seal(
            &format!("{}/{INDEX}", self.by),
            Value::Array(list).to_string().as_bytes(),
        );
        crate::platform::create_private(&self.path(DIR))?;
        crate::platform::write_whole(&self.path(INDEX), &sealed).map_err(|e| e.to_string())
    }

    /// **Name a thread in the index**, adding it or renaming it; the
    /// index is untouched where it already says so.
    pub fn list(&self, peer: &str, name: &str) -> Result<(), String> {
        let mut all = self.index();
        match all.iter_mut().find(|e| e.peer == peer) {
            Some(e) if e.name == name => return Ok(()),
            Some(e) => e.name = name.to_string(),
            None => all.push(Entry {
                peer: peer.to_string(),
                name: name.to_string(),
                direct: true,
                by: self.by.clone(),
            }),
        }
        self.write_index(&all)
    }

    /// **Append one record to the thread with `peer`**: its id (its
    /// offset), or why the file did not take it.
    pub fn append(&self, peer: &str, record: &Record) -> Result<u64, String> {
        crate::platform::create_private(&self.path(DIR))?;
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(self.path(&Threads::file_of(peer)))
            .map_err(|e| e.to_string())?;
        let at = f.metadata().map_err(|e| e.to_string())?.len();
        let sealed = self
            .sealer
            .seal(&self.label(peer, at), record.encode().as_bytes());
        let len = u32::try_from(sealed.len()).map_err(|_| "a record too large")?;
        let mut framed = Vec::with_capacity(sealed.len() + 8);
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(&sealed);
        framed.extend_from_slice(&len.to_be_bytes());
        f.write_all(&framed).map_err(|e| e.to_string())?;
        f.sync_data().map_err(|e| e.to_string())?;
        Ok(at)
    }

    /// **The newest `n` messages before `before`** (the file's end where
    /// none), delivery changes folded in — `pending` carries the ones an
    /// earlier page met for messages that lie here.
    pub fn page(
        &self,
        peer: &str,
        before: Option<u64>,
        n: usize,
        pending: HashMap<u64, (Delivery, Option<String>)>,
    ) -> Page {
        let mut out = Page {
            pending,
            ..Page::default()
        };
        let Ok(mut f) = std::fs::File::open(self.path(&Threads::file_of(peer))) else {
            return out;
        };
        let Ok(size) = f.metadata().map(|m| m.len()) else {
            return out;
        };
        let mut at = before.unwrap_or(size).min(size);
        let mut read = |from: u64, len: usize| -> Option<Vec<u8>> {
            let mut b = vec![0u8; len];
            f.seek(SeekFrom::Start(from)).ok()?;
            f.read_exact(&mut b).ok()?;
            Some(b)
        };
        while out.messages.len() < n && at >= 8 {
            let Some(len) = read(at - 4, 4).map(|t| u64::from(u32::from_be_bytes(len4(&t)))) else {
                out.broken = true;
                break;
            };
            let Some(start) = at.checked_sub(len + 8) else {
                out.broken = true;
                break;
            };
            let head = read(start, 4).map(|h| u64::from(u32::from_be_bytes(len4(&h))));
            let sealed = usize::try_from(len).ok().and_then(|l| read(start + 4, l));
            let (Some(head), Some(sealed)) = (head, sealed) else {
                out.broken = true;
                break;
            };
            let record = (head == len)
                .then(|| self.sealer.unseal(&self.label(peer, start), &sealed))
                .flatten()
                .and_then(|b| String::from_utf8(b).ok())
                .and_then(|t| Record::decode(&t));
            match record {
                None => {
                    out.broken = true;
                    break;
                }
                Some(Record::Settled {
                    id,
                    delivery,
                    reason,
                }) => {
                    out.pending.insert(id, (delivery, reason));
                }
                Some(Record::Said {
                    mine,
                    who,
                    text,
                    delivery,
                    reason,
                }) => {
                    let (delivery, reason) =
                        out.pending.remove(&start).unwrap_or((delivery, reason));
                    out.messages.push(Message {
                        id: start,
                        mine,
                        who,
                        text,
                        delivery,
                        reason,
                    });
                }
            }
            at = start;
        }
        out.messages.reverse();
        out.before = (at > 0 && !out.broken).then_some(at);
        out
    }
}

fn len4(b: &[u8]) -> [u8; 4] {
    [b[0], b[1], b[2], b[3]]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sealer that binds the label as the kernel's does, so a record
    /// moved to another offset does not open: the label and a length in
    /// front, checked on the way back.
    struct Labelled;

    impl Sealer for Labelled {
        fn seal(&self, label: &str, bytes: &[u8]) -> Vec<u8> {
            let mut out = (label.len() as u32).to_be_bytes().to_vec();
            out.extend_from_slice(label.as_bytes());
            out.extend_from_slice(bytes);
            out
        }
        fn unseal(&self, label: &str, bytes: &[u8]) -> Option<Vec<u8>> {
            let n = u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
            (bytes.get(4..4 + n)? == label.as_bytes()).then(|| bytes[4 + n..].to_vec())
        }
    }

    fn fresh(name: &str) -> (PathBuf, Threads) {
        let d = std::env::temp_dir().join(format!("rhtn-threads-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        crate::platform::create_private(&d).unwrap();
        let t = Threads::new(&d, "me", Box::new(Labelled));
        (d, t)
    }

    fn said(mine: bool, text: &str) -> Record {
        Record::Said {
            mine,
            who: if mine { "me" } else { "them" }.into(),
            text: text.into(),
            delivery: if mine {
                Delivery::Sending
            } else {
                Delivery::Sent
            },
            reason: None,
        }
    }

    #[test]
    fn a_record_reads_back_as_the_phone_writes_it() {
        let r = said(true, "hé \"there\"");
        assert_eq!(Record::decode(&r.encode()), Some(r));
        // the phone's own line, byte for byte as Threads.encode writes it
        let phone = r#"{"settle":12,"delivery":"UNSENT","reason":"no session"}"#;
        assert_eq!(
            Record::decode(phone),
            Some(Record::Settled {
                id: 12,
                delivery: Delivery::Unsent,
                reason: Some("no session".into())
            })
        );
        assert_eq!(Record::decode(r#"{"delivery":"LOST"}"#), None);
    }

    #[test]
    fn a_thread_appends_and_reads_back_a_page_at_a_time_with_its_settlements() {
        let (d, t) = fresh("pages");
        let mut ids = Vec::new();
        for i in 0..5 {
            ids.push(t.append("ab", &said(i % 2 == 0, &format!("m{i}"))).unwrap());
        }
        t.append(
            "ab",
            &Record::Settled {
                id: ids[0],
                delivery: Delivery::Sent,
                reason: None,
            },
        )
        .unwrap();
        let newest = t.page("ab", None, 3, HashMap::new());
        let texts: Vec<&str> = newest.messages.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(texts, ["m2", "m3", "m4"]);
        assert_eq!(newest.before, Some(ids[2]));
        assert!(
            newest.pending.contains_key(&ids[0]),
            "carried to the page it belongs to"
        );
        let older = t.page("ab", newest.before, 3, newest.pending);
        assert_eq!(older.messages.len(), 2);
        assert_eq!(
            older.messages[0].delivery,
            Delivery::Sent,
            "the settlement folded in"
        );
        assert_eq!(
            older.messages[1].delivery,
            Delivery::Sent,
            "theirs, as said"
        );
        assert_eq!(older.before, None);
        assert!(!older.broken);
        assert_eq!(t.page("nobody", None, 3, HashMap::new()), Page::default());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_record_moved_or_changed_stops_the_walk_where_it_lies() {
        let (d, t) = fresh("moved");
        t.append("ab", &said(false, "first")).unwrap();
        t.append("ab", &said(false, "second")).unwrap();
        // the other thread's file under this one's name: every label differs
        let bytes = std::fs::read(d.join("dm/ab")).unwrap();
        std::fs::write(d.join("dm/cd"), &bytes).unwrap();
        let p = t.page("cd", None, 10, HashMap::new());
        assert!(p.broken && p.messages.is_empty());
        let mut cut = bytes.clone();
        cut.truncate(bytes.len() - 3);
        std::fs::write(d.join("dm/ab"), &cut).unwrap();
        assert!(t.page("ab", None, 10, HashMap::new()).broken);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn the_index_names_each_thread_once_and_renames_in_place() {
        let (d, t) = fresh("index");
        assert!(t.index().is_empty());
        t.list("ab", "ab").unwrap();
        t.list("cd", "Carol").unwrap();
        t.list("ab", "Alice").unwrap();
        let names: Vec<(String, String)> =
            t.index().into_iter().map(|e| (e.peer, e.name)).collect();
        assert_eq!(
            names,
            [("ab".into(), "Alice".into()), ("cd".into(), "Carol".into())]
        );
        // another identity's sealer label does not open this index
        let other = Threads::new(&d, "else", Box::new(Labelled));
        assert!(other.index().is_empty());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn what_is_chosen_about_a_party_is_kept_sealed_and_read_back() {
        let (d, t) = fresh("people");
        let peer = "ab".repeat(32);
        t.choose(&peer, |c| {
            c.muted = true;
            c.unread = 2;
            c.colour = Some("Teal".into());
            c.unverified = true;
        })
        .unwrap();
        let bytes = std::fs::read(d.join(PEOPLE)).unwrap();
        assert!(Labelled.unseal("me/dm/people", &bytes).is_some());
        assert!(Labelled.unseal("them/dm/people", &bytes).is_none());
        let back = Threads::new(&d, "me", Box::new(Labelled)).people();
        assert_eq!(
            back.get(&peer),
            Some(&Chosen {
                colour: Some("Teal".into()),
                muted: true,
                silent: false,
                blocked: false,
                unread: 2,
                unverified: true,
            })
        );
        // back to nothing chosen and nothing unread, the party is not kept
        t.choose(&peer, |c| *c = Chosen::default()).unwrap();
        assert!(t.people().is_empty());
        let _ = std::fs::remove_dir_all(d);
    }
}
