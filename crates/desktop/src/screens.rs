//! **The participant's screens, once the desktop is up**: a rail of orbs
//! — direct messages, every subnet this key is in — a foot with the
//! options and the nameplate, and the place itself beside them, as the
//! phone's shell lays them out (its `ShellActivity` and `Chrome`) [author,
//! 2026-11-23].  There is no meeting here: a desktop has no camera, and
//! somebody is met on a phone.
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
    /// The landing place, which the nameplate returns to.
    Home,
    /// Direct messages, which is also the people this key knows.
    Direct,
    /// One thread, with the other party's keyhash in hex.
    Thread(String),
    /// One subnet, by its id in hex: its launchers.
    Subnet(String),
    /// One subnet's options, reached from its name.
    SubnetOptions(String),
    /// This person's own options, reached from the foot.
    Options,
    /// The node serving this desktop: where it is connected, and a node
    /// to connect to, reached from the options as the phone's is.
    Node,
}

impl Place {
    /// A place as the page names it: `home`, `direct`, `options`, `node`,
    /// `thread:<hex>`, `subnet:<hex>`, `subnet-options:<hex>`.
    pub fn parse(s: &str) -> Option<Place> {
        let hex64 = |h: &str| {
            (h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_lowercase())
        };
        match s.split_once(':') {
            None => match s {
                "home" => Some(Place::Home),
                "direct" => Some(Place::Direct),
                "options" => Some(Place::Options),
                "node" => Some(Place::Node),
                _ => None,
            },
            Some(("thread", h)) => hex64(h).map(Place::Thread),
            Some(("subnet", h)) => hex64(h).map(Place::Subnet),
            Some(("subnet-options", h)) => hex64(h).map(Place::SubnetOptions),
            Some(_) => None,
        }
    }

    /// The name [`Place::parse`] reads back.
    pub fn token(&self) -> String {
        match self {
            Place::Home => "home".into(),
            Place::Direct => "direct".into(),
            Place::Options => "options".into(),
            Place::Node => "node".into(),
            Place::Thread(h) => format!("thread:{h}"),
            Place::Subnet(h) => format!("subnet:{h}"),
            Place::SubnetOptions(h) => format!("subnet-options:{h}"),
        }
    }

    /// **Whether the rail and the foot are drawn**, as the phone's shell
    /// decides it (its `Chrome.chrome`) [author, 2026-11-23: *"same
    /// navigation of options and subnet content"*]: home, a subnet and
    /// direct messages keep them; a thread takes the window, and the
    /// options, reached from the foot, cover it.
    pub fn chrome(&self) -> bool {
        matches!(self, Place::Home | Place::Direct | Place::Subnet(_))
    }

    /// **Where `‹` goes when nothing is behind**: the place this one is
    /// reached from.
    fn parent(&self) -> Place {
        match self {
            Place::Node => Place::Options,
            Place::SubnetOptions(h) => Place::Subnet(h.clone()),
            Place::Thread(_) => Place::Direct,
            _ => Place::Home,
        }
    }
}

/// **Where the person has been, and where back goes**, by the phone's
/// rules (its `History`) [author, 2026-11-20]: a visit to where you are
/// records nothing, a visit home empties it, a visit to a place already
/// held cuts back to it, and anything else is kept, newest last, the
/// oldest dropped beyond [`History::DEPTH`]. Back is the previous place,
/// and with none, the place this one is reached from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    here: Place,
    before: Vec<Place>,
}

impl Default for History {
    fn default() -> Self {
        History {
            here: Place::Home,
            before: Vec::new(),
        }
    }
}

impl History {
    /// How many places are kept behind the one shown.
    pub const DEPTH: usize = 32;

    /// The place being shown.
    pub fn here(&self) -> &Place {
        &self.here
    }

    /// Go to `to`, recording the visit as the type's comment says.
    pub fn go(&mut self, to: Place) {
        if to == self.here {
            return;
        }
        if to == Place::Home {
            self.before.clear();
        } else if let Some(at) = self.before.iter().position(|p| *p == to) {
            self.before.truncate(at);
        } else {
            self.before
                .push(std::mem::replace(&mut self.here, Place::Home));
            if self.before.len() > Self::DEPTH {
                self.before.remove(0);
            }
        }
        self.here = to;
    }

    /// Go back, and say where to.
    pub fn back(&mut self) -> &Place {
        self.here = self.before.pop().unwrap_or_else(|| self.here.parent());
        &self.here
    }
}

/// **What this person is in a subnet, drawn as the crown its orb sits
/// in** [author, 2026-10-27], the highest that applies: a full pointed
/// crown for its root, a golden circlet for a node run there, a silver
/// one for a subordinate held there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crown {
    /// None of the three.
    None,
    /// At least one subordinate.
    Silver,
    /// A node run there.
    Gold,
    /// Its root.
    Full,
}

impl Crown {
    /// What a subnet's three facts come to, root before node before
    /// subordinates.
    pub fn of(root: bool, operator: bool, subordinates: u32) -> Crown {
        if root {
            Crown::Full
        } else if operator {
            Crown::Gold
        } else if subordinates > 0 {
            Crown::Silver
        } else {
            Crown::None
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
    /// The crown its orb sits in.
    pub crown: Crown,
    /// **The node serving this key there**, in hex: the patron's, else this
    /// key's own where it is the subnet's root. What that node offers is
    /// what the subnet's page launches.
    pub node: String,
    /// **What waits to be seen there**, the orb's count: a move of its
    /// node to answer, and what its resources ask, rolled up.
    pub unseen: u32,
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

/// **One resource a subnet's page launches**, as its node offered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRow {
    /// The name to show it by.
    pub instance: String,
    /// The node holding it, in hex.
    pub node: String,
    /// The resource's own keyhash, in hex, which is what opens it.
    pub resource: String,
    /// What waits for the person there, its card's count: the resource's
    /// own count and its asks unanswered.
    pub unseen: u32,
    /// **Its icon** [author, 2026-11-23]: the format tag (1 PNG, 2 JPEG,
    /// 3 WebP) and the bytes, where it served one.
    pub icon: Option<(u64, Vec<u8>)>,
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
/// subnet's launchers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// The place needs nothing more.
    Plain,
    /// A thread's messages, oldest first, and whether earlier ones exist.
    Thread(Vec<Message>, bool),
    /// **What a subnet's node offers this key**, one launcher each
    /// [author, 2026-11-23]: swept at every connection and whenever the
    /// subnet is opened, never browsed.
    Launchers(Vec<ResourceRow>),
}

/// **A place drawn whole**: the page the window loads when it is gone to.
/// The phone's layout [author, 2026-11-23]: *"narrow side navigation wide
/// enough for orbs only (preserve as much space as possible for the main
/// content)"*, the foot beneath, and a place that hides them carrying its
/// own way back.
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
    let main = format!("<main id=\"main\">{}</main>", main(snap, place, body));
    shell(
        "FuerosNet",
        &if place.chrome() {
            format!(
                "<nav id=\"rail\">{}</nav>\n{main}\n<footer id=\"foot\">{}</footer>",
                rail(snap, place),
                foot(snap)
            )
        } else {
            format!("{main}\n{compose}")
        },
    )
}

/// **A place redrawn where it stands**: the rail, the foot and the body
/// replaced, anything typed beside them kept, and a thread scrolled to its
/// newest.
pub fn refresh(snap: &Snapshot, place: &Place, body: &Body) -> String {
    format!(
        "(() => {{ const put = (id, h) => {{ const e = document.getElementById(id); \
         if (e) e.innerHTML = h; }}; put('rail', {}); put('foot', {}); put('main', {}); }})();\
         if (document.getElementById('messages')) window.scrollTo(0, document.body.scrollHeight);",
        js(&rail(snap, place)),
        js(&foot(snap)),
        js(&main(snap, place, body))
    )
}

/// **The rail**: direct messages pinned, a divider, then an orb for every
/// subnet, each in its crown and wearing its count. No meeting: a desktop
/// has no camera, and somebody is met on a phone.
pub fn rail(snap: &Snapshot, place: &Place) -> String {
    let mut out = orb(
        &Place::Direct,
        matches!(place, Place::Direct | Place::Thread(_)),
        "DM",
        "Direct messages",
        Crown::None,
        0,
    );
    if !snap.subnets.is_empty() {
        out.push_str("<hr>\n");
    }
    for s in &snap.subnets {
        let p = Place::Subnet(s.id.clone());
        let standing = match s.crown {
            Crown::Full => ", you are its root",
            Crown::Gold => ", you run a node here",
            Crown::Silver => ", you have a subordinate here",
            Crown::None => "",
        };
        out.push_str(&orb(
            &p,
            *place == p,
            &mark(s.name.as_deref(), &s.id),
            &format!("{}{standing}{}", subnet_name(s), said(s.unseen)),
            s.crown,
            s.unseen,
        ));
    }
    out
}

/// One orb: the crown's back, the disc and its mark, the crown's front
/// sweep, and the count above its corner.
fn orb(to: &Place, here: bool, mark: &str, said: &str, crown: Crown, unseen: u32) -> String {
    format!(
        "<a class=\"orb{}\" data-go=\"{}\" title=\"{}\" aria-label=\"{}\">{}\
         <span class=\"disc\">{}</span>{}{}</a>\n",
        if here { " here" } else { "" },
        to.token(),
        escape(said),
        escape(said),
        band(crown, false),
        escape(mark),
        band(crown, true),
        count(unseen)
    )
}

/// **Half of a crown**, as the phone draws it (its `Crowns`), in the orb's
/// own box of a hundred: the band's back passes behind the orb, its front
/// sweep crosses the orb's foot, and only the full crown has points.
fn band(crown: Crown, front: bool) -> String {
    let (colour, width) = match crown {
        Crown::None => return String::new(),
        Crown::Silver => ("var(--rail-silver)", 2.3),
        Crown::Gold | Crown::Full => ("var(--rail-gold)", 3.0),
    };
    let bow = if front { 100.0 } else { 57.6 };
    let points = if front && crown == Crown::Full {
        [
            "20.3,79.2 28.6,75.7 37,87.4",
            "40.9,88.5 50,71.2 59.1,88.5",
            "63,87.4 71.4,75.7 79.7,79.2",
        ]
        .iter()
        .map(|p| format!("<polygon points=\"{p}\" fill=\"{colour}\"/>"))
        .collect()
    } else {
        String::new()
    };
    format!(
        "<svg class=\"{}\" viewBox=\"0 0 100 100\" aria-hidden=\"true\">{points}\
         <path d=\"M19.7 78.8 Q50 {bow} 80.3 78.8\" fill=\"none\" stroke=\"{colour}\" \
         stroke-width=\"{width}\" stroke-linecap=\"round\"/></svg>",
        if front { "front" } else { "behind" }
    )
}

/// **The foot**: the options orb, marked as this person, and the
/// nameplate, which is home.
pub fn foot(snap: &Snapshot) -> String {
    format!(
        "<a class=\"me\" data-go=\"options\" title=\"You, and this desktop's options\" \
         aria-label=\"You, and this desktop's options\">{}</a>\
         <a class=\"plate\" data-go=\"home\"><b{}>{}</b><small>home</small></a>",
        escape(&mark(snap.name.as_deref(), &hex(&snap.me))),
        if snap.name.is_some() {
            ""
        } else {
            " class=\"mono\""
        },
        escape(&snap.who()),
    )
}

/// **What an orb says when it has no picture**, as the phone's does: two
/// letters of the name, or four hex characters of the keyhash where no
/// name has reached this device.
pub fn mark(alias: Option<&str>, id: &str) -> String {
    let name = alias.unwrap_or("").trim();
    if name.is_empty() {
        return id.chars().take(4).collect();
    }
    let words: Vec<&str> = name.split_whitespace().collect();
    let initial = |w: &str| {
        w.chars()
            .next()
            .map(|c| c.to_uppercase().collect::<String>())
    };
    match words.as_slice() {
        [a, b, ..] => format!(
            "{}{}",
            initial(a).unwrap_or_default(),
            initial(b).unwrap_or_default()
        ),
        _ => {
            let mut c = name.chars();
            let first = c.next().map(|c| c.to_uppercase().collect::<String>());
            format!(
                "{}{}",
                first.unwrap_or_default(),
                c.next().map(String::from).unwrap_or_default()
            )
        }
    }
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
            Body::Launchers(rows) => subnet(snap, id, rows),
            _ => subnet(snap, id, &[]),
        },
        Place::SubnetOptions(id) => subnet_options(snap, id),
        Place::Options => options(snap),
        Place::Node => node(snap),
    }
}

/// **A place's title**: with `‹` where the place hides the chrome, since
/// the way out went with it, and going to `go` where the title leads on.
fn bar(title: &str, back: bool, go: Option<&Place>) -> String {
    let title = escape(title);
    format!(
        "<header class=\"bar\">{}<h1>{}</h1></header>\n",
        if back {
            "<button class=\"back\" data-act=\"back\" aria-label=\"Back\">‹</button>"
        } else {
            ""
        },
        match go {
            Some(p) => format!("<a data-go=\"{}\">{title}</a>", p.token()),
            None => title,
        }
    )
}

/// **One row that goes somewhere**, as the phone's options list them: a
/// label, and beneath it what that is.
fn row(label: &str, sub: &str, attrs: &str) -> String {
    format!(
        "<button class=\"row\" {attrs}><span>{}</span><small>{}</small></button>\n",
        escape(label),
        escape(sub)
    )
}

/// **Home**: what the phone's home asks of the person and nothing else —
/// here, the archive a desktop restored from the phrase has not got yet.
fn home(snap: &Snapshot) -> String {
    // **a restore brings back the key and not the archive**, and says both
    // (`light-client-requirements.md` §2): asked here until the archive is
    // in, which an empty one is the sign of
    if snap.device == Device::Restored && snap.subnets.is_empty() {
        BACKUP.to_string()
    } else {
        String::new()
    }
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
    let mut out = bar(&snap.name_of(peer), true, None);
    out.push_str(&format!("<p class=\"sub mono\">{peer}</p>\n"));
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

/// **A subnet's page**: its name, which leads to its options, and its
/// launchers.
fn subnet(snap: &Snapshot, id: &str, rows: &[ResourceRow]) -> String {
    let s = snap.subnets.iter().find(|s| s.id == id);
    format!(
        "{}{}",
        bar(
            &s.map(subnet_name).unwrap_or_else(|| short(id)),
            false,
            Some(&Place::SubnetOptions(id.to_string()))
        ),
        launchers(rows)
    )
}

/// **A subnet's options**: what this key is there, as the phone's say it.
fn subnet_options(snap: &Snapshot, id: &str) -> String {
    let s = snap.subnets.iter().find(|s| s.id == id);
    format!(
        "{}<p>{}</p>",
        bar(&s.map(subnet_name).unwrap_or_else(|| short(id)), true, None),
        escape(
            &s.map(|s| s.rank.clone())
                .unwrap_or_else(|| "Your standing here has not been read yet.".into())
        )
    )
}

/// **One card per resource**, which is what opens it: its icon, else its
/// mark, its name, and its count above its corner where something waits
/// [author, 2026-11-23].
fn launchers(rows: &[ResourceRow]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::from("<div class=\"cards\">\n");
    for r in rows {
        let face = match &r.icon {
            Some((format, bytes)) => format!(
                "<img src=\"data:{};base64,{}\" alt=\"\">",
                match format {
                    2 => "image/jpeg",
                    3 => "image/webp",
                    _ => "image/png",
                },
                base64(bytes)
            ),
            None => escape(&mark(Some(&r.instance), &r.resource)),
        };
        out.push_str(&format!(
            "<button class=\"card\" data-open=\"{}\" data-node=\"{}\" data-name=\"{}\">\
             <span class=\"icon\">{face}</span><span class=\"name\">{}</span>{}</button>\n",
            escape(&r.resource),
            escape(&r.node),
            escape(&r.instance),
            escape(&r.instance),
            count(r.unseen),
        ));
    }
    out.push_str("</div>");
    out
}

/// A count as an orb or a card wears it: nothing at none.
fn count(n: u32) -> String {
    match n {
        0 => String::new(),
        n if n > 99 => "<sup>99+</sup>".into(),
        n => format!("<sup>{n}</sup>"),
    }
}

/// A count as a label says it.
fn said(n: u32) -> String {
    match n {
        0 => String::new(),
        1 => ", 1 unseen".into(),
        n => format!(", {n} unseen"),
    }
}

/// **This person's options**, as the phone's lists them, less what only a
/// phone does — another device, a desktop, the identities it holds — and
/// with what this desktop is.
fn options(snap: &Snapshot) -> String {
    let (said, dataset) = match &snap.device {
        Device::Paired { days, dataset } => (
            format!(
                "Paired from the phone: this desktop presents its own key under a run of \
                 {days} days the phone signed. It reads everything and signs nothing."
            ),
            match dataset {
                Some(true) => {
                    "<p class=\"sub\">Everything the phone holds of the identity came across.</p>"
                }
                Some(false) => {
                    "<p class=\"sub\">Nothing else was handed over; pair again to bring it across.</p>"
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
    format!(
        "{}{}{}<h2>This desktop</h2>\n<p class=\"sub\">{}</p>\n{dataset}\n\
         <p class=\"sub mono\">{}</p>\n{}",
        bar("Options", true, None),
        row("Serving node", &snap.connection, "data-go=\"node\""),
        row(
            "Instances you run",
            "launch one at a provider",
            "data-act=\"launch\""
        ),
        escape(&said),
        hex(&snap.me),
        if snap.device == Device::Restored {
            BACKUP
        } else {
            ""
        }
    )
}

/// **The serving node**, as the phone's attach screen has it: where this
/// desktop is connected, and a node already running to connect to.
fn node(snap: &Snapshot) -> String {
    format!(
        "{}<p class=\"mono\">{}</p>\n\
         <h2>Connect to a node already running</h2>\n\
         <p class=\"sub\">Its key and the address it answers at. A node you launched \
         answers to your own key, which is filled in.</p>\n\
         <label>Node key<input id=\"node\" class=\"mono\" value=\"{}\" \
         autocomplete=\"off\" spellcheck=\"false\"></label>\n\
         <label>Address<input id=\"address\" class=\"mono\" \
         placeholder=\"203.0.113.7:7431\" autocomplete=\"off\" spellcheck=\"false\"></label>\n\
         <p><button id=\"connect\">Connect</button></p>",
        bar("Serving node", true, None),
        escape(&snap.connection),
        hex(&snap.me),
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

/// Standard base64 with padding, for an icon carried in a `data:` URL.
fn base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i <= c.len() {
                A[(n >> (18 - 6 * i)) as usize & 63] as char
            } else {
                '='
            });
        }
    }
    out
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
                rank: "Its operator.".into(),
                crown: Crown::Gold,
                node: "55".repeat(32),
                unseen: 3,
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
            Place::Node,
            Place::Thread("ab".repeat(32)),
            Place::Subnet("cd".repeat(32)),
            Place::SubnetOptions("ef".repeat(32)),
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
            "resources",
            "thread:ab",
            "subnet:",
            "erase:00",
            "thread:../../x",
        ] {
            assert_eq!(Place::parse(bad), None, "{bad}");
        }
    }

    // **the phone's navigation** [author, 2026-11-23]
    #[test]
    fn the_chrome_and_back_are_the_phones() {
        let id = "22".repeat(32);
        for p in [Place::Home, Place::Direct, Place::Subnet(id.clone())] {
            assert!(p.chrome(), "{p:?}");
        }
        for p in [
            Place::Options,
            Place::Node,
            Place::SubnetOptions(id.clone()),
            Place::Thread(id.clone()),
        ] {
            assert!(!p.chrome(), "{p:?}");
        }
        let mut h = History::default();
        h.go(Place::Subnet(id.clone()));
        h.go(Place::SubnetOptions(id.clone()));
        assert_eq!(h.back(), &Place::Subnet(id.clone()));
        assert_eq!(h.back(), &Place::Home);
        // nothing behind: back is where the place is reached from
        h.go(Place::Home);
        let mut fresh = History {
            here: Place::Node,
            before: vec![],
        };
        assert_eq!(fresh.back(), &Place::Options);
        // home empties it, and a place held is cut back to
        h.go(Place::Options);
        h.go(Place::Node);
        h.go(Place::Options);
        assert_eq!(h.back(), &Place::Home);
        h.go(Place::Direct);
        h.go(Place::Home);
        assert_eq!(h.back(), &Place::Home);
        h.go(Place::Direct);
        h.go(Place::Direct);
        assert_eq!(h.back(), &Place::Home, "a redraw is not a visit");
        for i in 0..(History::DEPTH + 5) {
            h.go(Place::Thread(format!("{i:064}")));
        }
        assert_eq!(h.before.len(), History::DEPTH);
    }

    #[test]
    fn the_rail_is_orbs_only_and_the_foot_carries_the_options_and_home() {
        let s = snap();
        let id = "22".repeat(32);
        let r = rail(&s, &Place::Subnet(id.clone()));
        // **orbs only, as narrow as one** [author, 2026-11-23]
        assert!(r.contains(&format!("class=\"orb here\" data-go=\"subnet:{id}\"")));
        assert!(
            r.contains("<span class=\"disc\">M&lt;</span>"),
            "the mark, not the name"
        );
        assert!(r.contains("aria-label=\"Mirror &lt;Manse&gt;, you run a node here, 3 unseen\""));
        assert!(r.contains("<sup>3</sup>") && r.contains("var(--rail-gold)"));
        assert!(!r.contains("<polygon"), "a circlet has no points");
        assert!(r.contains("data-go=\"direct\"") && !r.contains("meet"));
        assert!(!r.contains("options") && !r.contains("Options"));
        assert!(
            rail(&s, &Place::Thread("33".repeat(32)))
                .contains("class=\"orb here\" data-go=\"direct\"")
        );
        let f = foot(&s);
        assert!(f.contains("data-go=\"options\"") && f.contains(">A&lt;</a>"));
        assert!(f.contains("data-go=\"home\"><b>Ann &lt;A&gt;</b><small>home</small>"));
        let unnamed = Snapshot {
            name: None,
            ..s.clone()
        };
        assert!(foot(&unnamed).contains(&format!(
            "<b class=\"mono\">{}…</b>",
            short(&"ab".repeat(32))
        )));
        assert!(foot(&unnamed).contains(">abab</a>"));
        assert_eq!(band(Crown::None, true), "");
        assert_eq!(band(Crown::Full, true).matches("<polygon").count(), 3);
        assert!(!band(Crown::Full, false).contains("<polygon"));
        assert_eq!(Crown::of(true, true, 3), Crown::Full);
        assert_eq!(Crown::of(false, true, 3), Crown::Gold);
        assert_eq!(Crown::of(false, false, 1), Crown::Silver);
        assert_eq!(Crown::of(false, false, 0), Crown::None);
        assert_eq!(mark(Some("Mirror's Manse"), "x"), "MM");
        assert_eq!(mark(Some("testapp"), "x"), "Te");
        assert_eq!(mark(Some(" "), "abcdef"), "abcd");
        assert_eq!(mark(None, "abcdef"), "abcd");
    }

    #[test]
    fn a_place_with_no_chrome_carries_its_own_way_back() {
        let s = snap();
        let o = page(&s, &Place::Options, &Body::Plain);
        assert!(!o.contains("id=\"rail\"") && !o.contains("id=\"foot\""));
        assert!(o.contains("data-act=\"back\""));
        let h = page(&s, &Place::Home, &Body::Plain);
        assert!(h.contains("id=\"rail\"") && h.contains("id=\"foot\""));
        assert!(!h.contains("data-act=\"back\""));
        let r = refresh(&s, &Place::Options, &Body::Plain);
        assert!(
            r.contains("put('rail'") && r.contains("if (e)"),
            "a rail that is not there is left"
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
        assert!(p.contains("Earlier messages are kept") && p.contains("data-act=\"back\""));
        let r = refresh(&s, &Place::Thread(peer), &Body::Thread(vec![], false));
        assert!(r.contains("put('main'") && !r.contains("compose"));
    }

    #[test]
    fn a_subnet_launches_what_its_node_offers_each_card_wearing_its_icon() {
        let s = snap();
        let id = "22".repeat(32);
        let rows = vec![
            ResourceRow {
                instance: "Shared drive".into(),
                node: "55".repeat(32),
                resource: "66".repeat(32),
                unseen: 0,
                icon: None,
            },
            ResourceRow {
                instance: "Notices".into(),
                node: "55".repeat(32),
                resource: "77".repeat(32),
                unseen: 2,
                icon: Some((1, b"\x89PNG".to_vec())),
            },
        ];
        let p = page(&s, &Place::Subnet(id.clone()), &Body::Launchers(rows));
        // **a card each, which is what opens it** [author, 2026-11-23], and
        // **badged with the icon the resource serves**
        assert!(p.contains(&format!(
            "<button class=\"card\" data-open=\"{}\" data-node=\"{}\" data-name=\"Notices\">\
             <span class=\"icon\"><img src=\"data:image/png;base64,iVBORw==\" alt=\"\"></span>\
             <span class=\"name\">Notices</span><sup>2</sup></button>",
            "77".repeat(32),
            "55".repeat(32)
        )));
        assert!(
            p.contains(
                "<span class=\"icon\">SD</span><span class=\"name\">Shared drive</span></button>"
            ),
            "its mark where it served none, and no count where nothing waits"
        );
        // the name leads to the subnet's options, which say the rank
        assert!(p.contains(&format!(
            "<a data-go=\"subnet-options:{id}\">Mirror &lt;Manse&gt;</a>"
        )));
        assert!(!p.contains("Its operator."));
        assert!(page(&s, &Place::SubnetOptions(id), &Body::Plain).contains("Its operator."));
        // and the words of the abstraction it replaced are nowhere
        for gone in ["catalog", "Catalog", "Resources", "sweep", "Sweep"] {
            assert!(!p.contains(gone), "{gone} on the page");
        }
    }

    #[test]
    fn the_options_lead_to_the_serving_node_and_the_archive_where_it_is_owed() {
        let s = snap();
        let o = page(&s, &Place::Options, &Body::Plain);
        assert!(o.contains("data-go=\"node\"><span>Serving node</span><small>Connected to 1111"));
        assert!(o.contains("data-act=\"launch\"") && o.contains(&"ab".repeat(32)));
        assert!(o.contains("Restored from the phrase"));
        assert!(
            !o.contains("id=\"connect\""),
            "the form is the serving node's"
        );
        let n = page(&s, &Place::Node, &Body::Plain);
        assert!(
            n.contains("id=\"connect\"") && n.contains(&format!("value=\"{}\"", "ab".repeat(32)))
        );
        let h = page(&s, &Place::Home, &Body::Plain);
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
                dataset: Some(true),
            },
            ..bare
        };
        assert!(!page(&paired, &Place::Home, &Body::Plain).contains("id=\"import\""));
        let po = page(&paired, &Place::Options, &Body::Plain);
        assert!(
            !po.contains("id=\"import\"") && po.contains("90 days") && po.contains("came across")
        );
    }

    #[test]
    fn base64_is_the_standard_alphabet_padded() {
        for (i, o) in [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"\xff\xfe\xfd", "//79"),
        ] {
            assert_eq!(base64(i), o);
        }
    }
}
