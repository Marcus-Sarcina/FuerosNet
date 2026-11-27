//! **The operator's desktop** (`light-client-requirements.md` §10): the
//! provider's own console in a window, the steps beside it, the token
//! pasted back, and what it is worth said at once.
//!
//! **Why a desktop client at all.** §8.3 has the provisioning pages ship
//! with the client, and an operator standing up their first node has no
//! node to serve them one: the pages must work before any node exists.
//! The phone does this already; a desktop is where an operator with a
//! keyboard would rather do it, and it is the only client that can put a
//! provider's sign-up next to the instructions for it.
//!
//! **Everything here is what the window is not.** The window and its web
//! view are the system's and are `main.rs`'s business, behind the `gui`
//! feature; this module is the page it shows, the acts that page can ask
//! for, and what each act does — all of it testable with no desktop
//! libraries present, which is what lets the gate check it.
//!
//! **No credential of the project's, and none scraped.** The provider's
//! console is a page of theirs in a window of ours: another origin, which
//! this client cannot read and does not try to (a token read out of
//! someone else's page would be a credential taken rather than given).
//! The operator makes a token there as the provider's own steps say, and
//! pastes it. What this client then does is try it once and keep it where
//! the command line's `--credential-file` expects it.

// Every public item carries its own documentation.
#![warn(missing_docs)]

// Two flavours of one build, never both (see `rhtn-client`'s manifest).
#[cfg(all(feature = "releasable", feature = "fieldtest"))]
compile_error!(
    "rhtn-desktop: `releasable` and `fieldtest` are two flavours of one build; \
     build the field-test flavour with `--no-default-features --features fieldtest`"
);

/// What the authority screen says: the lines held, and what ending one costs.
pub mod authority;
/// The code this desktop shows, drawn as the phone reads one.
pub mod code;
/// Custody of the storage key and the transport seed: the desktop keyring.
#[cfg(feature = "gui")]
pub mod custody;
/// A resource's page, framed in a window of its own.
pub mod frame;
/// The instances this identity runs, administered from a restored desktop.
pub mod operate;
/// Paired from the phone, over the code this desktop shows.
pub mod pairing;
/// The participant client's pages and what it keeps between runs.
pub mod participant;
/// A desktop restored from the phrase: the words back as the seeds.
pub mod phrase;
/// The kernel's platform, where the platform is a desktop.
pub mod platform;
/// The participant's screens once the desktop is up.
pub mod screens;
/// An identity's conversations at rest, sealed through the kernel.
pub mod threads;
/// A resource's web view, as the engine is told to build it.
#[cfg(feature = "gui")]
pub mod window;

use rhtn_launch::Transport;
use std::path::{Path, PathBuf};

/// What the page can ask the shell to do.  A closed set, because the page
/// is this crate's own and a message that is not one of these is a
/// message nobody wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Open this provider's console in a window of its own: the sign-up,
    /// the payment method, the two-factor prompt and the token page, at
    /// whatever pace they take.
    Console {
        /// The provider, as a credential is filed under.
        provider: String,
    },
    /// Try a pasted token at its provider and keep it where it works.
    Token {
        /// The provider.
        provider: String,
        /// What was pasted.
        token: String,
    },
    /// The person's answer to the question the kernel put.
    Answer {
        /// Yes or no.
        yes: bool,
    },
    /// Show the page that launches a node.
    Launch,
    /// **Pair this desktop again at its next start**, on the directory it
    /// holds, so that what it keeps is kept.
    PairAgain,
    /// **Show a page of earlier messages** in the thread with `peer`, as
    /// the phone's "earlier messages" does.
    Earlier {
        /// The party, in hex.
        peer: String,
    },
    /// **Administer the instance this identity runs** [author, 2026-11-26]:
    /// on a restored desktop, which derives the machine's keys.
    Administer {
        /// The identity that runs it.
        by: Vec<u8>,
    },
    /// Restore this desktop from the identity's phrase.
    Restore {
        /// What was typed.
        phrase: String,
    },
    /// Connect to a node already running: agree to it and attach.
    Attach {
        /// The node's keyhash.
        node: Vec<u8>,
        /// Where it answers, `host:port`.
        address: String,
    },
    /// **Back**, as the phone's `‹` and its platform's back are: the place
    /// before, else the one this is reached from.
    Back,
    /// Go to a place.
    Go {
        /// Where.
        place: screens::Place,
    },
    /// Open a backup of this identity and install its archive.
    Backup {
        /// The file, as read.
        bytes: Vec<u8>,
    },
    /// **Open a resource's page in a window of its own**
    /// (`light-client-requirements.md` §8), through the node that served
    /// its catalog entry.
    Open {
        /// The identity that holds the subnet it is offered in.
        by: Vec<u8>,
        /// The node.
        node: Vec<u8>,
        /// The resource.
        resource: Vec<u8>,
        /// Its instance name, as the catalog has it: the window's title.
        name: String,
    },
    /// Say something to a party.
    Send {
        /// Their keyhash.
        to: Vec<u8>,
        /// What, as typed.
        text: String,
    },
    /// **What leaving a patron would cost**, worked out and shown.
    Cost {
        /// The identity holding the line.
        by: Vec<u8>,
        /// The patron's keyhash.
        patron: Vec<u8>,
    },
    /// **Leave a patron**, on the second press (`light-client-requirements.md` §6).
    Leave {
        /// The identity holding the line, which is the one that leaves.
        by: Vec<u8>,
        /// The patron's keyhash.
        patron: Vec<u8>,
    },
    /// **Put an identity in focus** [author, 2026-11-23].
    Focus {
        /// Its keyhash.
        by: Vec<u8>,
    },
    /// **Begin a thread on a subnet's board** (`light-client-requirements.md`
    /// §4.5): a title, the words, and how far it travels.
    BoardPost {
        /// The subnet's id.
        subnet: Vec<u8>,
        /// Levels up.
        up: u32,
        /// Levels down.
        down: u32,
        /// The thread's title, as typed.
        title: String,
        /// The words, as typed.
        text: String,
    },
    /// **Continue a thread**: the words alone, a reply carrying its
    /// thread's reach.
    BoardReply {
        /// The subnet's id.
        subnet: Vec<u8>,
        /// The thread's id.
        thread: Vec<u8>,
        /// The words, as typed.
        text: String,
    },
    /// **Name a subnet**, as its root, by convention (`light-client-requirements.md` §4.4).
    Rename {
        /// The subnet's id.
        subnet: Vec<u8>,
        /// The name, as typed.
        name: String,
    },
    /// **The colour of the identity in focus** [author, 2026-11-12], by
    /// its name in the palette.
    Colour {
        /// The palette's name for it.
        name: String,
    },
    /// **How much each subnet's board keeps here** for the identity in
    /// focus (`light-client-requirements.md` §4.5), one of the bounds the
    /// options offer.
    BoardPool {
        /// The bound, in bytes.
        bytes: u64,
    },
    /// **Change what is chosen about a party** [author, 2026-11-23], as the
    /// identity their conversation is held as.
    Person {
        /// Their keyhash.
        peer: Vec<u8>,
        /// What changes.
        change: Change,
    },
}

/// **What a person's options change** [author, 2026-11-23]: their colour,
/// and three toggles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Their colour, by the palette's name; none for the one their key
    /// gives them.
    Colour(Option<String>),
    /// Mute, or unmute.
    Muted,
    /// Mute their notifications, or unmute them.
    Silent,
    /// Block, or unblock.
    Blocked,
}

/// Read one message from the page, or say why it is not one.
pub fn act(message: &str) -> Result<Act, String> {
    let v: serde_json::Value =
        serde_json::from_str(message).map_err(|e| format!("not a message: {e}"))?;
    let s = |k: &str| -> Result<String, String> {
        v.get(k)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(String::from)
            .ok_or_else(|| format!("`{k}` is missing"))
    };
    match s("act")?.as_str() {
        "answer" => {
            return v
                .get("yes")
                .and_then(serde_json::Value::as_bool)
                .map(|yes| Act::Answer { yes })
                .ok_or_else(|| "`yes` is missing".into());
        }
        "launch" => return Ok(Act::Launch),
        "pair-again" => return Ok(Act::PairAgain),
        "earlier" => {
            let peer = s("peer")?;
            node_key(&peer).map_err(|_| "a party is 64 hex characters")?;
            return Ok(Act::Earlier { peer });
        }
        "administer" => {
            return Ok(Act::Administer {
                by: node_key(&s("by")?).map_err(|_| "an identity is 64 hex characters")?,
            });
        }
        "back" => return Ok(Act::Back),
        "go" => {
            let named = s("place")?;
            return screens::Place::parse(&named)
                .map(|place| Act::Go { place })
                .ok_or_else(|| format!("{named} is not a place"));
        }
        "backup" => {
            return participant::unhex(&s("data")?)
                .map(|bytes| Act::Backup { bytes })
                .ok_or_else(|| "the backup did not arrive as hex".into());
        }
        "send" => {
            // what was typed is kept as typed: spaces at its ends are
            // the person's, and only a message of nothing is refused
            let text = v
                .get("text")
                .and_then(serde_json::Value::as_str)
                .filter(|t| !t.trim().is_empty())
                .ok_or("`text` is missing")?
                .to_string();
            return Ok(Act::Send {
                to: node_key(&s("to")?).map_err(|_| "a party's key is 64 hex characters")?,
                text,
            });
        }
        "attach" => {
            return Ok(Act::Attach {
                node: node_key(&s("node")?)?,
                address: node_address(&s("address")?)?,
            });
        }
        "restore" => return s("phrase").map(|phrase| Act::Restore { phrase }),
        "cost" | "leave" => {
            let patron =
                node_key(&s("patron")?).map_err(|_| "a patron's key is 64 hex characters")?;
            let by = node_key(&s("by")?).map_err(|_| "an identity is 64 hex characters")?;
            return Ok(if s("act")? == "cost" {
                Act::Cost { by, patron }
            } else {
                Act::Leave { by, patron }
            });
        }
        "focus" => {
            return Ok(Act::Focus {
                by: node_key(&s("by")?).map_err(|_| "an identity is 64 hex characters")?,
            });
        }
        kind @ ("board-post" | "board-reply") => {
            let subnet =
                node_key(&s("subnet")?).map_err(|_| "a subnet's id is 64 hex characters")?;
            // what was typed is kept as typed; only nothing is refused
            let typed = |k: &str| -> Result<String, String> {
                v.get(k)
                    .and_then(serde_json::Value::as_str)
                    .filter(|t| !t.trim().is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| format!("`{k}` is missing"))
            };
            if kind == "board-reply" {
                return Ok(Act::BoardReply {
                    subnet,
                    thread: node_key(&s("thread")?)
                        .map_err(|_| "a thread's id is 64 hex characters")?,
                    text: typed("text")?,
                });
            }
            // a reach is 0 to 24 each way (`wire-format.md` §10.3)
            let level = |k: &str| -> Result<u32, String> {
                v.get(k)
                    .and_then(serde_json::Value::as_u64)
                    .filter(|n| *n <= 24)
                    .map(|n| n as u32)
                    .ok_or_else(|| format!("`{k}` is a number of levels, 0 to 24"))
            };
            return Ok(Act::BoardPost {
                subnet,
                up: level("up")?,
                down: level("down")?,
                title: typed("title")?,
                text: typed("text")?,
            });
        }
        "rename" => {
            return Ok(Act::Rename {
                subnet: node_key(&s("subnet")?)
                    .map_err(|_| "a subnet's id is 64 hex characters")?,
                name: s("name")?,
            });
        }
        "board-pool" => {
            let bytes = v
                .get("bytes")
                .and_then(serde_json::Value::as_u64)
                .ok_or("a board pool is a number of bytes")?;
            return screens::POOL_CHOICES
                .iter()
                .any(|mb| mb * 1024 * 1024 == bytes)
                .then_some(Act::BoardPool { bytes })
                .ok_or_else(|| format!("{bytes} is not a bound the options offer"));
        }
        "colour" => {
            let name = s("name")?;
            return screens::PALETTE
                .iter()
                .any(|(n, _)| *n == name)
                .then_some(Act::Colour { name: name.clone() })
                .ok_or_else(|| format!("{name} is not a colour of the palette"));
        }
        "person" => {
            let change = match s("set")?.as_str() {
                "colour" => {
                    let name = v.get("colour").and_then(serde_json::Value::as_str);
                    if name.is_some_and(|n| !screens::PALETTE.iter().any(|(p, _)| *p == n)) {
                        return Err("not a colour of the palette".into());
                    }
                    Change::Colour(name.map(String::from))
                }
                "muted" => Change::Muted,
                "silent" => Change::Silent,
                "blocked" => Change::Blocked,
                other => return Err(format!("{other} is not a person's option")),
            };
            return Ok(Act::Person {
                peer: node_key(&s("peer")?).map_err(|_| "a party's key is 64 hex characters")?,
                change,
            });
        }
        "open" => {
            return Ok(Act::Open {
                by: node_key(&s("by")?).map_err(|_| "an identity is 64 hex characters")?,
                node: node_key(&s("node")?)?,
                resource: node_key(&s("resource")?)
                    .map_err(|_| "a resource's key is 64 hex characters")?,
                // the catalog's own words, bounded so a title stays one
                name: s("name")?.chars().take(120).collect(),
            });
        }
        _ => {}
    }
    let provider = s("provider")?;
    if rhtn_launch::provider(&provider).is_none() {
        return Err(format!("{provider} is not a provider this build drives"));
    }
    match s("act")?.as_str() {
        "console" => Ok(Act::Console { provider }),
        "token" => Ok(Act::Token {
            provider,
            token: s("token")?,
        }),
        other => Err(format!("`{other}` is not an act")),
    }
}

/// **A node's keyhash as typed**: 64 hex characters, spaces, colons and an
/// `0x` taken off, as the phone's attach screen takes one.
pub fn node_key(typed: &str) -> Result<Vec<u8>, String> {
    let t: String = typed
        .trim()
        .trim_start_matches("0x")
        .chars()
        .filter(|c| *c != ' ' && *c != ':')
        .collect();
    if t.len() != 64 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("a node's key is 64 hex characters".into());
    }
    Ok((0..32)
        .map(|i| u8::from_str_radix(&t[2 * i..2 * i + 2], 16).expect("hex, checked"))
        .collect())
}

/// **Where a node answers, as typed**: an IP and a port, or an IP alone
/// and the port a launched node answers on.  A name is not one: the
/// kernel resolves nothing, so it is refused here rather than three layers
/// down.
pub fn node_address(typed: &str) -> Result<String, String> {
    let t = typed.trim();
    if let Ok(a) = t.parse::<std::net::SocketAddr>() {
        return Ok(a.to_string());
    }
    let bare = t.trim_start_matches('[').trim_end_matches(']');
    match bare.parse::<std::net::IpAddr>() {
        Ok(ip) => Ok(std::net::SocketAddr::new(ip, rhtn_launch::QUIC_PORT).to_string()),
        Err(_) => Err(format!(
            "{t} is not an address: an IP and a port, such as 203.0.113.7:{}",
            rhtn_launch::QUIC_PORT
        )),
    }
}

/// Where a provider's token is kept: one file a provider, under the
/// directory [`directory`] names, and the same shape the command line
/// reads with `--credential-file`.
pub fn credential_file(dir: &Path, provider: &str) -> PathBuf {
    dir.join(format!("{}.token", provider.to_ascii_lowercase()))
}

/// **Where this client keeps what it is given**: the directory named on
/// the command line, else `$XDG_CONFIG_HOME/rhtn`, else `~/.config/rhtn`.
/// Nothing is written until an operator pastes something.
pub fn directory(named: Option<&str>) -> Result<PathBuf, String> {
    if let Some(d) = named {
        return Ok(PathBuf::from(d));
    }
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME").filter(|x| !x.is_empty()) {
        return Ok(PathBuf::from(x).join("rhtn"));
    }
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .ok_or("neither XDG_CONFIG_HOME nor HOME is set: name a directory on the command line")?;
    Ok(PathBuf::from(home).join(".config/rhtn"))
}

/// **Try a token at its provider, and keep it only if it answers.**
///
/// The one read-only call every adapter has is the regions, so that is
/// the try: a token that cannot list them cannot launch either, and the
/// operator hears the provider's own words now rather than at the launch.
/// What is kept is kept readable by its owner alone — a provider's token
/// is a key, and `keys mint` writes an identity the same way.
pub fn verify_and_keep(
    t: &dyn Transport,
    dir: &Path,
    provider: &str,
    token: &str,
) -> Result<String, String> {
    let p = rhtn_launch::provider(provider)
        .ok_or_else(|| format!("{provider} is not a provider this build drives"))?;
    let regions = p.regions(t, token)?;
    let sizes = p.sizes(t, token)?;
    let at = credential_file(dir, provider);
    if let Some(d) = at.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    write_private(&at, token)?;
    Ok(format!(
        "the token answers at {}: {} regions and {} sizes offered. Kept in {} — the launch reads \
         it from there, and `rhtn node launch --credential-file {}` does too.",
        p.name(),
        regions.len(),
        sizes.len(),
        at.display(),
        at.display()
    ))
}

/// Write a token readable by its owner alone.
fn write_private(at: &Path, text: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(at)
            .map_err(|e| format!("{}: {e}", at.display()))?;
        f.write_all(text.as_bytes())
            .map_err(|e| format!("{}: {e}", at.display()))?;
        std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("{}: {e}", at.display()))
    }
    #[cfg(not(unix))]
    {
        std::fs::write(at, text).map_err(|e| format!("{}: {e}", at.display()))
    }
}

/// The console a provider's steps start at.
pub fn console_of(provider: &str) -> Option<&'static str> {
    rhtn_launch::provider(provider).map(|p| p.token_page())
}

/// **The page this client serves itself**: every provider this build
/// drives, its steps in its own words, and one field for what they
/// produce.  Composed here rather than written as a file beside the
/// binary, so that a provider added to the launch crate appears here
/// without a second place to remember.
pub fn page() -> String {
    let mut cards = String::new();
    for name in rhtn_launch::provider_names() {
        let Some(p) = rhtn_launch::provider(name) else {
            continue;
        };
        let steps: String = p
            .token_walkthrough()
            .iter()
            .map(|s| format!("<li>{}</li>", escape(s)))
            .collect();
        cards.push_str(&format!(
            "<section data-provider=\"{name}\">\n\
             <h2>{} <span class=\"asn\">AS{}</span></h2>\n\
             <p class=\"sub\">The token is yours, made at {}&#39;s own console and pasted back. \
             Nothing of this project&#39;s is registered there, and this window cannot read \
             their page — you paste, it is tried once, and it is kept readable by you alone.</p>\n\
             <ol>{steps}</ol>\n\
             <button data-act=\"console\">Open {}&#39;s console</button>\n\
             <input type=\"password\" placeholder=\"paste the token\" autocomplete=\"off\" \
             spellcheck=\"false\">\n\
             <button data-act=\"token\">Try it and keep it</button>\n\
             </section>\n",
            escape(p.name()),
            p.asn(),
            escape(p.name()),
            escape(p.name()),
        ));
    }
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <title>Launch a node</title>\n<style>\n{STYLE}\n</style></head><body>\n\
         <p><button id=\"back\">← Back</button></p>\n\
         <h1>Launch a node at a hosting provider</h1>\n\
         <p class=\"sub\">One instance, at a provider you choose, with a credential of your own \
         and no broker between (<code>light-client-requirements.md</code> §10).</p>\n\
         {cards}<pre id=\"said\"></pre>\n<script>\n{SCRIPT}\n</script></body></html>\n"
    )
}

/// What the page says back, as the shell hands it: a line for the person,
/// and whether it went through.
pub fn said(ok: bool, text: &str) -> String {
    format!(
        "document.getElementById('said').textContent = {};\
         document.getElementById('said').className = {};",
        json_string(text),
        if ok { "'ok'" } else { "'bad'" }
    )
}

/// A string as JavaScript source reads it.
fn json_string(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = ":root { color-scheme: light dark }\n\
     body { font: 15px/1.55 system-ui, sans-serif; margin: 0 auto; padding: 1.5rem; max-width: 46rem }\n\
     h1 { font-size: 1.2rem; margin: 0 0 .25rem }\n\
     h2 { font-size: 1rem; margin: 0 0 .25rem }\n\
     .asn { font-weight: normal; opacity: .6; font-size: .85rem }\n\
     p.sub { margin: 0 0 1rem; opacity: .75 }\n\
     section { border: 1px solid rgba(127,127,127,.35); border-radius: 6px; padding: 1rem; margin: 1rem 0 }\n\
     ol { margin: 0 0 1rem; padding-left: 1.4rem }\n\
     li { margin: .3rem 0 }\n\
     input { width: 100%; box-sizing: border-box; padding: .5rem; margin: .5rem 0;\n\
       font: 13px ui-monospace, monospace }\n\
     button { padding: .45rem .8rem; margin-right: .4rem }\n\
     pre { padding: .6rem .8rem; background: rgba(127,127,127,.12); border-radius: 4px;\n\
       white-space: pre-wrap; font: 13px/1.5 ui-monospace, monospace; min-height: 1.5em }\n\
     pre.bad { background: rgba(200,80,80,.18) }";

const SCRIPT: &str = "document.getElementById('back').addEventListener('click', () =>\n\
  window.ipc.postMessage(JSON.stringify({ act: 'go', place: 'options' })));\n\
for (const s of document.querySelectorAll('section')) {\n\
  const provider = s.dataset.provider;\n\
  const field = s.querySelector('input');\n\
  for (const b of s.querySelectorAll('button')) {\n\
    b.addEventListener('click', () => {\n\
      const act = b.dataset.act;\n\
      const message = { act, provider };\n\
      if (act === 'token') {\n\
        if (!field.value.trim()) { document.getElementById('said').textContent = 'paste the token first'; return }\n\
        message.token = field.value.trim();\n\
      }\n\
      document.getElementById('said').textContent = 'asking…';\n\
      window.ipc.postMessage(JSON.stringify(message));\n\
    });\n\
  }\n\
}";

#[cfg(test)]
mod tests {
    use super::*;

    /// A transport that answers from a script, as the launch crate's own
    /// tests use.
    struct Scripted(std::cell::RefCell<Vec<(u16, String)>>);

    impl Transport for Scripted {
        fn exchange(
            &self,
            _: &str,
            _: &str,
            _: &[(&str, &str)],
            _: Option<&str>,
        ) -> Result<(u16, String), String> {
            self.0
                .borrow_mut()
                .pop()
                .ok_or("no answer scripted".to_string())
        }
    }

    fn scripted(answers: Vec<(u16, &str)>) -> Scripted {
        Scripted(std::cell::RefCell::new(
            answers
                .into_iter()
                .map(|(s, b)| (s, b.to_string()))
                .rev()
                .collect(),
        ))
    }

    #[test]
    fn the_page_carries_every_provider_this_build_drives_and_its_own_steps() {
        let html = page();
        for name in rhtn_launch::provider_names() {
            let p = rhtn_launch::provider(name).unwrap();
            assert!(
                html.contains(&format!("data-provider=\"{name}\"")),
                "{name} is on the page"
            );
            assert!(html.contains(&format!("AS{}", p.asn())));
            // its steps, in its own words
            let first = p.token_walkthrough()[0];
            assert!(html.contains(&escape(first)), "{name}: {first}");
        }
        assert!(
            html.contains("type=\"password\""),
            "a token is not shown as it is typed"
        );
        assert!(
            html.contains("cannot read \n             their page") || html.contains("cannot read"),
            "the page says what it does not do"
        );
    }

    #[test]
    fn a_message_from_the_page_is_read_and_anything_else_refused() {
        assert_eq!(
            act(r#"{"act":"console","provider":"hetzner"}"#).unwrap(),
            Act::Console {
                provider: "hetzner".into()
            }
        );
        assert_eq!(
            act(r#"{"act":"token","provider":"digitalocean","token":" dop_v1_x "}"#).unwrap(),
            Act::Token {
                provider: "digitalocean".into(),
                token: "dop_v1_x".into()
            }
        );
        assert_eq!(
            act(r#"{"act":"answer","yes":true}"#).unwrap(),
            Act::Answer { yes: true }
        );
        assert_eq!(act(r#"{"act":"launch"}"#).unwrap(), Act::Launch);
        assert_eq!(act(r#"{"act":"pair-again"}"#).unwrap(), Act::PairAgain);
        let k = "ab".repeat(32);
        assert_eq!(
            act(&format!(r#"{{"act":"earlier","peer":"{k}"}}"#)).unwrap(),
            Act::Earlier { peer: k }
        );
        assert!(act(r#"{"act":"earlier","peer":"ab"}"#).is_err());
        assert_eq!(act(r#"{"act":"back"}"#).unwrap(), Act::Back);
        let k = "ab".repeat(32);
        assert_eq!(
            act(&format!(r#"{{"act":"cost","patron":"{k}","by":"{k}"}}"#)).unwrap(),
            Act::Cost {
                by: vec![0xab; 32],
                patron: vec![0xab; 32]
            }
        );
        assert_eq!(
            act(&format!(r#"{{"act":"leave","patron":"{k}","by":"{k}"}}"#)).unwrap(),
            Act::Leave {
                by: vec![0xab; 32],
                patron: vec![0xab; 32]
            }
        );
        assert_eq!(
            act(&format!(
                r#"{{"act":"person","peer":"{k}","set":"colour","colour":"Teal"}}"#
            ))
            .unwrap(),
            Act::Person {
                peer: vec![0xab; 32],
                change: Change::Colour(Some("Teal".into()))
            }
        );
        assert_eq!(
            act(&format!(
                r#"{{"act":"person","peer":"{k}","set":"colour"}}"#
            ))
            .unwrap(),
            Act::Person {
                peer: vec![0xab; 32],
                change: Change::Colour(None)
            }
        );
        assert_eq!(
            act(&format!(
                r#"{{"act":"person","peer":"{k}","set":"blocked"}}"#
            ))
            .unwrap(),
            Act::Person {
                peer: vec![0xab; 32],
                change: Change::Blocked
            }
        );
        assert!(
            act(&format!(
                r#"{{"act":"person","peer":"{k}","set":"colour","colour":"Mauve"}}"#
            ))
            .is_err()
        );
        assert!(
            act(&format!(
                r#"{{"act":"person","peer":"{k}","set":"forget"}}"#
            ))
            .is_err()
        );
        assert_eq!(
            act(&format!(r#"{{"act":"focus","by":"{k}"}}"#)).unwrap(),
            Act::Focus { by: vec![0xab; 32] }
        );
        assert_eq!(
            act(&format!(
                r#"{{"act":"rename","subnet":"{k}","name":"Manse"}}"#
            ))
            .unwrap(),
            Act::Rename {
                subnet: vec![0xab; 32],
                name: "Manse".into()
            }
        );
        assert_eq!(
            act(r#"{"act":"colour","name":"Teal"}"#).unwrap(),
            Act::Colour {
                name: "Teal".into()
            }
        );
        assert!(
            act(r#"{"act":"colour","name":"Mauve"}"#).is_err(),
            "not the palette's"
        );
        assert_eq!(
            act(r#"{"act":"board-pool","bytes":26214400}"#).unwrap(),
            Act::BoardPool { bytes: 26_214_400 }
        );
        assert!(
            act(r#"{"act":"board-pool","bytes":1000}"#).is_err(),
            "not a bound the options offer"
        );
        assert_eq!(
            act(r#"{"act":"backup","data":"00Ff"}"#).unwrap(),
            Act::Backup {
                bytes: vec![0, 0xff]
            }
        );
        assert_eq!(
            act(r#"{"act":"restore","phrase":" a b "}"#).unwrap(),
            Act::Restore {
                phrase: "a b".into()
            }
        );
        assert_eq!(
            act(r#"{"act":"go","place":"direct"}"#).unwrap(),
            Act::Go {
                place: screens::Place::Direct
            }
        );
        assert_eq!(
            act(&format!(
                r#"{{"act":"send","to":"{}","text":" hi "}}"#,
                "cd".repeat(32)
            ))
            .unwrap(),
            Act::Send {
                to: vec![0xcd; 32],
                text: " hi ".into()
            }
        );
        let k = "ab".repeat(32);
        assert_eq!(
            act(&format!(
                r#"{{"act":"attach","node":"0x{k}","address":"203.0.113.7"}}"#
            ))
            .unwrap(),
            Act::Attach {
                node: vec![0xab; 32],
                address: "203.0.113.7:7431".into()
            }
        );
        assert_eq!(
            act(&format!(
                r#"{{"act":"open","by":"{k}","node":"{k}","resource":"{}","name":" Notices "}}"#,
                "cd".repeat(32)
            ))
            .unwrap(),
            Act::Open {
                by: vec![0xab; 32],
                node: vec![0xab; 32],
                resource: vec![0xcd; 32],
                name: "Notices".into()
            }
        );
        assert_eq!(node_address("[::1]:9").unwrap(), "[::1]:9");
        assert_eq!(node_address("::1").unwrap(), "[::1]:7431");
        for (message, why) in [
            (
                r#"{"act":"attach","node":"abcd","address":"203.0.113.7:1"}"#,
                "64 hex",
            ),
            (r#"{"act":"go","place":"meet"}"#, "not a place"),
            (
                &*format!(r#"{{"act":"open","by":"{k}","node":"{k}","resource":"ab","name":"x"}}"#),
                "resource's key",
            ),
            (
                &*format!(r#"{{"act":"open","by":"{k}","node":"{k}","resource":"{k}"}}"#),
                "name",
            ),
            (r#"{"act":"backup","data":"zz"}"#, "hex"),
            (r#"{"act":"backup","data":""}"#, "data"),
            (r#"{"act":"send","to":"ab","text":"x"}"#, "64 hex"),
            (
                &*format!(r#"{{"act":"send","to":"{k}","text":"  "}}"#),
                "text",
            ),
            (
                &*format!(r#"{{"act":"attach","node":"{k}","address":"node.example:7431"}}"#),
                "not an address",
            ),
            (
                r#"{"act":"token","provider":"aws","token":"x"}"#,
                "provider",
            ),
            (r#"{"act":"erase","provider":"hetzner"}"#, "act"),
            (r#"{"act":"answer"}"#, "yes"),
            (r#"{"act":"token","provider":"hetzner"}"#, "token"),
            ("not json", "message"),
        ] {
            let e = act(message).unwrap_err();
            assert!(e.contains(why), "{message}: {e}");
        }
    }

    #[test]
    fn a_token_that_answers_is_kept_owner_only_and_one_that_does_not_is_not_kept() {
        let dir = std::env::temp_dir().join(format!("rhtn-desktop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let refusing = scripted(vec![(
            401,
            r#"{"error":{"code":"unauthorized","message":"the token you have provided is invalid"}}"#,
        )]);
        let e = verify_and_keep(&refusing, &dir, "hetzner", "nope").unwrap_err();
        assert_eq!(
            e,
            "Hetzner answered 401: the token you have provided is invalid"
        );
        assert!(
            !credential_file(&dir, "hetzner").exists(),
            "a token that does not answer is not kept"
        );

        let answering = scripted(vec![
            (
                200,
                r#"{"locations":[{"id":1,"name":"fsn1","description":"Falkenstein","country":"DE","city":"Falkenstein"}]}"#,
            ),
            (
                200,
                r#"{"server_types":[{"name":"cx22","cores":2,"memory":4.0,"architecture":"x86","prices":[{"location":"fsn1","price_monthly":{"net":"4.59"}}],"locations":[{"name":"fsn1","available":true}]}]}"#,
            ),
        ]);
        let said = verify_and_keep(&answering, &dir, "hetzner", "good-token").unwrap();
        assert!(said.contains("1 regions and 1 sizes"), "{said}");
        let at = credential_file(&dir, "hetzner");
        assert_eq!(std::fs::read_to_string(&at).unwrap(), "good-token");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&at).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "a provider's token is a key");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_directory_is_the_one_named_or_the_platforms_own() {
        assert_eq!(directory(Some("/tmp/x")).unwrap(), PathBuf::from("/tmp/x"));
        // the environment of the test process is not ours to set about,
        // so this asserts the shape rather than one answer
        let d = directory(None).unwrap();
        assert!(d.ends_with("rhtn"), "{}", d.display());
    }

    #[test]
    fn what_the_page_is_told_is_a_string_a_script_can_hold() {
        let s = said(false, "Hetzner answered 401: \"no\"\nand stopped");
        assert!(s.contains("'bad'"));
        assert!(s.contains("\\\"no\\\""), "a quote is escaped: {s}");
        assert!(s.contains("\\n"), "and a newline: {s}");
        assert!(!s.contains("\nand stopped"), "nothing breaks the script");
    }

    #[test]
    fn the_boards_acts_keep_what_was_typed_and_bound_the_reach() {
        let subnet = "22".repeat(32);
        let thread = "44".repeat(32);
        let begun = format!(
            r#"{{"act":"board-post","subnet":"{subnet}","up":3,"down":0,"title":" A title ","text":"two\nlines "}}"#
        );
        assert_eq!(
            act(&begun),
            Ok(Act::BoardPost {
                subnet: vec![0x22; 32],
                up: 3,
                down: 0,
                title: " A title ".into(),
                text: "two\nlines ".into(),
            })
        );
        let reply = format!(
            r#"{{"act":"board-reply","subnet":"{subnet}","thread":"{thread}","text":"yes"}}"#
        );
        assert_eq!(
            act(&reply),
            Ok(Act::BoardReply {
                subnet: vec![0x22; 32],
                thread: vec![0x44; 32],
                text: "yes".into(),
            })
        );
        // nothing typed, a reach past the path bound, and ids that are not ids
        for bad in [
            format!(
                r#"{{"act":"board-post","subnet":"{subnet}","up":2,"down":2,"title":" ","text":"x"}}"#
            ),
            format!(
                r#"{{"act":"board-post","subnet":"{subnet}","up":2,"down":2,"title":"t","text":""}}"#
            ),
            format!(
                r#"{{"act":"board-post","subnet":"{subnet}","up":25,"down":2,"title":"t","text":"x"}}"#
            ),
            format!(r#"{{"act":"board-post","subnet":"{subnet}","up":2,"title":"t","text":"x"}}"#),
            r#"{"act":"board-post","subnet":"22","up":2,"down":2,"title":"t","text":"x"}"#
                .to_string(),
            format!(r#"{{"act":"board-reply","subnet":"{subnet}","thread":"44","text":"x"}}"#),
            format!(
                r#"{{"act":"board-reply","subnet":"{subnet}","thread":"{thread}","text":"  "}}"#
            ),
        ] {
            assert!(act(&bad).is_err(), "{bad}");
        }
    }
}
