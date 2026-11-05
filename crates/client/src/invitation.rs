//! **An invitation to serve from a node, as a link** [author,
//! 2026-11-04].
//!
//! ## What this is for
//!
//! *"Attachment is not a separate act. The operator should attach upon
//! launch and install, other horizon members should agree to attach once
//! when invited and then all horizon members should re-attach every time
//! they become detached."*
//!
//! The operator's half needs nothing from anybody: they launched the
//! machine, so their client holds its address, and the node speaks as its
//! operator, so the key is their own. A **member** has neither. Nodeless,
//! no endpoint record has reached them (`wire-format.md` §7.6 carries one
//! in the topology class, which needs a node to flood it), so the first
//! attach is the one thing a member cannot work out for itself.
//!
//! So the patron sends one string. The carrier is whatever reaches the
//! person — a message, a mail, a code on a screen — and what the device
//! sees is an `ACTION_VIEW` intent, exactly as it would from a tapped
//! link anywhere. **The carrier is not this module's business**, which is
//! also why it can be exercised without one.
//!
//! ## **An invitation carries no key, and that is the point**
//!
//! [author, 2026-11-04] *"The node knows its allowed users. The invitee
//! holds the secret that corresponds to the key material the node has been
//! provided for them. It seems like this should be enough for the node to
//! establish the identity of the client contacting it for attachment
//! without having to distribute a node key."*
//!
//! The node needs nothing: it holds its members' key material and each one
//! proves itself by signature. What the *client* needs is to know **who
//! must answer** at that address, or a wrong or hijacked one attaches it
//! silently to somebody else's node, which then relays its payload and
//! sees who it reaches.
//!
//! **And the client already holds that.** A member's node is its patron's,
//! and the patron is in the member's own archive. So an invitation carries
//! an address, and the key comes from the member's own records.
//!
//! Which makes the thing this is protecting against **impossible rather
//! than merely visible**: an invitation that cannot name a key cannot name
//! a key the member has never met. The most a link can do is point at an
//! address for a node the member already has a relationship with. The
//! author's reason for the tap [2026-10-29] — *"to prevent stealth
//! insertion of a node the user did not choose"* — is answered by the
//! format and not only by the screen.
//!
//! **Nothing here authorises anything**: every field fills a screen the
//! person agrees to, and a signature would prove who composed the link
//! without answering the question they are actually being asked.
//!
//! ## The shape
//!
//! ```text
//! fueros://attach?v=1&at=<host:port>[&patron=<hex>][&subnet=<hex>][&member=<hex>][&as=<label>]
//! ```
//!
//! - `at` — where the node answers the protocol, an address and a port. A
//!   name is not one: the kernel resolves nothing (`Attach`).
//! - `patron`, `subnet`, `member` — **which membership is being invited**
//!   [author, 2026-11-04]: "the user belongs to and attaches to many
//!   subnets and their device may hold multiple users, so it makes sense
//!   to give them something to index which of their subnet memberships is
//!   being invited". Each is **checked against what the device holds** and
//!   refused where it names something it does not — an index into the
//!   member's own records, never a fact taken on the link's word.
//! - `as` — what the inviter calls themselves, for the screen alone. **A
//!   claim and never a fact.**
//!
//! **Bounded on the way in**: a field that can be any length can fill a
//! screen, so the whole string is refused over [`MAX_INVITATION`] unparsed.

/// Why an invitation was not read. Its own type, and not the offering's:
/// each says what it is about, and a person told "this is not a package
/// offering" about an invitation has been told nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// Not an invitation at all.
    NotAnInvitation,
    /// Longer than [`MAX_INVITATION`].
    TooLong,
    /// A version this client does not read; the version it carried.
    Version(String),
    /// A field was missing or was not what it should be.
    Field(&'static str, String),
}

impl Refused {
    /// What to tell the person, in their client's words.
    #[must_use]
    pub fn say(&self) -> String {
        match self {
            Refused::NotAnInvitation => {
                format!("this is not an invitation to serve from a node; one begins `{PREFIX}`")
            }
            Refused::TooLong => {
                format!("an invitation is at most {MAX_INVITATION} characters")
            }
            Refused::Version(v) => format!(
                "this invitation is version `{v}` and this client reads version `{VERSION}`: a \
                 newer client will read it"
            ),
            Refused::Field(name, why) => format!("`{name}`: {why}"),
        }
    }
}

/// What a link carries before its query.
pub const PREFIX: &str = "fueros://attach";

/// The version this build composes and the only one it reads.
pub const VERSION: &str = "1";

/// The longest invitation this will look at. Two keyhashes, an address and
/// a short label come to a little over two hundred characters; the rest is
/// room for a field this build does not know about.
pub const MAX_INVITATION: usize = 1024;

/// The longest label an inviter may suggest for itself.
pub const MAX_AS: usize = 120;

/// **An invitation, as read.** Every field is the inviter's claim; nothing
/// here has been checked against anything this client holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    /// Where the node answers the protocol: an address and a port.
    pub at: String,
    /// **Which membership is being invited**, since a person belongs to
    /// many subnets and a device may hold several of them [author,
    /// 2026-11-04]. Each of the three is a hint checked against what this
    /// device holds, and refused where it names something it does not —
    /// never a fact taken on the link's word.
    ///
    /// Which patron's node this is.
    pub patron: Option<[u8; 32]>,
    /// Which subnet, where the same patron serves more than one
    /// (design §3.1.1: one node, several bindings, each its own).
    pub subnet: Option<[u8; 32]>,
    /// **Which identity on the device** this invitation is for, since a
    /// device may hold several (APP-013: one kernel per identity per
    /// storage location).
    pub member: Option<[u8; 32]>,
    /// What the inviter calls themselves, for the screen alone. A claim.
    pub as_label: Option<String>,
}

fn key32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// **An address and a port, and nothing a resolver would be needed for.**
///
/// The same rule the attach screen held to: the kernel resolves nothing,
/// so a name here would be an address that never answers and a person
/// told nothing useful about why.
fn address(s: &str) -> Option<String> {
    let (host, port) = s.rsplit_once(':')?;
    let p: u16 = port.parse().ok()?;
    if p == 0 {
        return None;
    }
    let octets: Vec<&str> = host.split('.').collect();
    if octets.len() != 4 {
        return None;
    }
    for o in &octets {
        if o.is_empty() || o.len() > 3 || !o.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if o.parse::<u16>().ok()? > 255 {
            return None;
        }
    }
    Some(s.to_string())
}

/// Percent-decoding, enough for the fields this carries.
fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// **Read an invitation**, or refuse it with the words a person is told.
pub fn parse(text: &str) -> Result<Invitation, Refused> {
    let text = text.trim();
    if text.len() > MAX_INVITATION {
        return Err(Refused::TooLong);
    }
    let rest = text.strip_prefix(PREFIX).ok_or(Refused::NotAnInvitation)?;
    let query = rest.strip_prefix('?').ok_or(Refused::NotAnInvitation)?;
    let mut at = None;
    let mut patron = None;
    let mut subnet = None;
    let mut member = None;
    let mut as_label = None;
    let mut version = None;
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "v" => version = Some(v.to_string()),
            "at" => at = address(&decode(v)),
            "patron" => patron = key32(v),
            "subnet" => subnet = key32(v),
            "member" => member = key32(v),
            "as" => {
                let label = decode(v);
                if label.len() > MAX_AS {
                    return Err(Refused::Field(
                        "as",
                        format!("a name is at most {MAX_AS} characters to show"),
                    ));
                }
                as_label = Some(label).filter(|l| !l.is_empty());
            }
            // **a field this build does not know is ignored, not refused**:
            // an invitation a later version wrote must still be openable,
            // for the same reason a retraction must be
            _ => {}
        }
    }
    match version.as_deref() {
        Some(VERSION) => {}
        Some(other) => return Err(Refused::Version(other.to_string())),
        None => {
            return Err(Refused::Field(
                "v",
                "an invitation says which version it is".into(),
            ));
        }
    }
    let at = at.ok_or(Refused::Field(
        "at",
        "where the node answers, as an address and a port".into(),
    ))?;
    Ok(Invitation {
        at,
        patron,
        subnet,
        member,
        as_label,
    })
}

/// **Compose one**, which is what a patron's client hands to whatever
/// carries it.
#[must_use]
pub fn compose(i: &Invitation) -> String {
    let hex = |b: &[u8; 32]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let mut s = format!("{PREFIX}?v={VERSION}&at={}", i.at);
    if let Some(p) = &i.patron {
        s.push_str(&format!("&patron={}", hex(p)));
    }
    if let Some(sn) = &i.subnet {
        s.push_str(&format!("&subnet={}", hex(sn)));
    }
    if let Some(m) = &i.member {
        s.push_str(&format!("&member={}", hex(m)));
    }
    if let Some(l) = &i.as_label {
        let esc: String = l
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect();
        s.push_str(&format!("&as={esc}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const NODE: &str = "e301f7e2b80a3755b41f806d936150c957ce5127f6ae537e0dfeaef39d3522e1";

    #[test]
    fn a_whole_invitation_round_trips() {
        let i = Invitation {
            at: "142.93.86.90:7431".into(),
            patron: key32(NODE),
            subnet: key32(&"bb".repeat(32)),
            member: key32(&"cc".repeat(32)),
            as_label: Some("Garden Club".into()),
        };
        let text = compose(&i);
        assert_eq!(parse(&text).expect("reads back"), i);
        assert!(text.starts_with("fueros://attach?v=1&at=142.93.86.90:7431"));
        assert!(text.contains("&patron=e301f7e2"), "{text}");
        assert!(text.contains("&as=Garden%20Club"), "{text}");
    }

    #[test]
    fn the_least_it_can_carry_is_an_address() {
        // **and it carries no key at all**: the member's own archive has
        // the patron, so a link that named one would be asking to be
        // trusted about who answers
        let text = format!("{PREFIX}?v=1&at=10.0.0.4:7431");
        let i = parse(&text).expect("enough to act on");
        assert_eq!(i.at, "10.0.0.4:7431");
        assert!(i.patron.is_none() && i.subnet.is_none() && i.member.is_none());
        assert!(i.as_label.is_none());

        for missing in [
            format!("{PREFIX}?v=1&patron={NODE}"),
            format!("{PREFIX}?at=10.0.0.4:7431"),
        ] {
            assert!(parse(&missing).is_err(), "{missing}");
        }
    }

    #[test]
    fn a_name_is_not_an_address_because_the_kernel_resolves_nothing() {
        for bad in [
            "node.example:7431",
            "142.93.86.90",
            "142.93.86.90:0",
            "142.93.86.300:7431",
            "142.93.86:7431",
        ] {
            let text = format!("{PREFIX}?v=1&at={bad}");
            assert!(parse(&text).is_err(), "{bad} must be refused");
        }
    }

    #[test]
    fn a_field_a_later_version_wrote_is_ignored_rather_than_refused() {
        // the same rule a retraction holds to: two parties will not be
        // running the same build
        let text = format!("{PREFIX}?v=1&at=10.0.0.4:7431&standing_hint=whatever");
        assert!(parse(&text).is_ok());
    }

    #[test]
    fn what_cannot_fit_on_a_screen_is_refused_unparsed() {
        let long = format!(
            "{PREFIX}?v=1&at=10.0.0.4:7431&as={}",
            "n".repeat(MAX_AS + 1)
        );
        assert!(parse(&long).is_err(), "a label that would not fit");
        let huge = format!("{PREFIX}?v=1&{}", "x".repeat(MAX_INVITATION));
        assert!(parse(&huge).is_err(), "and a string that would not either");
    }

    #[test]
    fn a_version_this_build_does_not_read_says_so() {
        let text = format!("{PREFIX}?v=9&at=10.0.0.4:7431");
        let e = parse(&text).expect_err("refused");
        assert_eq!(e, Refused::Version("9".into()));
        assert!(e.say().contains("version `9`"), "{}", e.say());
    }
}

/// **Whose node an invitation points at**, worked out from this client's
/// own archive rather than from the link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placed {
    /// One patron: the key that must answer at that address.
    Node(rhtn_archive::Keyhash),
    /// Several patrons and the link named none, so the person chooses
    /// whose node this is. A real choice, and the only one here.
    Choose(Vec<rhtn_archive::Keyhash>),
    /// The link named a party this client is not under. **Refused rather
    /// than offered**: an invitation cannot introduce a patron.
    NotAPatron(rhtn_archive::Keyhash),
    /// This client is under nobody, so there is no node for it to serve
    /// from and nothing an invitation could change about that.
    NoPatron,
    /// **The invitation is for a different identity than the one running
    /// here.** A device may hold several, so an invitation that named one
    /// is honoured for that one alone.
    NotThisMember(rhtn_archive::Keyhash),
}

impl crate::ceremony::Client {
    /// **Whose node this invitation points at.**
    ///
    /// **The key comes from this client's own archive, never from the
    /// link** [author, 2026-11-04]. A member's node is its patron's, and
    /// the patron is in the archive — so a link needs to carry only an
    /// address, and one that cannot name a key cannot name a key this
    /// client has never met.
    ///
    /// No request goes out: an invitation that had to be checked with
    /// somebody would be one whose sender chose who to ask.
    #[must_use]
    pub fn place_invitation(&self, i: &Invitation) -> Placed {
        let patrons: Vec<rhtn_archive::Keyhash> = {
            let mut v: Vec<_> = crate::rotation::relationships(&self.archive)
                .iter()
                .map(|r| r.patron)
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        if let Some(m) = i.member
            && m != self.public.keyhash
        {
            // **a device may hold several identities**, and an invitation
            // that indexed one is not this one's to take
            return Placed::NotThisMember(m);
        }
        match i.patron {
            Some(named) if patrons.contains(&named) => Placed::Node(named),
            Some(named) => Placed::NotAPatron(named),
            None => match patrons.len() {
                0 => Placed::NoPatron,
                1 => Placed::Node(patrons[0]),
                _ => Placed::Choose(patrons),
            },
        }
    }
}
