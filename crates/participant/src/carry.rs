//! What the instrument carries between processes.
//!
//! **The ceremony's own conversation has no encoding, deliberately.**
//! design §7 has it happen between two devices in each other's presence
//! over whatever channel they have — a screen and a camera, most often —
//! and `rhtn-ffi` carries it outward as fields for exactly that reason: a
//! shell moves it however the two devices manage and rebuilds it on the
//! other side. So a shell must choose, and this is this shell's choice.
//!
//! **It is not protocol and nothing should read it as one.** Two
//! instruments that agreed on a different encoding would interoperate with
//! each other and with nothing else, which is what §7 leaves open rather
//! than an omission to be closed here.
//!
//! One value is one token, because a command is a line split on spaces:
//! `|` separates fields, `,` separates a list, `;` separates lists of
//! lists, and `-` is an empty list. Nothing in it is binary — an
//! instrument is read by a person as often as by a script.

use rhtn_ffi::client::{Achieved, Intent, Proposed, Revealed, WitnessAsk, Witnessing};
use rhtn_ffi::types::{Channel, ChannelOutcome};

use crate::terminal::{hex, unhex};

/// A list, or `-` where it is empty.
fn list(parts: Vec<String>) -> String {
    if parts.is_empty() {
        "-".into()
    } else {
        parts.join(",")
    }
}

fn unlist(s: &str) -> Vec<&str> {
    if s == "-" {
        Vec::new()
    } else {
        s.split(',').collect()
    }
}

fn field<'a>(f: &[&'a str], i: usize, what: &str) -> Result<&'a str, String> {
    f.get(i)
        .copied()
        .ok_or_else(|| format!("{what} is missing field {}", i + 1))
}

fn num(s: &str, what: &str) -> Result<u64, String> {
    s.parse().map_err(|_| format!("`{s}` is not a {what}"))
}

fn bytes(s: &str, what: &str) -> Result<Vec<u8>, String> {
    unhex(s).ok_or_else(|| format!("{what} is not hex: `{s}`"))
}

fn ids(s: &str, what: &str) -> Result<Vec<Vec<u8>>, String> {
    unlist(s).iter().map(|x| bytes(x, what)).collect()
}

// ------------------------------------------------------------- intent

pub fn pack_intent(i: &Intent) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        hex(&i.contribution),
        list(i.nominees.iter().map(|n| hex(n)).collect()),
        list(i.bundle.iter().map(|b| hex(b)).collect()),
        i.started_at,
        i.retention_years,
        u8::from(i.initiator)
    )
}

pub fn take_intent(s: &str) -> Result<Intent, String> {
    let f: Vec<&str> = s.split('|').collect();
    Ok(Intent {
        contribution: bytes(field(&f, 0, "an intent")?, "a contribution")?,
        nominees: ids(field(&f, 1, "an intent")?, "a nominee")?,
        bundle: ids(field(&f, 2, "an intent")?, "a bundle entry")?,
        started_at: num(field(&f, 3, "an intent")?, "time")?,
        retention_years: num(field(&f, 4, "an intent")?, "count of years")?,
        initiator: field(&f, 5, "an intent")? == "1",
    })
}

// ----------------------------------------------------------- channels

fn channel_name(c: Channel) -> &'static str {
    match c {
        Channel::Uwb => "uwb",
        Channel::Nfc => "nfc",
        Channel::Optical => "optical",
        Channel::Latency => "latency",
    }
}

fn channel_of(s: &str) -> Result<Channel, String> {
    match s {
        "uwb" => Ok(Channel::Uwb),
        "nfc" => Ok(Channel::Nfc),
        "optical" => Ok(Channel::Optical),
        "latency" => Ok(Channel::Latency),
        _ => Err(format!("`{s}` is not a channel")),
    }
}

fn outcome_name(o: ChannelOutcome) -> &'static str {
    match o {
        ChannelOutcome::Pass => "pass",
        ChannelOutcome::Fail => "fail",
        ChannelOutcome::Unavailable => "unavailable",
    }
}

fn outcome_of(s: &str) -> Result<ChannelOutcome, String> {
    match s {
        "pass" => Ok(ChannelOutcome::Pass),
        "fail" => Ok(ChannelOutcome::Fail),
        "unavailable" => Ok(ChannelOutcome::Unavailable),
        _ => Err(format!("`{s}` is not an outcome")),
    }
}

pub fn pack_channels(a: &[Achieved]) -> String {
    list(
        a.iter()
            .map(|x| {
                format!(
                    "{}:{}:{}",
                    channel_name(x.channel),
                    outcome_name(x.outcome),
                    x.resolution_m.map_or("-".into(), |m| m.to_string())
                )
            })
            .collect(),
    )
}

pub fn take_channels(s: &str) -> Result<Vec<Achieved>, String> {
    unlist(s)
        .iter()
        .map(|one| {
            let p: Vec<&str> = one.split(':').collect();
            Ok(Achieved {
                channel: channel_of(field(&p, 0, "a channel")?)?,
                outcome: outcome_of(field(&p, 1, "a channel")?)?,
                resolution_m: match field(&p, 2, "a channel")? {
                    "-" => None,
                    m => Some(num(m, "count of metres")?),
                },
            })
        })
        .collect()
}

// ------------------------------------------------------- witness ask

pub fn pack_ask(a: &WitnessAsk) -> String {
    format!(
        "{}|{}|{}|{}",
        hex(&a.ceremony),
        list(a.participants.iter().map(|p| hex(p)).collect()),
        a.started_at,
        pack_channels(&a.channels)
    )
}

pub fn take_ask(s: &str) -> Result<WitnessAsk, String> {
    let f: Vec<&str> = s.split('|').collect();
    Ok(WitnessAsk {
        ceremony: bytes(field(&f, 0, "an ask")?, "a ceremony id")?,
        participants: ids(field(&f, 1, "an ask")?, "a participant")?,
        started_at: num(field(&f, 2, "an ask")?, "time")?,
        channels: take_channels(field(&f, 3, "an ask")?)?,
    })
}

// ---------------------------------------------------------- proposal

pub fn pack_proposed(p: &Proposed) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        p.started_at,
        p.finalized_at,
        list(p.participants.iter().map(|x| hex(x)).collect()),
        list(
            p.witnesses
                .iter()
                .map(|w| format!("{}:{}:{}", hex(&w.witness), hex(&w.nominated_by), w.flags))
                .collect()
        ),
        list(p.responses.iter().map(|r| hex(r)).collect()),
        hex(&p.root)
    )
}

/// `<witness>:<nominator>:<flags>` a piece, which is what a nominee's
/// answer comes back as and what a proposal is made from.
pub fn take_witnesses(s: &str) -> Result<Vec<Witnessing>, String> {
    unlist(s)
        .iter()
        .map(|one| {
            let p: Vec<&str> = one.split(':').collect();
            Ok(Witnessing {
                witness: bytes(field(&p, 0, "a witness")?, "a witness")?,
                nominated_by: bytes(field(&p, 1, "a witness")?, "a nominator")?,
                flags: num(field(&p, 2, "a witness")?, "flag set")?,
            })
        })
        .collect()
}

pub fn take_proposed(s: &str) -> Result<Proposed, String> {
    let f: Vec<&str> = s.split('|').collect();
    let witnesses = take_witnesses(field(&f, 3, "a proposal")?);
    Ok(Proposed {
        started_at: num(field(&f, 0, "a proposal")?, "time")?,
        finalized_at: num(field(&f, 1, "a proposal")?, "time")?,
        participants: ids(field(&f, 2, "a proposal")?, "a participant")?,
        witnesses: witnesses?,
        responses: ids(field(&f, 4, "a proposal")?, "a response")?,
        root: bytes(field(&f, 5, "a proposal")?, "a disclosure root")?,
    })
}

// ------------------------------------------------------- disclosures

pub fn pack_revealed(set: &[Revealed]) -> String {
    list(
        set.iter()
            .map(|r| format!("{}:{}:{}", r.label, hex(&r.salt), hex(&r.value)))
            .collect(),
    )
}

pub fn take_revealed(s: &str) -> Result<Vec<Revealed>, String> {
    unlist(s)
        .iter()
        .map(|one| {
            let p: Vec<&str> = one.split(':').collect();
            Ok(Revealed {
                label: field(&p, 0, "a disclosure")?.to_string(),
                salt: bytes(field(&p, 1, "a disclosure")?, "a salt")?,
                value: bytes(field(&p, 2, "a disclosure")?, "a value")?,
            })
        })
        .collect()
}

// ------------------------------------------------------ back-pointers

/// One list per signer, in signer order.
pub fn pack_back(back: &[Vec<Vec<u8>>]) -> String {
    if back.is_empty() {
        return "-".into();
    }
    back.iter()
        .map(|one| list(one.iter().map(|t| hex(t)).collect()))
        .collect::<Vec<_>>()
        .join(";")
}

pub fn take_back(s: &str) -> Result<Vec<Vec<Vec<u8>>>, String> {
    if s == "-" {
        return Ok(Vec::new());
    }
    s.split(';')
        .map(|one| ids(one, "a transaction id"))
        .collect()
}

/// One signer and what it signed.
pub type Entry = (Vec<u8>, Vec<u8>);

/// `<keyhash>:<signature>` pairs, one per signer, in signer order.
pub fn take_entries(s: &str) -> Result<Vec<Entry>, String> {
    unlist(s)
        .iter()
        .map(|one| {
            let p: Vec<&str> = one.split(':').collect();
            Ok((
                bytes(field(&p, 0, "an entry")?, "a signer")?,
                bytes(field(&p, 1, "an entry")?, "a signature")?,
            ))
        })
        .collect()
}
