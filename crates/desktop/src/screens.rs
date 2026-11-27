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
use crate::threads::{Chosen, Delivery, Entry, Message};

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
    /// **The nodes this identity runs** [author, 2026-11-26], reached from
    /// the options: each administered from here on a restored desktop, and
    /// another launched at a provider.
    Instances,
    /// **Who this key is under, and what leaving costs**, reached from a
    /// subnet's options: every line the key holds, not that subnet's alone.
    Authority,
    /// The colour of the identity in focus, chosen from the palette.
    Colour,
    /// **How much each subnet's board keeps here**, chosen from the
    /// bounds offered (`light-client-requirements.md` §4.5).
    Pool,
    /// **Which identity is in focus** [author, 2026-11-23], opened by a
    /// double click on the nameplate: the one a direct message starts as.
    Identities,
    /// **One party's options** [author, 2026-11-23], by their keyhash in
    /// hex, opened by a double click on their orb: their colour, mute,
    /// notifications, block.
    Person(String),
    /// **One thread of a subnet's board** [author, 2026-10-27: a thread
    /// takes the whole screen], by the subnet's id and the thread's, both
    /// in hex.
    Board(String, String),
    /// A thread being begun on a subnet's board, by the subnet's id in hex.
    BoardNew(String),
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
                "instances" => Some(Place::Instances),
                "authority" => Some(Place::Authority),
                "colour" => Some(Place::Colour),
                "pool" => Some(Place::Pool),
                "identities" => Some(Place::Identities),
                _ => None,
            },
            Some(("thread", h)) => hex64(h).map(Place::Thread),
            Some(("subnet", h)) => hex64(h).map(Place::Subnet),
            Some(("subnet-options", h)) => hex64(h).map(Place::SubnetOptions),
            Some(("person", h)) => hex64(h).map(Place::Person),
            Some(("board-new", h)) => hex64(h).map(Place::BoardNew),
            Some(("board", both)) => {
                let (subnet, thread) = both.split_once(':')?;
                Some(Place::Board(hex64(subnet)?, hex64(thread)?))
            }
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
            Place::Instances => "instances".into(),
            Place::Authority => "authority".into(),
            Place::Colour => "colour".into(),
            Place::Pool => "pool".into(),
            Place::Identities => "identities".into(),
            Place::Thread(h) => format!("thread:{h}"),
            Place::Subnet(h) => format!("subnet:{h}"),
            Place::SubnetOptions(h) => format!("subnet-options:{h}"),
            Place::Person(h) => format!("person:{h}"),
            Place::Board(s, t) => format!("board:{s}:{t}"),
            Place::BoardNew(s) => format!("board-new:{s}"),
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
            Place::Node | Place::Instances | Place::Colour | Place::Pool => Place::Options,
            Place::SubnetOptions(h) | Place::Board(h, _) | Place::BoardNew(h) => {
                Place::Subnet(h.clone())
            }
            Place::Thread(_) | Place::Person(_) => Place::Direct,
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
    /// **The identity holding it**, in hex: what acts there acts as this
    /// one [author, 2026-11-07].
    pub by: String,
    /// That identity's colour, which its orb wears [author, 2026-11-12].
    pub colour: String,
    /// **Whether this key runs a node here**: it published an endpoint
    /// record on the line it holds, which is what an operator is.
    pub operates: bool,
}

/// One person this key knows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Person {
    /// Their keyhash, in hex.
    pub key: String,
    /// The alias they published, where one has been received.
    pub alias: Option<String>,
    /// How they are known, in words.
    pub how: String,
    /// The identity that met them, in hex.
    pub by: String,
    /// **The picture their profile carries**: the format tag (1 PNG, 2
    /// JPEG, 3 WebP) and the bytes, where one has been received.
    pub picture: Option<(u64, Vec<u8>)>,
    /// **The subnet the two share**, by the name it goes by, where there is
    /// one [author, 2026-11-23].
    pub subnet: Option<String>,
    /// Where they stand from here in it, in levels: above positive.
    pub offset: Option<i32>,
    /// Whether they are in this key's direct line there.
    pub in_line: bool,
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
    /// **The colour of the identity in focus**, by its palette name: what
    /// the foot wears [author, 2026-11-12].
    pub colour: String,
    /// **The bound each subnet's board is held under here** for the
    /// identity in focus, in bytes (`light-client-requirements.md` §4.5).
    pub pool: u64,
    /// **Every identity this desktop holds** [author, 2026-11-23], the one
    /// in focus among them.
    pub held: Vec<Holder>,
    /// Where it is connected, in one line.
    pub connection: String,
    /// Every subnet this key is in.
    pub subnets: Vec<SubnetRow>,
    /// The people this key knows, nodes left out.
    pub people: Vec<Person>,
    /// The threads the index names.
    pub threads: Vec<Entry>,
    /// **What each identity chose about each party** [author, 2026-11-23],
    /// by the identity and the party, both in hex; a party absent chose
    /// nothing and has nothing unread.
    pub chosen: std::collections::BTreeMap<(String, String), Chosen>,
    /// **The last line said in each thread**, by the identity and the
    /// party, both in hex: whether it was this identity's, and its text.
    pub last: std::collections::BTreeMap<(String, String), (bool, String)>,
}

impl Snapshot {
    /// What `by` chose about `peer`.
    pub fn chosen_of(&self, by: &str, peer: &str) -> Chosen {
        self.chosen
            .get(&(by.to_string(), peer.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    /// **The direct-message orb's count** [author, 2026-11-23]: what
    /// arrived unseen on every thread neither muted nor blocked.
    pub fn direct_unseen(&self) -> u32 {
        self.chosen
            .values()
            .filter(|c| !c.muted && !c.blocked)
            .map(|c| c.unread)
            .sum()
    }

    /// **The identity a conversation with `peer` is held as**: the one
    /// whose thread it is, else the one that met them, else the one in
    /// focus, as the window's own choice of it goes.
    pub fn holder_of(&self, peer: &str) -> String {
        self.threads
            .iter()
            .find(|t| t.peer == peer)
            .map(|t| t.by.clone())
            .or_else(|| {
                self.people
                    .iter()
                    .find(|p| p.key == peer)
                    .map(|p| p.by.clone())
            })
            .unwrap_or_else(|| hex(&self.me))
    }

    /// The person `by` knows as `peer`, where they are one.
    pub fn person(&self, by: &str, peer: &str) -> Option<&Person> {
        self.people.iter().find(|p| p.by == by && p.key == peer)
    }

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
    /// subnet is opened, never browsed. **And its board beside them**
    /// [author, 2026-10-27]: the threads this key's pool holds.
    Launchers(Vec<ResourceRow>, BoardList),
    /// **One thread of a subnet's board**: its title where the post that
    /// began it is held, its posts oldest first, and the time they are
    /// read at.
    Board {
        /// The thread's title.
        title: Option<String>,
        /// Its posts, oldest first.
        posts: Vec<rhtn_model::BoardPost>,
        /// Now, in seconds: what a post's age is said against.
        now: u64,
    },
    /// **The lines this key holds**, each with its patron's alias where one
    /// is held.
    Authority(Vec<LineRow>),
}

/// **A subnet's board as its page lists it**: the threads held, the one
/// with the newest post first, and the time they are read at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardList {
    /// The threads.
    pub threads: Vec<rhtn_model::BoardThread>,
    /// Now, in seconds.
    pub now: u64,
}

/// One line of authority this key holds, as the authority screen rolls it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRow {
    /// The line, as the kernel holds it.
    pub line: rhtn_model::Line,
    /// The patron's alias, where one has been received.
    pub alias: Option<String>,
    /// The identity holding the line, in hex, which is the one that leaves.
    pub by: String,
}

/// **One identity this desktop holds**, as the picker lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    /// Its keyhash, in hex.
    pub by: String,
    /// What it is called, else none and its key stands in.
    pub name: Option<String>,
    /// Its colour.
    pub colour: String,
}

/// **The identity colours** [author, 2026-11-12], by the names the phone
/// offers, each with the token it is drawn in (`ui/tokens.css`).
pub const PALETTE: [(&str, &str); 8] = [
    ("Red", "--identity-red"),
    ("Green", "--identity-green"),
    ("Purple", "--identity-purple"),
    ("Blue", "--identity-blue"),
    ("Orange", "--identity-orange"),
    ("Teal", "--identity-teal"),
    ("Pink", "--identity-pink"),
    ("Slate", "--identity-slate"),
];

/// **A person's colour** [author, 2026-11-23: *"in a characteristic
/// color"*], by the palette's name: the one chosen for them, else the one
/// their key gives them — its first byte, over the palette, as the phone's
/// `Chrome.personColour` takes it.
pub fn person_colour<'a>(chosen: Option<&'a str>, peer: &str) -> &'a str {
    if let Some(c) = chosen.filter(|c| PALETTE.iter().any(|(n, _)| n == c)) {
        return c;
    }
    let first = peer
        .get(..2)
        .and_then(|b| u8::from_str_radix(b, 16).ok())
        .unwrap_or(0);
    PALETTE[usize::from(first) % PALETTE.len()].0
}

/// **How much of a person's colour their bubbles wear** over the ground,
/// in percent: a low-saturation version of it, as the phone's
/// `Chrome.BUBBLE_SHARE`.
pub const BUBBLE_SHARE: u32 = 28;

/// The token a palette name is drawn in; the orb's own where the name is
/// none of the palette's.
pub fn token(name: &str) -> &'static str {
    PALETTE
        .iter()
        .find(|(n, _)| *n == name)
        .map_or("--rail-orb", |(_, t)| t)
}

/// **A place drawn whole**: the page the window loads when it is gone to.
/// The phone's layout [author, 2026-11-23]: *"narrow side navigation wide
/// enough for orbs only (preserve as much space as possible for the main
/// content)"*, the foot beneath, and a place that hides them carrying its
/// own way back.
pub fn page(snap: &Snapshot, place: &Place, body: &Body) -> String {
    let compose = match place {
        // **not answered once blocked**, nor where the sender could not be
        // attributed: a reply goes only to a contact [author, 2026-11-20]
        Place::Thread(peer) if snap.chosen_of(&snap.holder_of(peer), peer).blocked => String::new(),
        Place::Thread(peer) if snap.chosen_of(&snap.holder_of(peer), peer).unverified => {
            "<p class=\"sub\">Unverified: this sender could not be attributed, so it cannot be \
             answered from here.</p>"
                .to_string()
        }
        Place::Thread(peer) => format!(
            "<div id=\"compose\"><textarea id=\"text\" rows=\"3\" data-peer=\"{peer}\" \
             placeholder=\"Say something to {}\"></textarea>\
             <p><button id=\"send\">Send</button></p></div>",
            escape(&snap.name_of(peer))
        ),
        // **a reply states its thread's reach and offers none** [author,
        // 2026-10-27]: read from where this key stands, as each post's is
        Place::Board(subnet, thread) => {
            let reach = match body {
                Body::Board { posts, .. } => posts.first().map(|p| reach(p.up, p.down)),
                _ => None,
            };
            format!(
                "<div id=\"compose\"><textarea id=\"board-reply\" rows=\"3\" \
                 data-subnet=\"{subnet}\" data-thread=\"{thread}\" \
                 placeholder=\"Reply to the thread\"></textarea>\
                 <p><button id=\"board-send\">Reply</button> <small class=\"sub\">{}</small></p></div>",
                reach.map_or(String::new(), |r| format!(
                    "It travels as the thread does: {r}"
                ))
            )
        }
        // **a new thread says how far it travels**, `+2/-2` until changed
        // [author, 2026-10-27], a level at a time either way
        Place::BoardNew(subnet) => format!(
            "<div id=\"compose\">\
             <p><input id=\"board-title\" maxlength=\"128\" placeholder=\"Title\"></p>\
             <textarea id=\"board-text\" rows=\"6\" placeholder=\"What you want to say\"></textarea>\
             <p><label>Levels up <input id=\"board-up\" type=\"number\" min=\"0\" max=\"24\" \
             value=\"2\"></label> <label>Levels down <input id=\"board-down\" type=\"number\" \
             min=\"0\" max=\"24\" value=\"2\"></label></p>\
             <p><button id=\"board-post\" data-subnet=\"{subnet}\">Post</button></p></div>"
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
         newest();",
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
        &format!("Direct messages{}", said(snap.direct_unseen())),
        Crown::None,
        // **unread direct messages count on its orb** [author, 2026-11-23]
        snap.direct_unseen(),
        None,
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
            // **a subnet's orb wears the colour of the identity holding it**
            // [author, 2026-11-12]
            Some(&s.colour),
        ));
    }
    out
}

/// One orb: the crown's back, the disc in its colour with its mark, the
/// crown's front sweep, and the count above its corner.
#[allow(clippy::too_many_arguments)]
fn orb(
    to: &Place,
    here: bool,
    mark: &str,
    said: &str,
    crown: Crown,
    unseen: u32,
    colour: Option<&str>,
) -> String {
    format!(
        "<a class=\"orb{}\" data-go=\"{}\" title=\"{}\" aria-label=\"{}\">{}\
         <span class=\"disc\"{}>{}</span>{}{}</a>\n",
        if here { " here" } else { "" },
        to.token(),
        escape(said),
        escape(said),
        band(crown, false),
        colour
            .map(|c| format!(" style=\"background: var({})\"", token(c)))
            .unwrap_or_default(),
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
    // **the options orb and the nameplate wear the colour of the identity
    // in focus** [author, 2026-11-12]
    let wears = format!(" style=\"background: var({})\"", token(&snap.colour));
    format!(
        "<a class=\"me\" data-go=\"options\"{wears} title=\"You, and this desktop's options\" \
         aria-label=\"You, and this desktop's options\">{}</a>\
         <a class=\"plate\" data-go=\"home\"{wears}><b{}>{}</b><small>home</small></a>",
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
            Body::Launchers(rows, board) => subnet(snap, id, rows, board),
            _ => subnet(snap, id, &[], &BoardList::default()),
        },
        Place::Board(subnet, _) => match body {
            Body::Board { title, posts, now } => {
                board_thread(snap, subnet, title.as_deref(), posts, *now)
            }
            _ => board_thread(snap, subnet, None, &[], 0),
        },
        Place::BoardNew(subnet) => board_new(snap, subnet),
        Place::SubnetOptions(id) => subnet_options(snap, id),
        Place::Options => options(snap),
        Place::Node => node(snap),
        Place::Instances => instances(snap),
        Place::Colour => colour(snap),
        Place::Pool => pool(snap),
        Place::Identities => identities(snap),
        Place::Person(peer) => person(snap, peer),
        Place::Authority => match body {
            Body::Authority(lines) => authority(lines),
            _ => authority(&[]),
        },
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
        out.push_str("<h2>Conversations</h2>\n<div class=\"talks\">\n");
        // **a blocked party's card goes last**, where it is unblocked from
        let (open, blocked): (Vec<&Entry>, Vec<&Entry>) = snap
            .threads
            .iter()
            .partition(|t| !snap.chosen_of(&t.by, &t.peer).blocked);
        for t in open.into_iter().chain(blocked) {
            out.push_str(&card(snap, &t.by, &t.peer, true));
        }
        out.push_str("</div>\n");
    }
    let quiet: Vec<&Person> = snap
        .people
        .iter()
        .filter(|p| !snap.threads.iter().any(|t| t.peer == p.key))
        .collect();
    if !quiet.is_empty() {
        out.push_str("<h2>People</h2>\n<div class=\"talks\">\n");
        for p in quiet {
            out.push_str(&card(snap, &p.by, &p.key, false));
        }
        out.push_str("</div>\n");
    }
    out
}

/// **A person's orb** [author, 2026-11-23]: the picture their profile
/// carries, else their initial on their colour; a double click opens their
/// options, where the phone presses and holds.
fn person_orb(snap: &Snapshot, by: &str, peer: &str) -> String {
    let name = snap.name_of(peer);
    let colour = token(person_colour(
        snap.chosen_of(by, peer).colour.as_deref(),
        peer,
    ));
    let inner = match snap.person(by, peer).and_then(|p| p.picture.as_ref()) {
        Some((format, bytes)) => format!(
            "<img alt=\"\" src=\"data:{};base64,{}\">",
            match format {
                2 => "image/jpeg",
                3 => "image/webp",
                _ => "image/png",
            },
            base64(bytes)
        ),
        None => escape(
            &name
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect::<String>())
                .unwrap_or_default(),
        ),
    };
    format!(
        "<span class=\"who\" data-person=\"{peer}\" style=\"background: var({colour})\" \
         title=\"{} — double-click for their options\">{inner}</span>",
        escape(&name)
    )
}

/// **One card the width of the page** [author, 2026-11-23]: the person's
/// orb; their name in bold, the subnet the two share and where they stand
/// from here in it, in bold where they are in this key's direct line; what
/// arrived unseen; and what was muted. Edged in the colour of the identity
/// it is held as [author, 2026-11-12].
fn card(snap: &Snapshot, by: &str, peer: &str, thread: bool) -> String {
    let c = snap.chosen_of(by, peer);
    let p = snap.person(by, peer);
    let place = p
        .and_then(|p| p.subnet.as_ref().map(|s| (p, s)))
        .map(|(p, s)| {
            let at = match p.offset {
                Some(n) if n > 0 => format!(" +{n}"),
                Some(n) => format!(" {n}"),
                None => String::new(),
            };
            format!(
                " <span class=\"place\">{}{}</span>",
                escape(s),
                if p.in_line && !at.is_empty() {
                    format!(" <b>{}</b>", at.trim())
                } else {
                    at
                }
            )
        })
        .unwrap_or_default();
    // **the last line said**, then what was muted about them, as the
    // phone's card has it
    let said = snap.last.get(&(by.to_string(), peer.to_string()));
    let line = if c.blocked {
        "blocked".to_string()
    } else if !thread {
        "nothing said yet".to_string()
    } else {
        match said {
            None => "no messages".to_string(),
            Some((true, text)) => format!("me: {text}"),
            Some((false, text)) => text.clone(),
        }
    };
    let quiet = if c.blocked {
        ""
    } else if c.muted {
        " · muted"
    } else if c.silent {
        " · notifications muted"
    } else {
        ""
    };
    let note: String = format!("{line}{quiet}").chars().take(160).collect();
    let edge = snap
        .held
        .iter()
        .find(|h| h.by == by)
        .map(|h| token(&h.colour))
        .unwrap_or("--rule");
    format!(
        "<a class=\"talk{}\" data-go=\"thread:{peer}\" style=\"border-color: var({edge})\">{}\
         <span class=\"lines\"><span><b>{}</b>{place}</span><span><small>{}</small></span></span>{}</a>\n",
        if c.blocked { " blocked" } else { "" },
        person_orb(snap, by, peer),
        escape(&snap.name_of(peer)),
        escape(&note),
        if c.unread > 0 {
            format!(
                "<sup{}>{}</sup>",
                if c.muted { " class=\"muted\"" } else { "" },
                c.unread
            )
        } else {
            String::new()
        }
    )
}

/// **One party's options** [author, 2026-11-23]: their colour, chosen from
/// the palette or left to the one their key gives them, and mute, mute
/// notifications and block, each a toggle saying what it will do.
fn person(snap: &Snapshot, peer: &str) -> String {
    let by = snap.holder_of(peer);
    let c = snap.chosen_of(&by, peer);
    let mut out = bar(&snap.name_of(peer), true, None);
    out.push_str(&format!(
        "<p class=\"sub mono\">{peer}</p>\n<h2>Colour</h2>\n"
    ));
    let given = person_colour(None, peer);
    out.push_str(&format!(
        "<button class=\"row\" data-person-set=\"colour\" data-peer=\"{peer}\" data-tint=\"\">\
         <span><i class=\"swatch\" style=\"background: var({})\"></i>{}The one their key gives \
         them ({given})</span></button>\n",
        token(given),
        if c.colour.is_none() { "● " } else { "" }
    ));
    for (name, t) in PALETTE {
        out.push_str(&format!(
            "<button class=\"row\" data-person-set=\"colour\" data-peer=\"{peer}\" \
             data-tint=\"{name}\"><span><i class=\"swatch\" style=\"background: var({t})\"></i>\
             {}{name}</span></button>\n",
            if c.colour.as_deref() == Some(name) {
                "● "
            } else {
                ""
            }
        ));
    }
    out.push_str("<h2>Options</h2>\n");
    for (set, on, off, now) in [
        ("muted", "Unmute", "Mute", c.muted),
        (
            "silent",
            "Unmute notifications",
            "Mute notifications",
            c.silent,
        ),
        ("blocked", "Unblock", "Block", c.blocked),
    ] {
        out.push_str(&format!(
            "<button class=\"row\" data-person-set=\"{set}\" data-peer=\"{peer}\"><span>{}</span>\
             </button>\n",
            if now { on } else { off }
        ));
    }
    out
}

fn thread(snap: &Snapshot, peer: &str, messages: &[Message], more: bool) -> String {
    let by = snap.holder_of(peer);
    let c = snap.chosen_of(&by, peer);
    // **as a chat is laid out** [author, 2026-11-23]: this person's
    // bubbles to the right, grey and outlined in the colour of the identity
    // they speak as; the other party's to the left, on a low-saturation
    // version of that party's colour
    let mine = snap
        .held
        .iter()
        .find(|h| h.by == by)
        .map_or("--rule", |h| token(&h.colour));
    let theirs = token(person_colour(c.colour.as_deref(), peer));
    let mut out = format!(
        "<header class=\"bar\"><button class=\"back\" data-act=\"back\" aria-label=\"Back\">‹\
         </button>{}<h1>{}</h1></header>\n",
        person_orb(snap, &by, peer),
        escape(&snap.name_of(peer))
    );
    out.push_str(&format!("<p class=\"sub mono\">{peer}</p>\n"));
    if c.blocked {
        out.push_str(
            "<p class=\"sub\">Blocked: what they send is dropped as it comes. Double-click their \
             orb to unblock.</p>\n",
        );
    }
    if more {
        out.push_str(&format!(
            "<p><button class=\"row\" data-earlier=\"{}\"><span>Earlier messages</span></button></p>\n",
            escape(peer)
        ));
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
            (Delivery::Sent, _) if m.mine => " <small>sent</small>".to_string(),
            (Delivery::Sent, _) => String::new(),
        };
        let style = if m.mine {
            format!("border-color: var({mine})")
        } else {
            format!("background: color-mix(in srgb, var({theirs}) {BUBBLE_SHARE}%, var(--ground))")
        };
        out.push_str(&format!(
            "<div class=\"said{}\" style=\"{style}\"><p>{}</p>{state}</div>\n",
            if m.mine { " mine" } else { "" },
            escape(&m.text)
        ));
    }
    out.push_str("</div>");
    out
}

/// **A subnet's page**: its name, which leads to its options, and its
/// launchers.
fn subnet(snap: &Snapshot, id: &str, rows: &[ResourceRow], board: &BoardList) -> String {
    let s = snap.subnets.iter().find(|s| s.id == id);
    format!(
        "{}{}{}",
        bar(
            &s.map(subnet_name).unwrap_or_else(|| short(id)),
            false,
            Some(&Place::SubnetOptions(id.to_string()))
        ),
        launchers(rows, s.map_or("", |s| s.by.as_str())),
        board_list(snap, id, board)
    )
}

/// A reach as a post says it: `+2/-2`.
fn reach(up: u32, down: u32) -> String {
    format!("+{up}/-{down}")
}

/// How long ago `then` was, said shortly.
fn ago(now: u64, then: u64) -> String {
    match now.saturating_sub(then) {
        s if s < 90 => "just now".into(),
        s if s < 5400 => format!("{} min ago", s.div_ceil(60)),
        s if s < 129_600 => format!("{} h ago", s / 3600),
        s => format!("{} d ago", s / 86_400),
    }
}

/// **A subnet's board on its page** [author, 2026-10-27: beside the
/// launchers, a single board]: each thread a row leading to it, the one
/// with the newest post first, wearing what is unread; and the way to
/// begin one.
fn board_list(snap: &Snapshot, subnet: &str, board: &BoardList) -> String {
    let mut out = format!(
        "<h2>Board</h2>\n<p><a class=\"row\" data-go=\"{}\">New thread</a></p>\n",
        Place::BoardNew(subnet.to_string()).token()
    );
    if board.threads.is_empty() {
        out.push_str("<p class=\"sub\">Nothing posted yet that reaches you here.</p>\n");
        return out;
    }
    out.push_str("<div class=\"talks\">\n");
    for t in &board.threads {
        let by = t
            .began_by
            .map(|k| format!("began by {}, ", escape(&snap.name_of(&hex(&k)))))
            .unwrap_or_default();
        out.push_str(&format!(
            "<a class=\"talk\" data-go=\"{}\"><span class=\"lines\"><span><b>{}</b>{}</span>\
             <small>{by}{} post{}, {}</small></span></a>\n",
            Place::Board(subnet.to_string(), hex(&t.id)).token(),
            escape(
                t.title
                    .as_deref()
                    .unwrap_or("A thread begun out of your reach")
            ),
            count(t.unread),
            t.posts,
            if t.posts == 1 { "" } else { "s" },
            ago(board.now, t.latest)
        ));
    }
    out.push_str("</div>");
    out
}

/// **One thread of the board, taking the window**: its posts as a chat's
/// are laid out, this key's to the right in the colour it speaks as, each
/// saying who wrote it, how far it travels as read from here, and when.
fn board_thread(
    snap: &Snapshot,
    subnet: &str,
    title: Option<&str>,
    posts: &[rhtn_model::BoardPost],
    now: u64,
) -> String {
    let s = snap.subnets.iter().find(|s| s.id == subnet);
    let mine = s.map_or("--rule", |s| token(&s.colour));
    let mut out = format!(
        "<header class=\"bar\"><button class=\"back\" data-act=\"back\" aria-label=\"Back\">‹\
         </button><h1>{}</h1></header>\n<p class=\"sub\">{}</p>\n<div id=\"messages\">\n",
        escape(title.unwrap_or("A thread begun out of your reach")),
        escape(&s.map(subnet_name).unwrap_or_else(|| short(subnet)))
    );
    if posts.is_empty() {
        out.push_str("<p class=\"sub\">Nothing of this thread is held.</p>\n");
    }
    for p in posts {
        let writer = hex(&p.writer);
        // the name this key holds for them, else the one the post carried
        let known = snap.name_of(&writer);
        let name = match &p.alias {
            Some(a) if known == short(&writer) => a.clone(),
            _ => known,
        };
        let style = if p.mine {
            format!("border-color: var({mine})")
        } else {
            format!(
                "background: color-mix(in srgb, var({}) {BUBBLE_SHARE}%, var(--ground))",
                token(person_colour(None, &writer))
            )
        };
        out.push_str(&format!(
            "<div class=\"said{}\" style=\"{style}\"><small><b>{}</b> · {} · {}</small>\
             <p>{}</p></div>\n",
            if p.mine { " mine" } else { "" },
            escape(&name),
            reach(p.up, p.down),
            ago(now, p.written_at),
            escape(&p.text)
        ));
    }
    out.push_str("</div>");
    out
}

/// A thread being begun: where, and what the window's box is for.
fn board_new(snap: &Snapshot, subnet: &str) -> String {
    let s = snap.subnets.iter().find(|s| s.id == subnet);
    format!(
        "<header class=\"bar\"><button class=\"back\" data-act=\"back\" aria-label=\"Back\">‹\
         </button><h1>New thread</h1></header>\n<p class=\"sub\">On the board of {}. It reaches \
         the levels you set above you, with each one's direct subordinates, and the levels you \
         set below you.</p>",
        escape(&s.map(subnet_name).unwrap_or_else(|| short(subnet)))
    )
}

/// **A subnet's options**: what this key is there, as the phone's say it.
///
/// **As the phone's** [author, 2026-11-23: *"Subnet authority screen and
/// renaming should carry over"*]: the authority row, the downline, and for
/// the root the subnet's name. **A paired desktop shows what it cannot
/// sign** and leaves the act to the phone, since it signs nothing (design
/// §23.3).
fn subnet_options(snap: &Snapshot, id: &str) -> String {
    let s = snap.subnets.iter().find(|s| s.id == id);
    let mut out = bar(&s.map(subnet_name).unwrap_or_else(|| short(id)), true, None);
    out.push_str(&row(
        "Authority — who you are under, and what leaving costs",
        "every line this key holds, not this subnet's alone; choose one for the cost of ending it",
        "data-go=\"authority\"",
    ));
    out.push_str(&format!(
        "<p>{}</p>\n",
        escape(
            &s.map(|s| s.rank.clone())
                .unwrap_or_else(|| "Your standing here has not been read yet.".into())
        )
    ));
    // **the instance it runs, administered here on a restored desktop**
    // [author, 2026-11-26]; a paired one derives no key for the machine
    if let Some(s) = s.filter(|s| s.operates) {
        out.push_str("<h2>Its node</h2>\n");
        out.push_str(&administer(snap, s));
    }
    if s.is_some_and(|s| s.crown == Crown::Full) {
        out.push_str(&match snap.device {
            Device::Restored => format!(
                "<h2>The subnet's name</h2>\n<p class=\"sub\">You are its root, so its name is \
                 yours to set — by convention; nothing enforces it.</p>\n\
                 <label>Name<input id=\"rename\" data-subnet=\"{id}\" value=\"{}\" \
                 autocomplete=\"off\"></label>\n<p><button id=\"name\">{}</button></p>",
                escape(s.and_then(|s| s.name.as_deref()).unwrap_or("")),
                if s.and_then(|s| s.name.as_ref()).is_some() {
                    "Rename the subnet"
                } else {
                    "Name the subnet"
                }
            ),
            Device::Paired { .. } => "<h2>The subnet's name</h2>\n<p class=\"sub\">You are its \
                 root, so its name is yours to set. Naming it is signed as the identity, and \
                 this desktop holds no key of the identity's: name it on the phone.</p>"
                .to_string(),
        });
    }
    out
}

/// **Administering the node one subnet's row says this key runs**: the
/// door on a restored desktop, and on a paired one where the door is.
fn administer(snap: &Snapshot, s: &SubnetRow) -> String {
    match snap.device {
        Device::Restored => format!(
            "<p class=\"sub\">You run this subnet's node. What it declares it can be asked — \
             installs, grants, upgrades — is drawn here and sent over its machine's own \
             loopback.</p>\n<p><button data-administer=\"{}\">Administer the node</button></p>\n",
            escape(&s.by)
        ),
        Device::Paired { .. } => "<p class=\"sub\">You run this subnet's node. Administering it \
             takes the machine's key, which comes from the identity's seed, and this desktop was \
             paired and holds none: administer it on the phone.</p>\n"
            .to_string(),
    }
}

/// **The nodes this identity runs** [author, 2026-11-26]: one per subnet
/// where this key published the endpoint record, each with its door, and
/// launching another below them.
fn instances(snap: &Snapshot) -> String {
    let mut out = bar("Instances you run", true, None);
    let running: Vec<&SubnetRow> = snap.subnets.iter().filter(|s| s.operates).collect();
    if running.is_empty() {
        out.push_str("<p class=\"sub\">You run no node in any subnet this desktop holds.</p>\n");
    }
    for s in running {
        out.push_str(&format!("<h2>{}</h2>\n", escape(&subnet_name(s))));
        out.push_str(&administer(snap, s));
    }
    out.push_str(&row(
        "Launch a node",
        "at a provider you choose, with a credential of your own",
        "data-act=\"launch\"",
    ));
    out
}

/// **Which identity is in focus** [author, 2026-11-23]: the phone's list,
/// the one in focus marked, and nothing else — no narrative, no door to
/// another, since a desktop is given its identities by pairing.
fn identities(snap: &Snapshot) -> String {
    let mut out = bar("Identity", true, None);
    for h in &snap.held {
        let name = h
            .name
            .clone()
            .unwrap_or_else(|| format!("{}…", short(&h.by)));
        out.push_str(&format!(
            "<button class=\"row\" data-focus=\"{}\"><span><i class=\"swatch\" \
             style=\"background: var({})\"></i>{}{}</span></button>\n",
            h.by,
            token(&h.colour),
            if h.by == hex(&snap.me) { "● " } else { "" },
            escape(&name)
        ));
    }
    out
}

/// What the options offer for a board's pool, in megabytes, as the
/// phone's do.
pub const POOL_CHOICES: [u64; 5] = [10, 25, 50, 100, 250];

/// A pool's bound as the options show it: whole megabytes.
#[must_use]
pub fn pool_name(bytes: u64) -> String {
    format!("{} MB", bytes / (1024 * 1024))
}

/// **How much each subnet's board keeps here**, the person's to set
/// (`light-client-requirements.md` §4.5): lowered, the oldest posts go at
/// once.
fn pool(snap: &Snapshot) -> String {
    let mut out = bar("Board storage", true, None);
    out.push_str(
        "<p class=\"sub\">What each subnet's board keeps on this desktop. At the bound the \
         oldest posts go first, by their own dates.</p>\n",
    );
    for mb in POOL_CHOICES {
        let bytes = mb * 1024 * 1024;
        out.push_str(&format!(
            "<button class=\"row\" data-pool=\"{bytes}\"><span>{}{}</span></button>\n",
            if snap.pool == bytes { "● " } else { "" },
            pool_name(bytes)
        ));
    }
    out
}

/// **The colour of the identity in focus**, chosen from the palette by
/// name, as the phone's options offer it.
fn colour(snap: &Snapshot) -> String {
    let mut out = bar("Colour", true, None);
    for (name, token) in PALETTE {
        out.push_str(&format!(
            "<button class=\"row\" data-colour=\"{name}\"><span><i class=\"swatch\" \
             style=\"background: var({token})\"></i>{}{name}</span></button>\n",
            if snap.colour == name { "● " } else { "" }
        ));
    }
    out
}

/// **Who this key is under, and what leaving costs** (`light-client-
/// requirements.md` §6; the phone's authority screen): every line held,
/// each chosen for the cost of ending it, which arrives below.
fn authority(lines: &[LineRow]) -> String {
    let mut out = bar("Authority", true, None);
    out.push_str(&format!(
        "<p class=\"sub\">{}</p>\n",
        escape(crate::authority::WHY)
    ));
    if lines.is_empty() {
        out.push_str(&format!("<p>{}</p>\n", escape(crate::authority::NONE)));
    }
    for l in lines {
        out.push_str(&format!(
            "<button class=\"row\" data-cost=\"{}\" data-by=\"{}\"><span class=\"mono\">{}</span></button>\n",
            hex(&l.line.patron),
            escape(&l.by),
            escape(&crate::authority::line(&l.line, l.alias.as_deref()))
        ));
    }
    out.push_str("<div id=\"cost\"></div>");
    out
}

/// **What ending one line costs, put where the authority screen keeps it**:
/// the words, and the act — two presses, since it cannot be undone — or,
/// on a desktop that signs nothing, where the act is done instead.
pub fn cost_script(
    device: &Device,
    by: &str,
    patron: &str,
    alias: Option<&str>,
    cost: Result<&rhtn_model::Cost, &str>,
    names: &std::collections::HashMap<String, String>,
) -> String {
    let html = match cost {
        Err(why) => format!(
            "<p class=\"bad\">No cost for {}: {}</p>",
            escape(&crate::authority::party(patron, alias)),
            escape(why)
        ),
        Ok(c) => format!(
            "<pre>{}</pre>{}",
            crate::authority::words(c, names),
            match device {
                Device::Restored => format!(
                    "<p><button data-leave=\"{patron}\" data-by=\"{by}\" data-again=\"{}\">{}</button></p>",
                    escape(&crate::authority::button(patron, alias, true)),
                    escape(&crate::authority::button(patron, alias, false))
                ),
                Device::Paired { .. } => format!(
                    "<p class=\"sub\">{}</p>",
                    escape(crate::authority::ON_THE_PHONE)
                ),
            }
        ),
    };
    format!(
        "(() => {{ const c = document.getElementById('cost'); if (c) c.innerHTML = {}; }})();",
        js(&html)
    )
}

/// **One card per resource**, which is what opens it: its icon, else its
/// mark, its name, and its count above its corner where something waits
/// [author, 2026-11-23].
fn launchers(rows: &[ResourceRow], by: &str) -> String {
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
            "<button class=\"card\" data-open=\"{}\" data-node=\"{}\" data-by=\"{by}\" data-name=\"{}\">\
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
                 {days} days the phone signed. It reads everything and signs nothing but a board post."
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
        "{}{}{}{}{}<h2>This desktop</h2>\n<p class=\"sub\">{}</p>\n{dataset}\n\
         <p class=\"sub mono\">{}</p>\n{}",
        bar("Options", true, None),
        row("Serving node", &snap.connection, "data-go=\"node\""),
        row(
            "Instances you run",
            &match snap.subnets.iter().filter(|s| s.operates).count() {
                0 => "none yet; launch one at a provider".to_string(),
                1 => "one, administered from here".to_string(),
                n => format!("{n}, administered from here"),
            },
            "data-go=\"instances\""
        ),
        row("Colour", &snap.colour, "data-go=\"colour\""),
        row(
            "Board storage",
            &format!(
                "{} on this desktop for each subnet's board, oldest posts out first",
                pool_name(snap.pool)
            ),
            "data-go=\"pool\""
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
     your contacts and records come from a backup or not at all, and never over the \
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
            colour: "Teal".into(),
            pool: rhtn_client::board::POOL_BYTES as u64,
            held: vec![Holder {
                by: "ab".repeat(32),
                name: Some("Ann <A>".into()),
                colour: "Teal".into(),
            }],
            connection: "Connected to 1111111111111111…".into(),
            subnets: vec![SubnetRow {
                id: "22".repeat(32),
                name: Some("Mirror <Manse>".into()),
                rank: "Its operator.".into(),
                crown: Crown::Gold,
                node: "55".repeat(32),
                unseen: 3,
                by: "ab".repeat(32),
                colour: "Teal".into(),
                operates: false,
            }],
            people: vec![
                Person {
                    key: "33".repeat(32),
                    alias: Some("Carol".into()),
                    how: "met".into(),
                    by: "ab".repeat(32),
                    ..Default::default()
                },
                Person {
                    key: "44".repeat(32),
                    alias: None,
                    how: "introduced".into(),
                    by: "ab".repeat(32),
                    ..Default::default()
                },
            ],
            threads: vec![Entry {
                peer: "33".repeat(32),
                name: "33".repeat(32),
                direct: true,
                by: "ab".repeat(32),
            }],
            chosen: Default::default(),
            last: Default::default(),
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
            r.contains("style=\"background: var(--identity-teal)\">M&lt;</span>"),
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
        assert!(f.contains("data-go=\"home\" style=\"background: var(--identity-teal)\"><b>Ann &lt;A&gt;</b><small>home</small>"));
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
        assert!(d.contains("4444444444444444") && d.matches("<b>Carol</b>").count() == 1);
    }

    #[test]
    fn a_conversation_is_a_card_naming_where_they_stand_and_what_waits() {
        let mut s = snap();
        let (me, carol) = ("ab".repeat(32), "33".repeat(32));
        s.people[0].subnet = Some("Mirror".into());
        s.people[0].offset = Some(1);
        s.people[0].in_line = true;
        s.chosen.insert(
            (me.clone(), carol.clone()),
            Chosen {
                unread: 4,
                ..Default::default()
            },
        );
        // **the last line said, then what was muted**, as the phone's card
        s.last
            .insert((me.clone(), carol.clone()), (false, "see <you>".into()));
        assert!(direct(&s).contains("<span><small>see &lt;you&gt;</small></span>"));
        s.last
            .insert((me.clone(), carol.clone()), (true, "on my way".into()));
        s.chosen
            .get_mut(&(me.clone(), carol.clone()))
            .unwrap()
            .muted = true;
        assert!(direct(&s).contains("<small>me: on my way · muted</small>"));
        s.last.remove(&(me.clone(), carol.clone()));
        assert!(direct(&s).contains("<small>no messages · muted</small>"));
        s.chosen
            .get_mut(&(me.clone(), carol.clone()))
            .unwrap()
            .muted = false;
        let d = direct(&s);
        assert!(d.contains("<b>Carol</b> <span class=\"place\">Mirror <b>+1</b></span>"));
        assert!(d.contains("<sup>4</sup>") && d.contains(&format!("data-person=\"{carol}\"")));
        assert!(rail(&s, &Place::Direct).contains("Direct messages, 4 unseen"));
        // **muted counts on its card alone**, and a branch beside this
        // key's line is not in bold
        s.people[0].in_line = false;
        s.chosen
            .get_mut(&(me.clone(), carol.clone()))
            .unwrap()
            .muted = true;
        let d = direct(&s);
        assert!(d.contains("Mirror +1</span>") && d.contains("<sup class=\"muted\">4</sup>"));
        assert_eq!(s.direct_unseen(), 0);
        let o = person(&s, &carol);
        assert!(o.contains("Unmute") && o.contains("Mute notifications") && o.contains("Block"));
        assert!(o.contains(&format!(
            "The one their key gives them ({})",
            person_colour(None, &carol)
        )));
    }

    // functional: PAY-023
    #[test]
    fn bubbles_wear_the_speaker_s_identity_and_the_party_s_tint_and_a_block_ends_the_box() {
        let mut s = snap();
        let (me, carol) = ("ab".repeat(32), "33".repeat(32));
        s.chosen.insert(
            (me.clone(), carol.clone()),
            Chosen {
                colour: Some("Pink".into()),
                ..Default::default()
            },
        );
        let said = |mine| Message {
            id: 0,
            mine,
            who: String::new(),
            text: "hi".into(),
            delivery: Delivery::Sent,
            reason: None,
        };
        let p = page(
            &s,
            &Place::Thread(carol.clone()),
            &Body::Thread(vec![said(false), said(true)], false),
        );
        assert!(p.contains("color-mix(in srgb, var(--identity-pink) 28%, var(--ground))"));
        assert!(p.contains("class=\"said mine\" style=\"border-color: var(--identity-teal)\""));
        assert!(p.contains("id=\"compose\""));
        // **a sender that could not be attributed is not answered**
        // [author, 2026-11-20], and the thread says why
        s.chosen
            .get_mut(&(me.clone(), carol.clone()))
            .unwrap()
            .unverified = true;
        let p = page(
            &s,
            &Place::Thread(carol.clone()),
            &Body::Thread(vec![], false),
        );
        assert!(!p.contains("id=\"compose\"") && p.contains("Unverified: this sender"));
        s.chosen
            .get_mut(&(me.clone(), carol.clone()))
            .unwrap()
            .unverified = false;
        s.chosen.get_mut(&(me, carol.clone())).unwrap().blocked = true;
        let p = page(&s, &Place::Thread(carol), &Body::Thread(vec![], false));
        assert!(!p.contains("id=\"compose\"") && p.contains("Blocked:"));
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
        assert!(
            p.contains(&format!(
                "data-earlier=\"{peer}\"><span>Earlier messages</span>"
            )) && p.contains("data-act=\"back\"")
        );
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
        let p = page(
            &s,
            &Place::Subnet(id.clone()),
            &Body::Launchers(rows, BoardList::default()),
        );
        // **a card each, which is what opens it** [author, 2026-11-23], and
        // **badged with the resource's icon**, as its node hands it
        assert!(p.contains(&format!(
            "<button class=\"card\" data-open=\"{}\" data-node=\"{}\" data-by=\"{}\" data-name=\"Notices\">\
             <span class=\"icon\"><img src=\"data:image/png;base64,iVBORw==\" alt=\"\"></span>\
             <span class=\"name\">Notices</span><sup>2</sup></button>",
            "77".repeat(32),
            "55".repeat(32),
            "ab".repeat(32)
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
        assert!(o.contains("data-go=\"instances\"") && o.contains(&"ab".repeat(32)));
        // **the nodes this key runs, then launching another** [author,
        // 2026-11-26]: where an operator looks for them
        assert!(o.contains("none yet; launch one at a provider"));
        let running = Snapshot {
            subnets: vec![SubnetRow {
                operates: true,
                ..s.subnets[0].clone()
            }],
            ..s.clone()
        };
        assert!(
            page(&running, &Place::Options, &Body::Plain).contains("one, administered from here")
        );
        let i = page(&running, &Place::Instances, &Body::Plain);
        assert!(
            i.contains("data-administer=") && i.contains("data-act=\"launch\""),
            "{i}"
        );
        let none = page(&s, &Place::Instances, &Body::Plain);
        assert!(!none.contains("data-administer=") && none.contains("data-act=\"launch\""));
        assert_eq!(Place::parse("instances"), Some(Place::Instances));
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

    // **the authority screen and the subnet's name carry over** [author,
    // 2026-11-23], and a paired desktop leaves the signed acts to the phone
    #[test]
    fn a_subnets_options_lead_to_authority_and_its_root_names_it_where_this_desktop_signs() {
        let s = snap();
        let id = "22".repeat(32);
        let root = Snapshot {
            subnets: vec![SubnetRow {
                crown: Crown::Full,
                ..s.subnets[0].clone()
            }],
            ..s.clone()
        };
        let o = page(&root, &Place::SubnetOptions(id.clone()), &Body::Plain);
        assert!(o.contains("data-go=\"authority\"") && o.contains("Its operator."));
        assert!(o.contains(&format!(
            "id=\"rename\" data-subnet=\"{id}\" value=\"Mirror &lt;Manse&gt;\""
        )));
        assert!(o.contains(">Rename the subnet<"));
        let paired = Snapshot {
            device: Device::Paired {
                days: 90,
                dataset: None,
            },
            ..root.clone()
        };
        let po = page(&paired, &Place::SubnetOptions(id.clone()), &Body::Plain);
        assert!(!po.contains("id=\"rename\"") && po.contains("name it on the phone"));
        // **the node it runs**, administered on a restored desktop alone
        // [author, 2026-11-26]
        assert!(
            !o.contains("data-administer"),
            "only where this key runs one"
        );
        let running = |snap: &Snapshot| Snapshot {
            subnets: vec![SubnetRow {
                operates: true,
                ..snap.subnets[0].clone()
            }],
            ..snap.clone()
        };
        let ro = page(
            &running(&root),
            &Place::SubnetOptions(id.clone()),
            &Body::Plain,
        );
        assert!(
            ro.contains(&format!("data-administer=\"{}\"", root.subnets[0].by)),
            "{ro}"
        );
        let rpo = page(
            &running(&paired),
            &Place::SubnetOptions(id.clone()),
            &Body::Plain,
        );
        assert!(!rpo.contains("data-administer") && rpo.contains("administer it on the phone"));
        // not the root: no name to set at all
        assert!(!page(&s, &Place::SubnetOptions(id), &Body::Plain).contains("The subnet's name"));

        let line = rhtn_model::Line {
            patron: [0x33; 32],
            series: 2,
            anchor: [0; 32],
            nibbles: 0,
        };
        let a = page(
            &s,
            &Place::Authority,
            &Body::Authority(vec![LineRow {
                line,
                alias: Some("Carol".into()),
                by: "ab".repeat(32),
            }]),
        );
        assert!(a.contains(&format!("data-cost=\"{}\"", "33".repeat(32))));
        assert!(a.contains("Carol, 333333333333…   series 2") && a.contains("id=\"cost\""));
        assert!(page(&s, &Place::Authority, &Body::Authority(vec![])).contains("under no patron"));

        let cost = rhtn_model::Cost {
            losing: vec![],
            keeping: 2,
            leaving: vec![],
            limits: vec![rhtn_model::Limit::PredicateNotHeld],
        };
        let names = std::collections::HashMap::new();
        let who = "33".repeat(32);
        let restored = cost_script(
            &Device::Restored,
            &"ab".repeat(32),
            &who,
            Some("Carol"),
            Ok(&cost),
            &names,
        );
        assert!(restored.contains("Nothing this device can see goes with it."));
        assert!(restored.contains("2 resources stay within reach."));
        assert!(
            restored.contains(&format!("data-leave=\\\"{who}\\\"")),
            "{restored}"
        );
        assert!(
            restored.contains("Press again to leave Carol") && restored.contains(">Leave Carol<")
        );
        let on_phone = cost_script(
            &paired.device,
            &"ab".repeat(32),
            &who,
            Some("Carol"),
            Ok(&cost),
            &names,
        );
        assert!(!on_phone.contains("data-leave") && on_phone.contains("leave on the phone"));
        let refused = cost_script(
            &Device::Restored,
            &"ab".repeat(32),
            &who,
            None,
            Err("no such line"),
            &names,
        );
        assert!(refused.contains("No cost for 333333333333…: no such line"));
    }

    #[test]
    fn the_identitys_colour_is_chosen_from_the_palette_and_worn_by_its_orbs_and_foot() {
        let s = snap();
        let c = page(&s, &Place::Colour, &Body::Plain);
        assert_eq!(c.matches("data-colour=").count(), PALETTE.len());
        assert!(c.contains("● Teal") && !c.contains("● Red"));
        assert!(
            rail(&s, &Place::Home)
                .contains("class=\"disc\" style=\"background: var(--identity-teal)\"")
        );
        assert!(foot(&s).matches("background: var(--identity-teal)").count() == 2);
        assert!(
            page(&s, &Place::Options, &Body::Plain)
                .contains("data-go=\"colour\"><span>Colour</span><small>Teal</small>")
        );
        assert_eq!(token("Nonesuch"), "--rail-orb");
        assert_eq!(Place::parse("colour"), Some(Place::Colour));
        assert_eq!(Place::parse("authority"), Some(Place::Authority));
    }

    /// **How much each subnet's board keeps here is the person's**
    /// (`light-client-requirements.md` §4.5), chosen on the options from
    /// the bounds offered, the one in force marked.
    // functional: BRD-013
    #[test]
    fn the_board_pool_is_chosen_from_the_bounds_offered_the_one_in_force_marked() {
        let s = snap();
        let p = page(&s, &Place::Pool, &Body::Plain);
        assert_eq!(p.matches("data-pool=").count(), POOL_CHOICES.len());
        assert!(
            p.contains("data-pool=\"52428800\"><span>● 50 MB</span>"),
            "{p}"
        );
        assert!(!p.contains("● 10 MB"));
        assert!(
            page(&s, &Place::Options, &Body::Plain).contains(
                "data-go=\"pool\"><span>Board storage</span><small>50 MB on this desktop"
            )
        );
        assert_eq!(Place::parse("pool"), Some(Place::Pool));
        assert_eq!(Place::Pool.parent(), Place::Options);
    }

    // **every identity the desktop holds, and which acts** [author,
    // 2026-11-23]: the picker marks the one in focus, a thread is marked
    // with the colour of the identity it is held as once there are two, and
    // a subnet's orb wears its holder's colour
    #[test]
    fn the_identities_held_are_listed_and_each_row_wears_its_holders_colour() {
        let mut s = snap();
        let one = page(&s, &Place::Direct, &Body::Plain);
        assert!(
            !one.contains("class=\"swatch\""),
            "one identity marks nothing"
        );
        s.held.push(Holder {
            by: "cd".repeat(32),
            name: None,
            colour: "Pink".into(),
        });
        s.threads.push(Entry {
            peer: "44".repeat(32),
            name: "44".repeat(32),
            direct: true,
            by: "cd".repeat(32),
        });
        s.subnets[0].colour = "Pink".into();
        let d = page(&s, &Place::Direct, &Body::Plain);
        assert!(d.contains("var(--identity-teal)") && d.contains("var(--identity-pink)"));
        let i = page(&s, &Place::Identities, &Body::Plain);
        assert!(i.contains(&format!("data-focus=\"{}\"", "ab".repeat(32))));
        assert!(
            i.contains("● Ann &lt;A&gt;") && i.contains(&format!("{}…", short(&"cd".repeat(32))))
        );
        assert!(
            !i.contains("New"),
            "no door to another: a desktop's are paired"
        );
        assert!(rail(&s, &Place::Home).contains("var(--identity-pink)\">M&lt;"));
        assert!(
            foot(&s).contains("var(--identity-teal)"),
            "the foot is the focus's"
        );
        assert_eq!(Place::parse("identities"), Some(Place::Identities));
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

    fn a_post(mine: bool, alias: Option<&str>, text: &str, at: u64) -> rhtn_model::BoardPost {
        rhtn_model::BoardPost {
            hash: [1; 32],
            thread: [0x44; 32],
            writer: if mine { [0xab; 32] } else { [0x66; 32] },
            alias: alias.map(String::from),
            mine,
            text: text.into(),
            written_at: at,
            up: 2,
            down: 3,
            read: true,
        }
    }

    #[test]
    fn a_subnets_page_lists_its_board_beside_its_launchers() {
        let s = snap();
        let id = "22".repeat(32);
        // nothing held yet: the way to begin a thread, and that it is empty
        let empty = page(
            &s,
            &Place::Subnet(id.clone()),
            &Body::Launchers(Vec::new(), BoardList::default()),
        );
        assert!(empty.contains("<h2>Board</h2>"));
        assert!(empty.contains(&format!("data-go=\"board-new:{id}\"")));
        assert!(empty.contains("Nothing posted yet"));
        // a thread heads the list by its title, wearing what is unread and
        // leading to itself
        let board = BoardList {
            threads: vec![
                rhtn_model::BoardThread {
                    id: [0x44; 32],
                    title: Some("Meeting <notes>".into()),
                    began_by: Some([0x66; 32]),
                    posts: 3,
                    unread: 2,
                    latest: 1_000,
                },
                rhtn_model::BoardThread {
                    id: [0x45; 32],
                    title: None,
                    began_by: None,
                    posts: 1,
                    unread: 0,
                    latest: 400,
                },
            ],
            now: 1_000 + 7_200,
        };
        let p = page(
            &s,
            &Place::Subnet(id.clone()),
            &Body::Launchers(Vec::new(), board),
        );
        let thread = format!("board:{id}:{}", "44".repeat(32));
        assert!(p.contains(&format!("data-go=\"{thread}\"")));
        assert!(
            p.contains("<b>Meeting &lt;notes&gt;</b><sup>2</sup>"),
            "escaped, and counted"
        );
        assert!(p.contains("3 posts, 2 h ago") && p.contains("1 post, "));
        assert!(p.contains("A thread begun out of your reach"));
        // the places read back as they are named, and a thread takes the
        // window while the subnet's page keeps the rail
        for place in [
            Place::Board(id.clone(), "44".repeat(32)),
            Place::BoardNew(id.clone()),
        ] {
            assert_eq!(Place::parse(&place.token()), Some(place.clone()));
            assert!(!place.chrome());
        }
        assert_eq!(Place::parse("board:22:44"), None);
        assert_eq!(Place::parse(&format!("board:{id}")), None);
    }

    #[test]
    fn a_thread_takes_the_window_with_each_post_saying_who_how_far_and_when() {
        let s = snap();
        let id = "22".repeat(32);
        let place = Place::Board(id.clone(), "44".repeat(32));
        let body = Body::Board {
            title: Some("Meeting notes".into()),
            posts: vec![
                a_post(false, Some("Cy <the writer>"), "First <b>post</b>", 1_000),
                a_post(true, None, "My reply", 1_030),
            ],
            now: 1_060,
        };
        let p = page(&s, &place, &body);
        assert!(p.contains("<h1>Meeting notes</h1>") && p.contains("data-act=\"back\""));
        assert!(!p.contains("id=\"rail\""), "the rail went with the chrome");
        // the alias the post carried, where this key holds no name for them
        assert!(p.contains("<b>Cy &lt;the writer&gt;</b> · +2/-3 · just now"));
        assert!(
            p.contains("First &lt;b&gt;post&lt;/b&gt;"),
            "what was written is text"
        );
        // this key's own to the right, outlined in the colour it speaks as
        assert!(p.contains("class=\"said mine\" style=\"border-color: var(--identity-teal)\""));
        // the reply box states the thread's reach and offers none
        assert!(p.contains(&format!(
            "id=\"board-reply\" rows=\"3\" data-subnet=\"{id}\""
        )));
        assert!(p.contains("It travels as the thread does: +2/-3"));
        assert!(!p.contains("id=\"board-up\""));
        // a refresh leaves what is being typed where it is
        let r = refresh(&s, &place, &body);
        assert!(r.contains("put('main'") && !r.contains("board-reply"));
        // a new thread offers its reach, two each way until changed
        let n = page(&s, &Place::BoardNew(id.clone()), &Body::Plain);
        assert!(n.contains("id=\"board-title\"") && n.contains("id=\"board-text\""));
        assert!(n.contains("id=\"board-up\" type=\"number\" min=\"0\" max=\"24\" value=\"2\""));
        assert!(n.contains(&format!("id=\"board-post\" data-subnet=\"{id}\"")));
        // an age is said shortly
        assert_eq!(ago(100, 100), "just now");
        assert_eq!(ago(700, 100), "10 min ago");
        assert_eq!(ago(100 + 7_200, 100), "2 h ago");
        assert_eq!(ago(100 + 3 * 86_400, 100), "3 d ago");
        assert_eq!(ago(100, 900), "just now", "a clock behind the writer's");
    }
}
