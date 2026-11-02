//! **A package offered for installation, as a link or a code** [author,
//! 2026-11-02].
//!
//! ## What this is for
//!
//! An operator installs a package on an instance they run
//! (`infra-client-requirements.md` §9.4), and to do it they need the
//! image reference, the authority, a port and **the package's own
//! manifest** — which `infra-client-requirements.md` §9.1 puts in the
//! manifest that ships with the package, so it has to come from whoever
//! is distributing it. Reading five fields off a web page and typing them
//! into a phone is how that goes wrong.
//!
//! So a distributor publishes one string. A text link, where the operator
//! is reading; a code on a screen, where they are not. Opening it hands
//! the client everything the install act wants except what only the
//! operator has.
//!
//! ## An offering is not an installation
//!
//! **Nothing here authorises anything.** This is untrusted content from
//! outside the client — a page, a message, a sticker on a wall — and
//! every field in it is **text that fills a screen the operator
//! confirms**. Three things keep that true:
//!
//! - **What the link cannot say.** It carries no resource identity (the
//!   operator's client mints that), no owner (that is the operator's own
//!   identity), no instance (which of their instances is their choice),
//!   and no key. An offering that arrived with those would be an install
//!   somebody else composed.
//! - **The manifest is shown before the act.** §9.1's supply chain is
//!   *"trusting the package author's judgment about access"*, and
//!   `resource-requirements.md` §7.3 has the author's incentive run
//!   toward breadth. What the package declares is the thing the operator
//!   is admitting, so a client draws it and waits.
//! - **Bounded on the way in.** A field that can be any length is a
//!   field that can fill a screen or a request; each is bounded here, and
//!   an offering over [`MAX_OFFERING`] is refused unparsed.
//!
//! ## The shape
//!
//! ```text
//! fueros://install?v=1&name=<label>&image=<reference>
//!                  &authority=<name>&port=<number>&manifest=<base64url>
//! ```
//!
//! `scheme://host` rather than a path, because that is what a device
//! registers a handler for. `v` is first and is checked first: a client
//! meeting a version it does not know says so rather than reading the
//! fields it recognises out of a format that may have changed under them.
//!
//! The manifest is carried **inline and base64url**, not as a URL to
//! fetch. A fetch would make the client retrieve attacker-chosen content
//! before the operator had agreed to anything, and would stop a code
//! working where there is no network — and a manifest is a few hundred
//! bytes, which a code holds comfortably.

use std::collections::BTreeMap;

/// The scheme and host a device registers a handler for.
pub const PREFIX: &str = "fueros://install";

/// **The version this client composes and reads.**
pub const VERSION: &str = "1";

/// The most an offering may be, whole. A code holds a few thousand
/// characters; beyond that something is being sent that is not an
/// offering.
pub const MAX_OFFERING: usize = 8192;

/// The most a label may be: a line a person reads.
pub const MAX_NAME: usize = 120;

/// The most a manifest may be, which is what a package's own declaration
/// is bounded at anywhere (`rhtn_node::resources::MAX_TEXT` is the same
/// number for a declared text field).
pub const MAX_MANIFEST: usize = 4096;

/// A package offered for installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offering {
    /// What the operator reads: the package's name, as its distributor
    /// put it. **A label and nothing more** — it is not checked against
    /// the image, the authority or the manifest, and a client that
    /// presented it as though it were would be presenting a claim as a
    /// fact.
    pub name: String,
    /// The image reference the machine will pull.
    pub image: String,
    /// The authority suggested for it, which the operator may change.
    pub authority: String,
    /// The port suggested, where one is suggested.
    pub port: Option<u16>,
    /// **The package's own manifest**, as its author wrote it: what it
    /// declares it needs and what roles it offers. Shown to the operator
    /// before the act and relayed to the instance unchanged.
    pub manifest: String,
}

/// Why an offering was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// Not an offering at all.
    NotAnOffering,
    /// Longer than [`MAX_OFFERING`].
    TooLong,
    /// A version this client does not read; the version it carried.
    Version(String),
    /// A field was missing or was not what it should be.
    Field(&'static str, String),
}

impl Refused {
    /// What to tell the person, in their client's words.
    pub fn say(&self) -> String {
        match self {
            Refused::NotAnOffering => {
                format!("this is not a package offering; one begins `{PREFIX}`")
            }
            Refused::TooLong => {
                format!("a package offering is at most {MAX_OFFERING} characters")
            }
            Refused::Version(v) => format!(
                "this offering is version `{v}` and this client reads version `{VERSION}`: a \
                 newer client will read it"
            ),
            Refused::Field(name, why) => format!("`{name}`: {why}"),
        }
    }
}

/// **Read an offering**, or say why not.
pub fn parse(text: &str) -> Result<Offering, Refused> {
    let text = text.trim();
    if text.len() > MAX_OFFERING {
        return Err(Refused::TooLong);
    }
    let rest = text.strip_prefix(PREFIX).ok_or(Refused::NotAnOffering)?;
    let query = match rest.strip_prefix('?') {
        Some(q) => q,
        None if rest.is_empty() => "",
        // `fueros://installsomethingelse` is not this
        None => return Err(Refused::NotAnOffering),
    };
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        // **a repeated field is a refusal, not a last-one-wins**: two
        // images in one offering is an offering whose meaning depends on
        // which end read it
        if fields.insert(unescape(k), unescape(v)).is_some() {
            return Err(Refused::Field("", format!("`{k}` is given twice")));
        }
    }
    let given = fields.get("v").map(String::as_str).unwrap_or("");
    if given != VERSION {
        return Err(Refused::Version(given.to_string()));
    }
    let want = |name: &'static str| -> Result<String, Refused> {
        match fields.get(name) {
            Some(v) if !v.is_empty() => Ok(v.clone()),
            _ => Err(Refused::Field(name, "an offering names this".into())),
        }
    };

    let name = want("name")?;
    if name.chars().count() > MAX_NAME {
        return Err(Refused::Field(
            "name",
            format!("at most {MAX_NAME} characters"),
        ));
    }
    // **a label a person reads carries no control characters**, which is
    // what would let one line of it look like another
    if name.chars().any(|c| c.is_control()) {
        return Err(Refused::Field("name", "a label is one line of text".into()));
    }

    let image = want("image")?;
    if image.len() > 512 || image.starts_with('-') || !image.bytes().all(image_byte) {
        return Err(Refused::Field(
            "image",
            "a registry path and a tag or digest".into(),
        ));
    }

    let authority = want("authority")?;
    if authority.len() > 253 || !authority.bytes().all(authority_byte) {
        return Err(Refused::Field(
            "authority",
            "lower-case letters, digits, dots and dashes, with no port and no path".into(),
        ));
    }

    let port = match fields.get("port").map(String::as_str) {
        None | Some("") => None,
        Some(p) => match p.parse::<u16>() {
            Ok(n) if n > 0 => Some(n),
            _ => return Err(Refused::Field("port", "a number from 1 to 65535".into())),
        },
    };

    let manifest = {
        let encoded = want("manifest")?;
        let bytes =
            unbase64url(&encoded).ok_or(Refused::Field("manifest", "not base64url".into()))?;
        if bytes.len() > MAX_MANIFEST {
            return Err(Refused::Field(
                "manifest",
                format!("at most {MAX_MANIFEST} bytes"),
            ));
        }
        // **it is text the instance will parse as its own**, so it must be
        // text: a manifest that is not UTF-8 would be refused there, and
        // meeting that here is meeting it where the person is standing
        String::from_utf8(bytes).map_err(|_| Refused::Field("manifest", "not text".into()))?
    };

    Ok(Offering {
        name,
        image,
        authority,
        port,
        manifest,
    })
}

/// **Compose one**, which is what a distributor does — and what this
/// project does for its own test application, so the format has one
/// definition and the two ends cannot drift.
pub fn compose(o: &Offering) -> String {
    let mut out = format!("{PREFIX}?v={VERSION}");
    let mut add = |k: &str, v: &str| {
        out.push('&');
        out.push_str(k);
        out.push('=');
        out.push_str(&escape(v));
    };
    add("name", &o.name);
    add("image", &o.image);
    add("authority", &o.authority);
    if let Some(p) = o.port {
        add("port", &p.to_string());
    }
    add("manifest", &base64url(o.manifest.as_bytes()));
    out
}

fn image_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"./:@_-".contains(&b)
}

fn authority_byte(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-'
}

/// Percent-escaping, leaving the unreserved set alone.
fn escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Percent-decoding, and `+` for a space as a form's own decoder reads it.
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < b.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(v) => {
                    out.push(v);
                    i += 3;
                }
                Err(_) => {
                    out.push(b'%');
                    i += 1;
                }
            },
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// base64url without padding, which is what survives a query string and a
/// code's alphanumeric mode alike.
fn base64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let take = chunk.len() + 1;
        for i in 0..take {
            out.push(ALPHABET[((n >> (18 - 6 * i)) & 0x3f) as usize] as char);
        }
    }
    out
}

/// And back, refusing a character that is not in the alphabet and a
/// length that cannot be one.
fn unbase64url(text: &str) -> Option<Vec<u8>> {
    let mut bits: Vec<u8> = Vec::with_capacity(text.len());
    for c in text.bytes() {
        bits.push(ALPHABET.iter().position(|a| *a == c)? as u8);
    }
    if bits.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(bits.len() / 4 * 3);
    for chunk in bits.chunks(4) {
        let mut n: u32 = 0;
        for (i, v) in chunk.iter().enumerate() {
            n |= u32::from(*v) << (18 - 6 * i);
        }
        for i in 0..chunk.len() - 1 {
            out.push(((n >> (16 - 8 * i)) & 0xff) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one() -> Offering {
        Offering {
            name: "The utility test application".into(),
            image: "ggcr.io/mzuech/fuerosnet/rhtn-testapp:abc12345".into(),
            authority: "testapp.internal".into(),
            port: Some(7460),
            manifest: "roles = [\"reader\"]\n".into(),
        }
    }

    // acceptance: PRD-21
    #[test]
    fn an_offering_composes_and_reads_back_as_it_was() {
        let o = one();
        let link = compose(&o);
        assert!(link.starts_with("fueros://install?v=1&"), "{link}");
        assert_eq!(parse(&link), Ok(o));
        // and a code read with whitespace around it is the same offering
        assert_eq!(parse(&format!("  {link}\n")).unwrap(), one());
    }

    // acceptance: PRD-21
    #[test]
    fn an_offering_carries_no_identity_no_owner_and_no_key() {
        // **what the format cannot say** is the half that keeps a link
        // from being an install somebody else composed: these names are
        // not fields, so an offering that carried them carries them into
        // nothing
        let link = compose(&one());
        for cannot in ["resource=", "owner=", "key=", "instance="] {
            assert!(!link.contains(cannot), "{cannot} is in {link}");
        }
        // and a link that names them anyway is read without them
        let o = parse(&format!(
            "{link}&resource={}&owner={}&key=secret",
            "aa".repeat(32),
            "bb".repeat(32)
        ))
        .expect("the unknown fields are ignored, not obeyed");
        assert_eq!(o, one());
    }

    // acceptance: PRD-21
    #[test]
    fn a_version_is_checked_before_any_field_is_read() {
        let link = compose(&one()).replace("v=1", "v=2");
        assert_eq!(parse(&link), Err(Refused::Version("2".into())));
        assert!(parse(&link).unwrap_err().say().contains("newer client"));
        // and an offering with no version at all
        let bare = format!("{PREFIX}?name=x&image=a/b:1&authority=a.b&manifest=cm9sZXM");
        assert_eq!(parse(&bare), Err(Refused::Version(String::new())));
    }

    // acceptance: PRD-21
    #[test]
    fn every_field_is_bounded_and_a_malformed_one_is_refused_where_the_person_is() {
        let ok = compose(&one());
        let bad = |s: String| parse(&s).expect_err("refused");

        assert_eq!(bad("https://example.test/x".into()), Refused::NotAnOffering);
        assert_eq!(
            bad(format!("{PREFIX}somethingelse?v=1")),
            Refused::NotAnOffering
        );
        assert_eq!(bad("x".repeat(MAX_OFFERING + 1)), Refused::TooLong);

        // a label of more than one line, and one too long
        let long = compose(&Offering {
            name: "a".repeat(MAX_NAME + 1),
            ..one()
        });
        assert!(matches!(bad(long), Refused::Field("name", _)));
        let lines = compose(&Offering {
            name: "two\nlines".into(),
            ..one()
        });
        assert!(matches!(bad(lines), Refused::Field("name", _)));

        // an image that is a flag, and an authority with a path
        for (field, value) in [("image", "-rm"), ("authority", "shop/internal")] {
            let mut o = one();
            match field {
                "image" => o.image = value.into(),
                _ => o.authority = value.into(),
            }
            let said = bad(compose(&o));
            assert!(
                matches!(&said, Refused::Field(f, _) if *f == field),
                "{said:?}"
            );
        }

        // a port that is not one
        assert!(matches!(
            bad(ok.replace("port=7460", "port=0")),
            Refused::Field("port", _)
        ));
        assert!(matches!(
            bad(ok.replace("port=7460", "port=70000")),
            Refused::Field("port", _)
        ));

        // a manifest that is not base64url, and one over the bound
        assert!(matches!(
            parse(&ok.replace("&manifest=", "&manifest=!!!")).unwrap_err(),
            Refused::Field("manifest", _)
        ));
        let big = compose(&Offering {
            manifest: "x".repeat(MAX_MANIFEST + 1),
            ..one()
        });
        assert!(matches!(bad(big), Refused::Field("manifest", _)));

        // **a field given twice is refused**, not resolved
        assert!(matches!(
            bad(format!("{ok}&image=other/thing:1")),
            Refused::Field(_, _)
        ));
    }

    // acceptance: PRD-21
    #[test]
    fn a_port_is_optional_and_the_manifest_survives_every_byte() {
        let mut o = one();
        o.port = None;
        let link = compose(&o);
        assert!(!link.contains("port="), "{link}");
        assert_eq!(parse(&link).unwrap().port, None);

        // the manifest is TOML with newlines, quotes, brackets and
        // equals: a format that mangled one would relay a declaration
        // the package did not make
        o.manifest = "roles = [\"reader\", \"writer\"]\n[budget]\nblock_bytes = 65536\n".into();
        let round = parse(&compose(&o)).unwrap();
        assert_eq!(round.manifest, o.manifest);

        // and every byte value round-trips through the encoding
        let all: Vec<u8> = (0u8..=255).collect();
        assert_eq!(unbase64url(&base64url(&all)).unwrap(), all);
        for n in 0..8usize {
            let bytes = &all[..n];
            assert_eq!(unbase64url(&base64url(bytes)).unwrap(), bytes);
        }
    }
}
