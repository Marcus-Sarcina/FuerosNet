//! **The participant's screens, once the desktop is up**: a rail of the
//! places — direct messages, every subnet this key is in, the options —
//! and the place itself beside it, as the phone's shell lays them out
//! (its `ShellActivity` and `Chrome`).  There is no meeting here: a
//! desktop has no camera, and somebody is met on a phone.
//!
//! **Everything is drawn from what the kernel says, handed in as plain
//! rows**, so this module is the mapping from the model to the page and
//! nothing else: `main.rs` asks the kernel, builds a [`Snapshot`], and
//! carries the HTML to the window.  A place is drawn whole when it is
//! gone to, and its rail and body alone when something it shows changed,
//! so what is being typed beside them survives a message arriving.

use crate::participant::{Device, escape, hex, shell};
use crate::threads::{Delivery, Entry, Message};

/// **Where the window is.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// Who this desktop is a device of, and where it stands.
    Home,
    /// Direct messages, which is also the people this key knows.
    Direct,
    /// One thread, with the other party's keyhash in hex.
    Thread(String),
    /// One subnet, by its id in hex.
    Subnet(String),
    /// The catalog as swept from every node attached to.
    Resources,
    /// The connection, a node to connect to, a node to launch.
    Options,
}

impl Place {
    /// A place as the page names it: `home`, `direct`, `options`,
    /// `thread:<hex>`, `subnet:<hex>`.
    pub fn parse(s: &str) -> Option<Place> {
        let hex64 = |h: &str| {
            (h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_lowercase())
        };
        match s.split_once(':') {
            None => match s {
                "home" => Some(Place::Home),
                "direct" => Some(Place::Direct),
                "options" => Some(Place::Options),
                "resources" => Some(Place::Resources),
                _ => None,
            },
            Some(("thread", h)) => hex64(h).map(Place::Thread),
            Some(("subnet", h)) => hex64(h).map(Place::Subnet),
            Some(_) => None,
        }
    }

    /// The name [`Place::parse`] reads back.
    pub fn token(&self) -> String {
        match self {
            Place::Home => "home".into(),
            Place::Direct => "direct".into(),
            Place::Options => "options".into(),
            Place::Resources => "resources".into(),
            Place::Thread(h) => format!("thread:{h}"),
            Place::Subnet(h) => format!("subnet:{h}"),
        }
    }
}

/// One subnet as the rail and its page name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubnetRow {
    /// Its id, in hex.
    pub id: String,
    /// The name its root published, else this key's own for it.
    pub name: Option<String>,
    /// This key's rank there, said in words.
    pub rank: String,
}

/// One person this key knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// Their keyhash, in hex.
    pub key: String,
    /// The alias they published, where one has been received.
    pub alias: Option<String>,
    /// How they are known, in words.
    pub how: String,
}

impl Person {
    /// What the screens call them: their alias, else the start of their key.
    pub fn name(&self) -> String {
        self.alias.clone().unwrap_or_else(|| short(&self.key))
    }
}

/// One resource of the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRow {
    /// Its service type.
    pub service: String,
    /// Its instance name.
    pub instance: String,
    /// The node holding it, in hex.
    pub node: String,
    /// The resource's own keyhash, in hex, which is what opens it.
    pub resource: String,
    /// Whether the node's word on it is older than its refresh.
    pub stale: bool,
}

/// **What every screen is drawn from**: the kernel's answers at one
/// moment, as rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// This identity's keyhash.
    pub me: Vec<u8>,
    /// **What this person is called**, as the phone's foot has it: a
    /// subnet's own name for them first, then the default the kernel
    /// keeps [author, 2026-10-27]; none where they have published neither.
    pub name: Option<String>,
    /// What this desktop is to it.
    pub device: Device,
    /// Where it is connected, in one line.
    pub connection: String,
    /// Every subnet this key is in.
    pub subnets: Vec<SubnetRow>,
    /// The people this key knows, nodes left out.
    pub people: Vec<Person>,
    /// The threads the index names.
    pub threads: Vec<Entry>,
}

impl Snapshot {
    /// This person's name, else the start of their key.
    pub fn who(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("{}…", short(&hex(&self.me))))
    }

    /// What the screens call the party with key `peer`: a thread's name,
    /// a contact's alias, else the start of the key.
    pub fn name_of(&self, peer: &str) -> String {
        self.people
            .iter()
            .find(|p| p.key == peer)
            .and_then(|p| p.alias.clone())
            .or_else(|| {
                self.threads
                    .iter()
                    .find(|t| t.peer == peer && t.name != peer)
                    .map(|t| t.name.clone())
            })
            .unwrap_or_else(|| short(peer))
    }
}

/// **What a place shows beyond the snapshot**: a thread's newest page, a
/// subnet's catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// The place needs nothing more.
    Plain,
    /// A thread's messages, oldest first, and whether earlier ones exist.
    Thread(Vec<Message>, bool),
    /// The catalog as swept from the serving node.
    Catalog(Vec<ResourceRow>),
}

/// **A place drawn whole**: the page the window loads when it is gone to.
pub fn page(snap: &Snapshot, place: &Place, body: &Body) -> String {
    let compose = match place {
        Place::Thread(peer) => format!(
            "<div id=\"compose\"><textarea id=\"text\" rows=\"3\" data-peer=\"{peer}\" \
             placeholder=\"Say something to {}\"></textarea>\
             <p><button id=\"send\">Send</button></p></div>",
            escape(&snap.name_of(peer))
        ),
        _ => String::new(),
    };
    shell(
        "FuerosNet",
        &format!(
            "<nav id=\"rail\">{}</nav>\n<main id=\"main\">{}</main>\n{compose}",
            rail(snap, place),
            main(snap, place, body)
        ),
    )
}

/// **A place redrawn where it stands**: the rail and the body replaced,
/// anything typed beside them kept, and a thread scrolled to its newest.
pub fn refresh(snap: &Snapshot, place: &Place, body: &Body) -> String {
    format!(
        "document.getElementById('rail').innerHTML = {};\
         document.getElementById('main').innerHTML = {};\
         if (document.getElementById('messages')) window.scrollTo(0, document.body.scrollHeight);",
        js(&rail(snap, place)),
        js(&main(snap, place, body))
    )
}

/// The rail: who, the pinned direct messages, every subnet, the options.
pub fn rail(snap: &Snapshot, place: &Place) -> String {
    let here = |p: &Place| if p == place { " class=\"here\"" } else { "" };
    let mut out = format!(
        "<a data-go=\"home\"{}><b{}>{}</b><br><small>{}</small></a>\n\
         <a data-go=\"direct\"{}>Direct messages</a>\n\
         <a data-go=\"resources\"{}>Resources</a>\n<h3>Subnets</h3>\n",
        here(&Place::Home),
        if snap.name.is_some() {
            ""
        } else {
            " class=\"mono\""
        },
        escape(&snap.who()),
        match snap.device {
            Device::Paired { .. } => "paired from the phone",
            Device::Restored => "restored from the phrase",
        },
        match place {
            Place::Thread(_) => " class=\"here\"",
            _ => here(&Place::Direct),
        },
        here(&Place::Resources),
    );
    if snap.subnets.is_empty() {
        out.push_str("<small>none yet</small>\n");
    }
    for s in &snap.subnets {
        let p = Place::Subnet(s.id.clone());
        out.push_str(&format!(
            "<a data-go=\"{}\"{}>{}</a>\n",
            p.token(),
            here(&p),
            escape(&subnet_name(s))
        ));
    }
    out.push_str(&format!(
        "<div class=\"foot\"><small>{}</small><a data-go=\"options\"{}>Options</a></div>",
        escape(&snap.connection),
        here(&Place::Options)
    ));
    out
}

fn main(snap: &Snapshot, place: &Place, body: &Body) -> String {
    match place {
        Place::Home => home(snap),
        Place::Direct => direct(snap),
        Place::Thread(peer) => match body {
            Body::Thread(messages, more) => thread(snap, peer, messages, *more),
            _ => thread(snap, peer, &[], false),
        },
        Place::Subnet(id) => match body {
            Body::Catalog(rows) => subnet(snap, id, rows),
            _ => subnet(snap, id, &[]),
        },
        Place::Options => options(snap),
        Place::Resources => match body {
            Body::Catalog(rows) => resources(rows),
            _ => resources(&[]),
        },
    }
}

fn home(snap: &Snapshot) -> String {
    let (said, dataset) = match &snap.device {
        Device::Paired { days, dataset } => (
            format!(
                "This desktop presents its own key under a run of {days} days the phone \
                 signed. It reads everything and signs nothing."
            ),
            match dataset {
                Some(true) => "<p>Everything the phone holds of the identity came across.</p>",
                Some(false) => {
                    "<p>Nothing else was handed over; pair again to bring it across.</p>"
                }
                None => "",
            },
        ),
        Device::Restored => (
            "Restored from the phrase: this desktop holds the identity and can sign as it. \
             It knows only the nodes you introduce it to."
                .to_string(),
            "",
        ),
    };
    // **a restore brings back the key and not the archive**, and says both
    // (`light-client-requirements.md` §2): asked here until the archive is
    // in, which an empty one is the sign of
    let archive = if snap.device == Device::Restored && snap.subnets.is_empty() {
        BACKUP
    } else {
        ""
    };
    let next = if snap.connection.starts_with("Connected") {
        format!(
            "<p>{} subnets, {} people, {} conversations.</p>",
            snap.subnets.len(),
            snap.people.len(),
            snap.threads.len()
        )
    } else {
        "<p><button data-go=\"options\">Connect to a node</button></p>".to_string()
    };
    format!(
        "<h1>A device of {}</h1>\n<p class=\"sub mono\">{}</p>\n\
         <p class=\"sub\">{said}</p>\n{dataset}\n<p>{}</p>\n{next}\n{archive}",
        escape(&snap.who()),
        hex(&snap.me),
        escape(&snap.connection)
    )
}

fn direct(snap: &Snapshot) -> String {
    let mut out = String::from("<h1>Direct messages</h1>\n");
    if snap.threads.is_empty() && snap.people.is_empty() {
        out.push_str(
            "<p class=\"sub\">Nobody yet. Somebody is met on a phone; once met, they are \
             here too, and a message sent from either device is one conversation.</p>",
        );
        return out;
    }
    if !snap.threads.is_empty() {
        out.push_str("<h2>Conversations</h2>\n<ul class=\"rows\">\n");
        for t in &snap.threads {
            out.push_str(&format!(
                "<li><a data-go=\"thread:{}\">{}</a></li>\n",
                t.peer,
                escape(&snap.name_of(&t.peer))
            ));
        }
        out.push_str("</ul>\n");
    }
    let quiet: Vec<&Person> = snap
        .people
        .iter()
        .filter(|p| !snap.threads.iter().any(|t| t.peer == p.key))
        .collect();
    if !quiet.is_empty() {
        out.push_str("<h2>People</h2>\n<ul class=\"rows\">\n");
        for p in quiet {
            out.push_str(&format!(
                "<li><a data-go=\"thread:{}\">{}</a> <small>{}</small></li>\n",
                p.key,
                escape(&p.name()),
                escape(&p.how)
            ));
        }
        out.push_str("</ul>\n");
    }
    out
}

fn thread(snap: &Snapshot, peer: &str, messages: &[Message], more: bool) -> String {
    let mut out = format!(
        "<h1>{}</h1>\n<p class=\"sub mono\">{peer}</p>\n",
        escape(&snap.name_of(peer))
    );
    if more {
        out.push_str("<p class=\"sub\">Earlier messages are kept; the newest are shown.</p>\n");
    }
    out.push_str("<div id=\"messages\">\n");
    if messages.is_empty() {
        out.push_str("<p class=\"sub\">Nothing said yet.</p>\n");
    }
    for m in messages {
        let state = match (m.delivery, &m.reason) {
            (Delivery::Sending, _) => " <small>sending…</small>".to_string(),
            (Delivery::Unsent, why) => format!(
                " <small class=\"bad\">not sent{}</small>",
                why.as_ref()
                    .map(|w| format!(": {}", escape(w)))
                    .unwrap_or_default()
            ),
            (Delivery::Sent, _) => String::new(),
        };
        out.push_str(&format!(
            "<div class=\"said{}\"><p>{}</p>{state}</div>\n",
            if m.mine { " mine" } else { "" },
            escape(&m.text)
        ));
    }
    out.push_str("</div>");
    out
}

fn subnet(snap: &Snapshot, id: &str, rows: &[ResourceRow]) -> String {
    let s = snap.subnets.iter().find(|s| s.id == id);
    format!(
        "<h1>{}</h1>\n<p class=\"sub\">{}</p>\n<p class=\"sub mono\">{id}</p>\n\
         <h2>Resources</h2>\n<p class=\"sub\">The catalog as swept from your serving node — \
         which of them belong to this subnet is not a question it can answer yet.</p>\n{}",
        escape(&s.map(subnet_name).unwrap_or_else(|| short(id))),
        escape(&s.map(|s| s.rank.clone()).unwrap_or_default()),
        catalog(rows)
    )
}

/// **The catalog on its own**, with or without a subnet held: each
/// device sweeps every node it attaches to for what is available now
/// (design §23.3), so a desktop reaches it once connected.
fn resources(rows: &[ResourceRow]) -> String {
    format!(
        "<h1>Resources</h1>\n<p class=\"sub\">The catalog as swept from every node this \
         desktop is connected to.</p>\n{}\n\
         <p><button data-act=\"sweep\">Sweep again</button></p>",
        catalog(rows)
    )
}

fn catalog(rows: &[ResourceRow]) -> String {
    if rows.is_empty() {
        return "<p>Nothing swept yet.</p>".to_string();
    }
    let mut out = String::from("<ul class=\"rows\">\n");
    for r in rows {
        out.push_str(&format!(
            "<li>{} <small>{} · at {}…{}</small> <button data-open=\"{}\" data-node=\"{}\" \
             data-name=\"{}\">Open</button></li>\n",
            escape(&r.instance),
            escape(&r.service),
            short(&r.node),
            if r.stale { " · stale" } else { "" },
            escape(&r.resource),
            escape(&r.node),
            escape(&r.instance),
        ));
    }
    out.push_str("</ul>");
    out
}

fn options(snap: &Snapshot) -> String {
    let full = hex(&snap.me);
    format!(
        "<h1>Options</h1>\n<p>{}</p>\n\
         <h2>Connect to a node already running</h2>\n\
         <p class=\"sub\">Its key and the address it answers at. A node you launched \
         answers to your own key, which is filled in.</p>\n\
         <label>Node key<input id=\"node\" class=\"mono\" value=\"{full}\" \
         autocomplete=\"off\" spellcheck=\"false\"></label>\n\
         <label>Address<input id=\"address\" class=\"mono\" \
         placeholder=\"203.0.113.7:7431\" autocomplete=\"off\" spellcheck=\"false\"></label>\n\
         <p><button id=\"connect\">Connect</button></p>\n\
         <h2>Or launch one</h2>\n\
         <p><button data-act=\"launch\">Launch a node at a hosting provider</button></p>\n{}",
        escape(&snap.connection),
        if snap.device == Device::Restored {
            BACKUP
        } else {
            ""
        }
    )
}

/// **The archive from a backup file**, the one way to it on a desktop
/// restored from the phrase: the archive never crosses the network
/// (design §23.3), and a restore brings back the key without it
/// (`light-client-requirements.md` §2).  The file is opened under the
/// identity, so only a backup of this one takes.
const BACKUP: &str = "<h2>Bring the archive from a backup</h2>\n\
     <p class=\"sub\">Restoring from the phrase brought back the key and not the archive: \
     your subnets, contacts and records come from a backup or not at all, and never over the \
     network. Write one on the phone, bring the file here, and open it.</p>\n\
     <label>Backup file<input id=\"backup\" type=\"file\" accept=\".rhtn\"></label>\n\
     <p><button id=\"import\">Open the backup</button></p>";

fn subnet_name(s: &SubnetRow) -> String {
    s.name
        .clone()
        .unwrap_or_else(|| format!("subnet {}", short(&s.id)))
}

fn short(h: &str) -> String {
    h.chars().take(16).collect()
}

fn js(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            me: vec![0xab; 32],
            name: Some("Ann <A>".into()),
            device: Device::Restored,
            connection: "Connected to 1111111111111111…".into(),
            subnets: vec![SubnetRow {
                id: "22".repeat(32),
                name: Some("Mirror <Manse>".into()),
                rank: "operator".into(),
            }],
            people: vec![
                Person {
                    key: "33".repeat(32),
                    alias: Some("Carol".into()),
                    how: "met".into(),
                },
                Person {
                    key: "44".repeat(32),
                    alias: None,
                    how: "introduced".into(),
                },
            ],
            threads: vec![Entry {
                peer: "33".repeat(32),
                name: "33".repeat(32),
                direct: true,
            }],
        }
    }

    #[test]
    fn a_place_reads_back_from_its_name_and_nothing_else_is_one() {
        for p in [
            Place::Home,
            Place::Direct,
            Place::Options,
            Place::Thread("ab".repeat(32)),
            Place::Subnet("cd".repeat(32)),
            Place::Resources,
        ] {
            assert_eq!(Place::parse(&p.token()), Some(p));
        }
        assert_eq!(
            Place::parse(&format!("thread:{}", "AB".repeat(32))),
            Some(Place::Thread("ab".repeat(32)))
        );
        for bad in [
            "",
            "meet",
            "thread:ab",
            "subnet:",
            "erase:00",
            "thread:../../x",
        ] {
            assert_eq!(Place::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_rail_holds_every_subnet_and_marks_where_the_window_is() {
        let s = snap();
        let r = rail(&s, &Place::Subnet("22".repeat(32)));
        assert!(r.contains("Mirror &lt;Manse&gt;"), "escaped");
        assert!(r.contains(&format!(
            "data-go=\"subnet:{}\" class=\"here\"",
            "22".repeat(32)
        )));
        assert!(r.contains("restored from the phrase") && r.contains("Ann &lt;A&gt;"));
        assert!(!r.contains("abababababababab…"), "the name, not the key");
        assert!(
            rail(&s, &Place::Thread("33".repeat(32))).contains("data-go=\"direct\" class=\"here\"")
        );
    }

    #[test]
    fn direct_messages_name_conversations_first_then_the_people_not_yet_spoken_to() {
        let d = direct(&snap());
        let carol = d.find("Carol").unwrap();
        let people = d.find("<h2>People</h2>").unwrap();
        assert!(carol < people, "the thread's party is under conversations");
        assert!(d.contains("4444444444444444") && d.matches("Carol").count() == 1);
    }

    #[test]
    fn a_thread_escapes_what_was_said_and_says_what_was_not_sent() {
        let s = snap();
        let peer = "33".repeat(32);
        let messages = vec![
            Message {
                id: 0,
                mine: false,
                who: "Carol".into(),
                text: "<script>x</script>".into(),
                delivery: Delivery::Sent,
                reason: None,
            },
            Message {
                id: 40,
                mine: true,
                who: "me".into(),
                text: "hi".into(),
                delivery: Delivery::Unsent,
                reason: Some("no session".into()),
            },
        ];
        let p = page(
            &s,
            &Place::Thread(peer.clone()),
            &Body::Thread(messages, true),
        );
        assert!(p.contains("&lt;script&gt;x") && !p.contains("<script>x"));
        assert!(p.contains("not sent: no session") && p.contains("class=\"said mine\""));
        assert!(
            p.contains(&format!("data-peer=\"{peer}\"")) && p.contains("Say something to Carol")
        );
        assert!(p.contains("Earlier messages are kept"));
        let r = refresh(&s, &Place::Thread(peer), &Body::Thread(vec![], false));
        assert!(r.contains("getElementById('main').innerHTML") && !r.contains("compose"));
    }

    #[test]
    fn a_subnet_lists_the_catalog_and_options_keep_the_connect_form() {
        let s = snap();
        let id = "22".repeat(32);
        let rows = vec![ResourceRow {
            service: "_files._tcp".into(),
            instance: "Shared drive".into(),
            node: "55".repeat(32),
            resource: "66".repeat(32),
            stale: true,
        }];
        let p = page(&s, &Place::Subnet(id), &Body::Catalog(rows));
        assert!(p.contains("Shared drive") && p.contains("stale") && p.contains("operator"));
        let all = page(
            &s,
            &Place::Resources,
            &Body::Catalog(vec![ResourceRow {
                service: "_board._tcp".into(),
                instance: "Notices".into(),
                node: "55".repeat(32),
                resource: "66".repeat(32),
                stale: false,
            }]),
        );
        assert!(all.contains("Notices") && all.contains("data-act=\"sweep\""));
        assert!(all.contains(&format!(
            "data-open=\"{}\" data-node=\"{}\" data-name=\"Notices\"",
            "66".repeat(32),
            "55".repeat(32)
        )));
        assert!(all.contains("data-go=\"resources\" class=\"here\""));
        let o = page(&s, &Place::Options, &Body::Plain);
        assert!(o.contains("id=\"connect\"") && o.contains(&"ab".repeat(32)));
        assert!(o.contains("data-act=\"launch\""));
        let h = page(&s, &Place::Home, &Body::Plain);
        assert!(h.contains("1 subnets, 2 people, 1 conversations"));
        assert!(h.contains("<h1>A device of Ann &lt;A&gt;</h1>") && h.contains(&"ab".repeat(32)));
        assert!(rail(&s, &Place::Home).contains("<b>Ann &lt;A&gt;</b>"));
        let unnamed = Snapshot {
            name: None,
            ..s.clone()
        };
        assert!(rail(&unnamed, &Place::Home).contains(&format!(
            "<b class=\"mono\">{}…</b>",
            short(&"ab".repeat(32))
        )));
        assert!(!h.contains("id=\"import\""), "the archive is in");
        assert!(
            o.contains("id=\"import\""),
            "a restored desktop can always open one"
        );
        let bare = Snapshot {
            subnets: vec![],
            ..s.clone()
        };
        assert!(page(&bare, &Place::Home, &Body::Plain).contains("not the archive"));
        let paired = Snapshot {
            device: Device::Paired {
                days: 90,
                dataset: None,
            },
            ..bare
        };
        assert!(!page(&paired, &Place::Home, &Body::Plain).contains("id=\"import\""));
        assert!(!page(&paired, &Place::Options, &Body::Plain).contains("id=\"import\""));
    }
}
