//! **What the authority screen says** — who this key is under, and what
//! ending a line costs — in the phone's words (its `Leaving`), so the two
//! clients say one thing (`light-client-requirements.md` §6; UX-006)
//! [author, 2026-11-23: *"Subnet authority screen and renaming should carry
//! over"*].
//!
//! **Never more than was computed.** A cost that came to nothing and one
//! that could not be worked out both arrive as an empty list of losses and
//! mean opposite things, so [`computed`] decides which sentence leads, and
//! the limits are said whether or not anything is lost.

use crate::participant::escape;
use rhtn_model::{Cost, Limit, Line, Loss, LossReason};
use std::collections::HashMap;

/// How much of a keyhash stands for a party, the same in the row and on
/// the button, since they name the same party and the act is not undoable.
pub const SHORT: usize = 12;

/// **A party, named**: the alias this device was told, and the keyhash
/// with it [author, 2026-10-28] — both, never one.
pub fn party(who: &str, alias: Option<&str>) -> String {
    let short: String = who.chars().take(SHORT).collect();
    match alias.filter(|a| !a.trim().is_empty()) {
        Some(a) => format!("{a}, {short}…"),
        None => format!("{short}…"),
    }
}

/// One line this key holds, as the roll draws it.
pub fn line(l: &Line, alias: Option<&str>) -> String {
    format!("{}   series {}", party(&hex(&l.patron), alias), l.series)
}

/// What to say where this key is under no patron at all.
pub const NONE: &str = "This key is under no patron, so there is nothing to leave. \
     A patron adopts you at a meeting, which happens on a phone.";

/// **Whether the answer is an answer**: false where this device holds no
/// edge for the relationship in its own copy of the topology.
pub fn computed(c: &Cost) -> bool {
    !c.limits.iter().any(|l| matches!(l, Limit::NotInTable))
}

fn many(n: usize, one: &str, more: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {more}")
    }
}

fn verb<'a>(n: usize, one: &'a str, more: &'a str) -> &'a str {
    if n == 1 { one } else { more }
}

fn went(n: usize) -> String {
    format!(
        "{} {} your horizon",
        many(n, "person", "people"),
        verb(n, "leaves", "leave")
    )
}

/// The sentence the screen leads with.
pub fn headline(c: &Cost) -> String {
    if !computed(c) {
        "What leaving costs cannot be worked out on this device.".into()
    } else if c.losing.is_empty() && c.leaving.is_empty() {
        "Nothing this device can see goes with it.".into()
    } else if c.losing.is_empty() {
        format!(
            "No resource this device knows of goes, but {}.",
            went(c.leaving.len())
        )
    } else {
        format!(
            "{} {}, and {}.",
            many(c.losing.len(), "resource", "resources"),
            verb(c.losing.len(), "goes", "go"),
            went(c.leaving.len())
        )
    }
}

/// What is kept, where anything is and the answer is one.
pub fn kept(c: &Cost) -> Option<String> {
    (computed(c) && c.keeping > 0).then(|| {
        let n = c.keeping as usize;
        format!(
            "{} {} within reach.",
            many(n, "resource", "resources"),
            verb(n, "stays", "stay")
        )
    })
}

/// One resource that goes, and why: a host out of reach is nothing left
/// to ask; an owner gone is a door still there that will not open.
pub fn lost(r: &Loss, names: &HashMap<String, String>) -> String {
    let why = match r.reason {
        LossReason::HostLeaves => format!(
            "the node serving it leaves your horizon: {}",
            party(&hex(&r.node), names.get(&hex(&r.node)).map(String::as_str))
        ),
        LossReason::OwnerLeaves => format!(
            "its owner leaves your horizon: {} — the node stays, and admission is theirs to gate",
            party(
                &hex(&r.owner),
                names.get(&hex(&r.owner)).map(String::as_str)
            )
        ),
    };
    let named = if r.instance.trim().is_empty() {
        format!(
            "{}…",
            hex(&r.resource).chars().take(SHORT).collect::<String>()
        )
    } else {
        r.instance.clone()
    };
    format!("{named}   {}\n    {why}", r.service_type)
}

/// **Why the figure is a floor**, in the order that changes what a person
/// should believe.
pub fn limits(c: &Cost) -> Vec<String> {
    let mut out = Vec::new();
    if !computed(c) {
        out.push(LIMIT_UNCOMPUTED.to_string());
    }
    for l in &c.limits {
        if let Limit::SeveralLines { lines } = l {
            out.push(format!(
                "This relationship runs on {lines} lines in this device's copy, and one \
                 departure ends one of them. What is above is the cost of ending them all."
            ));
        }
    }
    let count = |f: fn(&Limit) -> bool| c.limits.iter().filter(|l| f(l)).count();
    let stale = count(|l| matches!(l, Limit::Stale { .. }));
    if stale > 0 {
        out.push(format!(
            "{} could not be refreshed when this device last asked, so what is counted \
             there is what was counted then.",
            many(stale, "node's list", "nodes' lists")
        ));
    }
    let cut = count(|l| matches!(l, Limit::Truncated { .. }));
    if cut > 0 {
        out.push(format!(
            "{} {} more than {} will list at once, so {} things this device has never seen.",
            many(cut, "node", "nodes"),
            verb(cut, "holds", "hold"),
            verb(cut, "it", "they"),
            verb(cut, "it serves", "they serve")
        ));
    }
    let unswept = count(|l| matches!(l, Limit::Unswept { .. }));
    if unswept > 0 {
        out.push(format!(
            "{} within reach {} never been asked what {}, so whatever that is, it is not \
             counted above.",
            many(unswept, "node", "nodes"),
            verb(unswept, "has", "have"),
            verb(unswept, "it serves", "they serve")
        ));
    }
    if c.limits
        .iter()
        .any(|l| matches!(l, Limit::PredicateNotHeld))
    {
        out.push(LIMIT_PREDICATE.to_string());
    }
    out
}

/// The whole warning, as the screen shows it, escaped for the page.
pub fn words(c: &Cost, names: &HashMap<String, String>) -> String {
    let mut parts = vec![headline(c)];
    if let Some(k) = kept(c) {
        parts.push(k);
    }
    if !c.losing.is_empty() {
        parts.push(String::new());
        parts.push(
            c.losing
                .iter()
                .map(|r| lost(r, names))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if !c.leaving.is_empty() && computed(c) {
        parts.push(String::new());
        parts.push(format!(
            "Out of your horizon: {}",
            c.leaving
                .iter()
                .map(|k| names.get(&hex(k)).cloned().unwrap_or_else(|| format!(
                    "{}…",
                    hex(k).chars().take(SHORT).collect::<String>()
                )))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let why = limits(c);
    if !why.is_empty() {
        parts.push(String::new());
        parts.push(
            why.iter()
                .map(|w| format!("· {w}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    escape(&parts.join("\n"))
}

/// **What the button says, and what the second press means**: leaving is a
/// right, not a request, so two presses because it cannot be undone.
pub fn button(who: &str, alias: Option<&str>, armed: bool) -> String {
    let name = alias
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}…", who.chars().take(SHORT).collect::<String>()));
    if armed {
        format!("Press again to leave {name}")
    } else {
        format!("Leave {name}")
    }
}

/// Said every time, because it is true every time.
pub const LIMIT_PREDICATE: &str = "Whether a resource admits you is its own rule, held where \
     it is hosted and not on this device: something still within reach may refuse you once \
     the line is gone. What is above is the least that goes, not the whole of it.";

/// Said where the topology this device holds has no edge to subtract.
pub const LIMIT_UNCOMPUTED: &str = "Nobody has told this device the shape of this relationship, \
     so there is nothing here to subtract and no figure to give. Connect to a serving node \
     and let it reach you before reading anything into an empty list.";

/// Why a person is on this screen at all.
pub const WHY: &str = "A patron is who placed you, and leaving one is yours alone to do: they \
     do not sign it and cannot refuse it. What it changes is what you can reach, so that is \
     worked out first.";

/// **What a paired desktop says in place of the act** [author, 2026-11-23:
/// show it, act on the phone]: it reads everything and signs nothing
/// (design §23.3), and leaving is signed.
pub const ON_THE_PHONE: &str = "Leaving is signed as the identity, and this desktop signs \
     nothing: leave on the phone.";

fn hex(b: &[u8]) -> String {
    crate::participant::hex(b)
}
