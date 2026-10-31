//! **What a resource may ask of the client, and what goes back**
//! (`light-client-requirements.md` §8.1; `resource-requirements.md` §3.4).
//!
//! A resource's page is framed so that it reaches nothing of the person's
//! (§8): no script, no bridge. So anything that needs a device capability
//! under the person's consent — a scan, a photograph with provenance, a
//! place, a tag read, a signature — cannot be the page's to take. It is
//! asked for instead: **the resource answers a request with one typed
//! ask**, under [`MEDIA`], naming the utility, what it is for, how long
//! the result is kept and where the answer should go; **the kernel reads
//! it once** here, puts it to the person as a question, and when they
//! give or decline **composes the answer itself** and sends it back to
//! the resource that asked, on the same path every other exchange takes.
//!
//! **The audience is the resource that asked**, never one a resource
//! names: the answer is addressed to it through the node that served it,
//! and where a result is signed it is signed under a key derived for
//! that one audience ([`application_key`]) — a per-device key with no
//! relation to the network identity, so that a resource holding a
//! claim cannot tie it to anything the person is elsewhere.
//!
//! **A refusal is an answer the application sees**, with a reason, never
//! a silence it has to interpret or an error loop (`resource-requirements.md`
//! §3.4). And nothing here widens what the resource layer can see: what
//! goes back is user-submitted application data, exactly as a form
//! carried by the client would be.

use rhtn_codec::cose::sha256;
use rhtn_codec::encode::*;
use rhtn_model::Keyhash;
use zeroize::Zeroizing;

/// The media type a resource's ask is declared under.  **Nothing else is
/// read as one**: an undeclared body is never inspected to find out
/// whether it might be.
pub const MEDIA: &str = "application/vnd.rhtn.utility+json";

/// The tag a claim's signed array opens with.
pub const CLAIM_TAG: &str = "rhtn/utility@1";

/// The most an ask may be, as bytes.  An ask names things; it does not
/// carry them.
pub const ASK_BYTES: usize = 16_384;

/// The most a purpose or a retention sentence may be, in bytes: it is
/// shown to a person whole, and a wall of text is how a consent screen
/// is made to say nothing.
pub const WORDS_BYTES: usize = 280;

/// The most a token may be: `[A-Za-z0-9._-]`, the resource's own name for
/// its ask, echoed back so it can tell which one was answered.
pub const TOKEN_BYTES: usize = 64;

/// The most a return path may be.
pub const PATH_BYTES: usize = 256;

/// The most an exact payload to be signed may be.
pub const PAYLOAD_BYTES: usize = 65_536;

/// The most a result may be.  **Bounded by §11's message bound**
/// (`wire-format.md` §11): a request larger than a frame travels as a
/// sequence of them, and the whole is what this leaves room in beside
/// its headers; a capture larger than this is the client's to shrink
/// before it is offered.
pub const RESULT_BYTES: usize = rhtn_codec::bounds::RESOURCE_MESSAGE_BYTES - 4096;

/// The most asks held unanswered at once; the oldest goes when another
/// arrives.  An ask is in-flight state, bound to a screen: nothing of it
/// outlives the process.
pub const PENDING: usize = 8;

/// The utilities a resource may ask for — the vocabulary, closed, so that
/// a consent screen is one per kind and never one per application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Utility {
    /// A document scanned: the camera, straightened and read, as a PDF
    /// or an image.
    Scan,
    /// A photograph or a video taken now, with the time it was taken and
    /// a claim under the application key that this device took it.
    Capture,
    /// A place, as a geohash at the precision the resource asked and the
    /// person agreed to, sent once.
    Location,
    /// Presence at a place, read from a tag on a proximity channel.
    CheckIn,
    /// A tag read: a code on a thing.
    Tag,
    /// A signature over an exact payload, under the application key,
    /// authorised by the person — the platform's own unlock where it
    /// has one.  **An application signature and never a ceremony
    /// claim**: the key is not the identity's.
    Sign,
    /// A live session: capture pushed to a service the resource names,
    /// for a stated duration, with its own indicator.
    Live,
    /// Standing permission to be shown this resource's notifications.
    Notify,
    /// A copy of what the resource holds, handed to the person as a
    /// file.
    Export,
    /// A file the person picks, handed to the resource.
    Import,
}

impl Utility {
    /// Every utility, in the order a document lists them.
    pub const ALL: [Utility; 10] = [
        Utility::Scan,
        Utility::Capture,
        Utility::Location,
        Utility::CheckIn,
        Utility::Tag,
        Utility::Sign,
        Utility::Live,
        Utility::Notify,
        Utility::Export,
        Utility::Import,
    ];

    /// The name an ask carries.
    pub fn name(self) -> &'static str {
        match self {
            Utility::Scan => "scan",
            Utility::Capture => "capture",
            Utility::Location => "location",
            Utility::CheckIn => "check-in",
            Utility::Tag => "tag",
            Utility::Sign => "sign",
            Utility::Live => "live",
            Utility::Notify => "notify",
            Utility::Export => "export",
            Utility::Import => "import",
        }
    }

    /// The utility a name names, or none: an unknown name is an ask the
    /// kernel cannot put to anyone, and is refused as such.
    pub fn of(name: &str) -> Option<Utility> {
        Utility::ALL.into_iter().find(|u| u.name() == name)
    }

    /// Whether a given result goes back under a claim: a capture carries
    /// provenance and a signature is one.  Nothing else is signed —
    /// a place, a tag, a file say nothing about who produced them, and a
    /// claim that they did would be one nobody made.
    pub fn signs(self) -> bool {
        matches!(self, Utility::Capture | Utility::Sign)
    }
}

/// An ask, as the resource wrote it and the kernel read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// Which utility.
    pub utility: Utility,
    /// What it is for, in the resource's words, shown whole.
    pub purpose: String,
    /// How long the resource keeps what it gets, in the resource's
    /// words, shown whole.  **A statement, not a guarantee**: the client
    /// shares no state with the resource and cannot enforce it, and
    /// shows it as what the resource said.
    pub retention: String,
    /// Where the answer goes: a path on the same resource.
    pub return_path: String,
    /// The resource's own name for this ask, echoed back.
    pub token: String,
    /// `sign`: the exact bytes to be approved and signed.
    pub payload: Option<Vec<u8>>,
    /// `location`: the geohash length asked for, 1 to 12.
    pub precision: Option<u8>,
    /// `scan`, `capture`, `import`: the media types the resource takes.
    pub media: Vec<String>,
    /// `check-in`, `tag`: the channel the tag is on — `nfc`, `uwb`,
    /// `qr` — where the resource minds.
    pub channel: Option<String>,
    /// `live`: the seconds asked for.
    pub seconds: Option<u64>,
}

/// Why an ask was not taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Malformed {
    /// Larger than an ask may be.
    TooLarge,
    /// Not a JSON object.
    NotAnObject,
    /// A field absent, of the wrong type, out of bounds or unknown, by
    /// name.
    Field(&'static str),
    /// A utility this kernel has no question for.
    Unknown(String),
}

impl Malformed {
    /// The refusal in a sentence, for a shell to show.
    pub fn say(&self) -> String {
        match self {
            Malformed::TooLarge => format!("the ask is larger than {ASK_BYTES} bytes"),
            Malformed::NotAnObject => "the ask is not a JSON object".into(),
            Malformed::Field(f) => {
                format!("the ask's `{f}` is absent, ill-formed or out of bounds")
            }
            Malformed::Unknown(u) => {
                format!("the ask names a utility this client has none of: {u}")
            }
        }
    }
}

/// Read one ask.  **Reject rather than guess**: every field has one
/// shape, and a field in another is the whole ask refused, since an ask
/// half-read is a question half-put.
pub fn parse(bytes: &[u8]) -> Result<Ask, Malformed> {
    if bytes.len() > ASK_BYTES {
        return Err(Malformed::TooLarge);
    }
    let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| Malformed::NotAnObject)?;
    let o = v.as_object().ok_or(Malformed::NotAnObject)?;
    let text = |k: &'static str, most: usize| -> Result<String, Malformed> {
        let s = o
            .get(k)
            .and_then(|x| x.as_str())
            .ok_or(Malformed::Field(k))?;
        if s.is_empty() || s.len() > most || s.chars().any(|c| c.is_control()) {
            return Err(Malformed::Field(k));
        }
        Ok(s.to_string())
    };
    let name = text("utility", 32)?;
    let utility = Utility::of(&name).ok_or(Malformed::Unknown(name))?;
    let purpose = text("ask", WORDS_BYTES)?;
    let retention = text("retention", WORDS_BYTES)?;
    let return_path = text("return", PATH_BYTES)?;
    // a path on the resource: rooted, no whitespace, and not the node's
    // own namespace, which the node would refuse in any case
    if !return_path.starts_with('/')
        || return_path.chars().any(|c| c.is_whitespace())
        || return_path.starts_with("/.rhtn/")
        || return_path == "/.rhtn"
    {
        return Err(Malformed::Field("return"));
    }
    let token = text("token", TOKEN_BYTES)?;
    if !token
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
    {
        return Err(Malformed::Field("token"));
    }
    let payload = match o.get("payload") {
        None => None,
        Some(x) => {
            let s = x.as_str().ok_or(Malformed::Field("payload"))?;
            let b = unhex(s).ok_or(Malformed::Field("payload"))?;
            if b.len() > PAYLOAD_BYTES {
                return Err(Malformed::Field("payload"));
            }
            Some(b)
        }
    };
    if utility == Utility::Sign && payload.is_none() {
        return Err(Malformed::Field("payload"));
    }
    let precision = match o.get("precision") {
        None => None,
        Some(x) => {
            let n = x.as_u64().ok_or(Malformed::Field("precision"))?;
            if !(1..=12).contains(&n) {
                return Err(Malformed::Field("precision"));
            }
            Some(n as u8)
        }
    };
    if utility == Utility::Location && precision.is_none() {
        return Err(Malformed::Field("precision"));
    }
    let media = match o.get("media") {
        None => Vec::new(),
        Some(x) => {
            let a = x.as_array().ok_or(Malformed::Field("media"))?;
            if a.len() > 16 {
                return Err(Malformed::Field("media"));
            }
            a.iter()
                .map(|m| {
                    let s = m.as_str().ok_or(Malformed::Field("media"))?;
                    if s.is_empty() || s.len() > 128 || !s.contains('/') {
                        return Err(Malformed::Field("media"));
                    }
                    Ok(s.to_ascii_lowercase())
                })
                .collect::<Result<_, _>>()?
        }
    };
    let channel = match o.get("channel") {
        None => None,
        Some(x) => {
            let s = x.as_str().ok_or(Malformed::Field("channel"))?;
            if !matches!(s, "nfc" | "uwb" | "qr") {
                return Err(Malformed::Field("channel"));
            }
            Some(s.to_string())
        }
    };
    let seconds = match o.get("seconds") {
        None => None,
        Some(x) => Some(x.as_u64().ok_or(Malformed::Field("seconds"))?),
    };
    Ok(Ask {
        utility,
        purpose,
        retention,
        return_path,
        token,
        payload,
        precision,
        media,
        channel,
        seconds,
    })
}

/// What the person did with an ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Declined, with why in a word or a sentence: the person said no,
    /// or this device has no such utility.
    Declined(String),
    /// A result as bytes under a media type: a scan, a capture, a file.
    Bytes {
        /// The declared type of what is given.
        media: String,
        /// The bytes.
        bytes: Vec<u8>,
    },
    /// A result as text: a geohash, what a tag said.
    Text(String),
    /// Approved and nothing to carry: a signature (the payload was the
    /// ask's), a live session, a standing permission, an export taken.
    Approved,
}

/// Why an outcome does not answer the ask it was offered for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// A result of a shape the utility does not produce.
    Shape,
    /// A result larger than [`RESULT_BYTES`].
    TooLarge,
    /// A geohash not at the precision asked, or not a geohash.
    Precision,
    /// A reason or a media type that is not plain text.
    Text,
}

impl Mismatch {
    /// In a sentence.
    pub fn say(&self) -> String {
        match self {
            Mismatch::Shape => "the result is not of the shape that utility produces".into(),
            Mismatch::TooLarge => format!("the result is larger than {RESULT_BYTES} bytes"),
            Mismatch::Precision => "the place is not a geohash at the precision asked".into(),
            Mismatch::Text => "a reason or a media type must be plain text".into(),
        }
    }
}

/// Whether `outcome` is an answer to `ask`.
pub fn fits(ask: &Ask, outcome: &Outcome) -> Result<(), Mismatch> {
    let plain = |s: &str, most: usize| {
        !s.is_empty() && s.len() <= most && !s.chars().any(|c| c.is_control())
    };
    match (ask.utility, outcome) {
        (_, Outcome::Declined(why)) => {
            if plain(why, WORDS_BYTES) {
                Ok(())
            } else {
                Err(Mismatch::Text)
            }
        }
        (
            Utility::Scan | Utility::Capture | Utility::Import | Utility::Tag,
            Outcome::Bytes { media, bytes },
        ) => {
            if !plain(media, 128) || !media.contains('/') {
                return Err(Mismatch::Text);
            }
            if bytes.len() > RESULT_BYTES {
                return Err(Mismatch::TooLarge);
            }
            Ok(())
        }
        (Utility::Location, Outcome::Text(t)) => {
            let want = usize::from(ask.precision.unwrap_or(0));
            if t.len() != want || !t.bytes().all(|c| GEOHASH.contains(&c)) {
                return Err(Mismatch::Precision);
            }
            Ok(())
        }
        (Utility::CheckIn | Utility::Tag, Outcome::Text(t)) => {
            if plain(t, RESULT_BYTES) {
                Ok(())
            } else {
                Err(Mismatch::Text)
            }
        }
        (Utility::Sign | Utility::Live | Utility::Notify | Utility::Export, Outcome::Approved) => {
            Ok(())
        }
        _ => Err(Mismatch::Shape),
    }
}

/// The geohash alphabet: base 32 without `a`, `i`, `l` and `o`.
const GEOHASH: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";

/// What a claim is over: the digest of the result bytes, of the text, or
/// — for a signature — of the exact payload the ask carried.
pub fn claimed(ask: &Ask, outcome: &Outcome) -> Option<[u8; 32]> {
    match outcome {
        Outcome::Bytes { bytes, .. } => Some(sha256(bytes)),
        Outcome::Text(t) => Some(sha256(t.as_bytes())),
        Outcome::Approved if ask.utility == Utility::Sign => ask.payload.as_deref().map(sha256),
        _ => None,
    }
}

/// The bytes a claim signs: a CBOR array `[tag, audience, utility, token,
/// digest, at]` — the tag [`CLAIM_TAG`], the resource asked (32 bytes),
/// the utility's name, the ask's token, the digest [`claimed`] names, and
/// the time in milliseconds.  **The audience is inside the signature**,
/// so a claim given to one resource says nothing when shown to another.
pub fn claim_tbs(
    audience: &Keyhash,
    utility: Utility,
    token: &str,
    digest: &[u8; 32],
    at: u64,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    emit_array_head(&mut out, 6);
    emit_tstr(&mut out, CLAIM_TAG);
    emit_bstr(&mut out, audience);
    emit_tstr(&mut out, utility.name());
    emit_tstr(&mut out, token);
    emit_bstr(&mut out, digest);
    emit_uint(&mut out, at);
    out
}

/// The tag [`application_key`] expands under.
pub const KEY_TAG: &[u8] = b"rhtn/utility@1:key";

/// **The application key for one audience**: an Ed25519 seed expanded
/// from the device's application secret and the resource's keyhash, so
/// each resource sees a key of its own and none can be matched to
/// another's.  The secret is the device's — minted at first need, kept
/// with the device's own state and never in the backup — so a device
/// restored from its phrase signs under a new key, which is the recovery
/// policy: an application key is not the identity and comes back with
/// nothing (`light-client-requirements.md` §8.1).
pub fn application_key(secret: &[u8; 32], audience: &Keyhash) -> Zeroizing<[u8; 32]> {
    use aws_lc_rs::hkdf;
    let mut info = Vec::with_capacity(KEY_TAG.len() + 32);
    info.extend_from_slice(KEY_TAG);
    info.extend_from_slice(audience);
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]).extract(secret);
    let mut out = Zeroizing::new([0u8; 32]);
    prk.expand(&[&info], hkdf::HKDF_SHA256)
        .expect("32 bytes is within HKDF's bound")
        .fill(&mut out[..])
        .expect("filled");
    out
}

/// A claim: the public key it verifies under and the signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// The application key's public half, for this audience.
    pub key: [u8; 32],
    /// The Ed25519 signature over [`claim_tbs`].
    pub signature: [u8; 64],
}

/// Sign a claim for `audience`.
pub fn claim(
    secret: &[u8; 32],
    audience: &Keyhash,
    utility: Utility,
    token: &str,
    digest: &[u8; 32],
    at: u64,
) -> Claim {
    let seed = application_key(secret, audience);
    let tbs = claim_tbs(audience, utility, token, digest, at);
    let (key, signature) = rhtn_crypto::identity::sign_ed_seed(&seed, &tbs);
    Claim { key, signature }
}

/// What a resource does with a claim it was handed: the same array, the
/// same key.  Here so the two halves cannot drift.
pub fn verify_claim(
    c: &Claim,
    audience: &Keyhash,
    utility: Utility,
    token: &str,
    digest: &[u8; 32],
    at: u64,
) -> bool {
    rhtn_crypto::verify::verify_ed_raw(
        &c.key,
        &c.signature,
        &claim_tbs(audience, utility, token, digest, at),
    )
}

/// **The answer, as the one HTTP/1.1 request the kernel composes**: a
/// `POST` to the ask's return path with a `multipart/form-data` body —
/// which is what a form the client carried would be, and what an
/// application already reads.  The parts, each once, in this order:
///
/// - `utility`, the name; `token`, echoed; `at`, milliseconds;
/// - `outcome`, `given` or `declined`, and `reason` where declined;
/// - `result`, where given: a file part under its media type for bytes,
///   a text part for text, absent for an approval;
/// - `key` and `claim`, hex, where the utility signs: the application
///   key's public half for this audience and the signature over
///   [`claim_tbs`], and `digest`, hex, what it is over.
///
/// `boundary` is the caller's random: it is checked against the body and
/// refused by [`Compose::Boundary`] where it occurs in it.
pub fn return_request(
    ask: &Ask,
    outcome: &Outcome,
    at: u64,
    claim: Option<(&[u8; 32], &Claim)>,
    boundary: &str,
) -> Result<Vec<u8>, Compose> {
    let mut parts: Vec<(&str, Option<&str>, Vec<u8>)> = Vec::new();
    parts.push(("utility", None, ask.utility.name().into()));
    parts.push(("token", None, ask.token.clone().into()));
    parts.push(("at", None, at.to_string().into()));
    match outcome {
        Outcome::Declined(why) => {
            parts.push(("outcome", None, b"declined".to_vec()));
            parts.push(("reason", None, why.clone().into()));
        }
        Outcome::Bytes { media, bytes } => {
            parts.push(("outcome", None, b"given".to_vec()));
            parts.push(("result", Some(media), bytes.clone()));
        }
        Outcome::Text(t) => {
            parts.push(("outcome", None, b"given".to_vec()));
            parts.push(("result", None, t.clone().into()));
        }
        Outcome::Approved => {
            parts.push(("outcome", None, b"given".to_vec()));
        }
    }
    if let Some((digest, c)) = claim {
        parts.push(("digest", None, hex(digest).into()));
        parts.push(("key", None, hex(&c.key).into()));
        parts.push(("claim", None, hex(&c.signature).into()));
    }
    let mut body = Vec::new();
    for (name, media, bytes) in &parts {
        body.extend_from_slice(b"--");
        body.extend_from_slice(boundary.as_bytes());
        body.extend_from_slice(b"\r\n");
        match media {
            Some(m) => {
                body.extend_from_slice(
                    format!("content-disposition: form-data; name=\"{name}\"; filename=\"result\"\r\ncontent-type: {m}\r\n\r\n").as_bytes(),
                );
            }
            None => {
                body.extend_from_slice(
                    format!("content-disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
                );
            }
        }
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b"--");
    body.extend_from_slice(boundary.as_bytes());
    body.extend_from_slice(b"--\r\n");
    // a boundary inside a part is two parsers disagreeing about where a
    // part ends: refused, and the caller draws another
    let delim = format!("\r\n--{boundary}");
    for (_, _, bytes) in &parts {
        if bytes.windows(delim.len()).any(|w| w == delim.as_bytes()) {
            return Err(Compose::Boundary);
        }
    }
    let head = format!(
        "POST {} HTTP/1.1\r\nhost: resource\r\ncontent-type: multipart/form-data; boundary={boundary}\r\ncontent-length: {}\r\n\r\n",
        ask.return_path,
        body.len()
    );
    let mut out = head.into_bytes();
    out.extend_from_slice(&body);
    if out.len() > rhtn_codec::bounds::RESOURCE_MESSAGE_BYTES {
        return Err(Compose::TooLarge);
    }
    Ok(out)
}

/// Why an answer could not be composed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compose {
    /// The boundary drawn occurs in a part; draw another.
    Boundary,
    /// The whole exceeds the message bound.
    TooLarge,
}

/// A boundary out of random bytes: `rhtn` and their hex.
pub fn boundary(random: &[u8; 12]) -> String {
    format!("rhtn{}", hex(random))
}

/// Lower-case hex.
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Bytes out of hex, or none where it is not hex of whole bytes.
pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect()
}
