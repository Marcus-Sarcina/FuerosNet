//! Diagnostic helpers for the field-test build
//! (`Robot/field-test-diagnostics.md` is the plan; its section 2 is the
//! redaction rule this module exists to make easy to follow).
//!
//! **Helpers, not a framework.** The events are `tracing` events, raised at
//! the sites the plan's section 3 lists; what this module adds is the
//! constructors an event's fields are built from, so that an identity, a
//! count or a duration is formatted one way everywhere and a secret has no
//! constructor at all. An event names variants, counts, sizes, durations,
//! step names and the reason strings the code already produces. It never
//! formats a `Msg`, a `KeyGrant`, a `Capture`, a `SealedCapture`, a
//! `ClientStore`, a backup's `Contents`, `PayloadKeys`, a `OneTimePair`, a
//! `Prefetched` bundle, an `InitialMessage`, anything `Zeroizing`, nor any
//! field named seed, key, template, profile, pixels, frames, ciphertext,
//! consent or provider; `tests/diag.rs` runs a whole harness ceremony under
//! a collector and asserts that no byte run of any secret made there
//! appears in any rendered event.
//!
//! **Compiled out of a releasable build.** The `releasable` feature, the
//! default, sets `tracing/max_level_off`, under which every `event!` is a
//! comparison against a constant and the optimiser removes the body, the
//! field expressions (an `id8` is never even formatted) and the callsite
//! string with it. The field-test flavour, `--no-default-features
//! --features fieldtest`, keeps them and adds [`json`], the renderer the
//! FFI's `Diagnostics` object receives lines from.

use std::fmt;
use std::sync::OnceLock;
use std::time::Instant;

/// The first eight hex characters of a 32-byte identifier: a keyhash, a
/// txid, a query id or a ceremony id. Enough to follow one party through
/// one bundle, which is the plan's declared and accepted linkability for a
/// test build (section 7), and not the identifier.
#[must_use]
pub fn id8(k: &[u8; 32]) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// A count, as an event field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Count(pub usize);

impl fmt::Display for Count {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A duration in milliseconds, as an event field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ms(pub u64);

impl fmt::Display for Ms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<std::time::Duration> for Ms {
    fn from(d: std::time::Duration) -> Ms {
        Ms(d.as_millis() as u64)
    }
}

/// A size in bytes, as an event field: how much went by, never what.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bytes(pub usize);

impl fmt::Display for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

static STARTED: OnceLock<Instant> = OnceLock::new();

/// Milliseconds since this process first asked, which the field-test
/// installer does at start (plan, section 3: durations are milliseconds
/// since process start, and the bundle header carries the one wall-clock
/// anchor).
#[must_use]
pub fn since_start() -> Ms {
    Ms(STARTED.get_or_init(Instant::now).elapsed().as_millis() as u64)
}

/// The name of a payload kind (`payload::KIND_*`), for an event field.
#[must_use]
pub fn kind_name(kind: u64) -> &'static str {
    use crate::payload::*;
    match kind {
        KIND_APPLICATION => "application",
        KIND_KEY_GRANT => "key_grant",
        KIND_LATE_RESPONSE => "late_response",
        KIND_CANDIDATES => "candidates",
        KIND_RESPONSE_COPY => "response_copy",
        KIND_ARCHIVE_REQUEST => "archive_request",
        KIND_ARCHIVE_REPLY => "archive_reply",
        KIND_QUERY => "query",
        KIND_RESPONSE => "response",
        KIND_CONSENT_REQUEST => "consent_request",
        KIND_CONSENT_REPLY => "consent_reply",
        KIND_FISHING_PROPOSAL => "fishing_proposal",
        KIND_WITNESS_REQUEST => "witness_request",
        KIND_WITNESS_ANSWER => "witness_answer",
        KIND_GATHERED_RESPONSES => "gathered_responses",
        KIND_BACK_POINTERS => "back_pointers",
        KIND_PROPOSED_BODY => "proposed_body",
        KIND_SIGNING_REPLY => "signing_reply",
        KIND_RECORD => "record",
        _ => "unknown",
    }
}

/// An [`Abort`](crate::ceremony::Abort) as an event field: the variant,
/// and the reason string where it carries one. Written out rather than
/// derived, because `Debug` of the enum would print the 32 bytes of
/// `NoRelationship`'s patron whole.
#[must_use]
pub fn abort(a: &crate::ceremony::Abort) -> String {
    use crate::ceremony::Abort::*;
    match a {
        Malformed(s) | Record(s) | PatronRefused(s) | Payload(s) => {
            format!("{}({s})", variant(a))
        }
        Refused(r) => format!("Refused({r:?})"),
        NotMine(s) | Waiting(s) => format!("{}({s})", variant(a)),
        NoRelationship(k) => format!("NoRelationship({})", id8(k)),
        _ => variant(a).to_string(),
    }
}

/// The variant name of an [`Abort`](crate::ceremony::Abort).
#[must_use]
pub fn variant(a: &crate::ceremony::Abort) -> &'static str {
    use crate::ceremony::Abort::*;
    match a {
        Declined => "Declined",
        NotActive => "NotActive",
        NoCeremonyId => "NoCeremonyId",
        ClockFar => "ClockFar",
        NoProximity => "NoProximity",
        ChannelDisagreement => "ChannelDisagreement",
        ContributionMismatch => "ContributionMismatch",
        CeremonyIdMismatch => "CeremonyIdMismatch",
        Malformed(_) => "Malformed",
        TemplateLength => "TemplateLength",
        Refused(_) => "Refused",
        RootMismatch => "RootMismatch",
        BackPointers => "BackPointers",
        NoWitness => "NoWitness",
        Record(_) => "Record",
        NoPriorKey => "NoPriorKey",
        NoSeed => "NoSeed",
        NotMine(_) => "NotMine",
        NoRelationship(_) => "NoRelationship",
        CounterExhausted => "CounterExhausted",
        NotRecognised => "NotRecognised",
        PatronRefused(_) => "PatronRefused",
        NoCounterparty => "NoCounterparty",
        InitiatorClaim => "InitiatorClaim",
        Waiting(_) => "Waiting",
        NotProposer => "NotProposer",
        Payload(_) => "Payload",
    }
}

/// The variant name of a [`Notice`](crate::notice::Notice), for an event
/// field; the identities some variants carry stay out of it.
#[must_use]
pub fn notice(n: &crate::notice::Notice) -> &'static str {
    use crate::notice::Notice::*;
    match n {
        RecordDisclosure { .. } => "RecordDisclosure",
        QuerySurfaced { .. } => "QuerySurfaced",
        ProbingRefused { .. } => "ProbingRefused",
        NomineesOutnumbered { .. } => "NomineesOutnumbered",
        NoCandidateRecognised => "NoCandidateRecognised",
        UnrecognisedDeclaration { .. } => "UnrecognisedDeclaration",
        PayloadUnattributable { .. } => "PayloadUnattributable",
    }
}

/// What a received payload came to, as an event field: the
/// [`Dispatched`](crate::ceremony::Dispatched) variant and its outcome,
/// with identifiers truncated and bytes counted. `Debug` of the enum would
/// print application payload and candidates whole.
#[must_use]
pub fn dispatched(d: &crate::ceremony::Dispatched) -> String {
    use crate::ceremony::Dispatched::*;
    use crate::verifier::{GrantOutcome, QueryOutcome};
    match d {
        Application(b) => format!("Application({} bytes)", b.len()),
        Grant(GrantOutcome::Answered(a)) => format!("Grant(answered {})", id8(&a.query_id)),
        Grant(GrantOutcome::Buffered) => "Grant(buffered)".into(),
        Grant(GrantOutcome::Ignored) => "Grant(ignored)".into(),
        Grant(GrantOutcome::Rejected(why)) => format!("Grant(rejected: {why})"),
        Late(Ok(t)) => format!("Late(record {})", id8(t)),
        Late(Err(why)) => format!("Late(refused: {why})"),
        Candidates(b) => format!("Candidates({} bytes)", b.len()),
        ResponseCopy(Ok(q)) => format!("ResponseCopy(query {})", id8(q)),
        ResponseCopy(Err(why)) => format!("ResponseCopy(refused: {why})"),
        Query { query, outcome } => {
            let q = query.map(|q| id8(&q)).unwrap_or_default();
            match outcome {
                QueryOutcome::Answered(_) => format!("Query({q}, answered)"),
                QueryOutcome::AwaitingGrant => format!("Query({q}, awaiting grant)"),
                QueryOutcome::Closed(why) => format!("Query({q}, closed: {why})"),
            }
        }
        Response(Ok((q, v))) => format!("Response(query {}, {v:?})", id8(q)),
        Response(Err(why)) => format!("Response(refused: {why})"),
        Served { records, more } => format!("Served({records} records, more: {more})"),
        Fetched(Ok(n)) => format!("Fetched({n} records)"),
        Fetched(Err(why)) => format!("Fetched(refused: {why})"),
        Conversation(c) => format!("Conversation({c})"),
    }
}

/// The variant name of a ceremony [`Msg`](crate::ceremony::Msg), for the
/// harness's path events: the kind of thing that moved, never its bytes.
#[must_use]
pub fn msg(m: &crate::ceremony::Msg) -> &'static str {
    use crate::ceremony::Msg::*;
    match m {
        Intent(_) => "Intent",
        Channels(_) => "Channels",
        CaptureKey(_) => "CaptureKey",
        ConsentRequest(_) => "ConsentRequest",
        Consent { .. } => "Consent",
        Grant(_) => "Grant",
        Query(_) => "Query",
        Response(_) => "Response",
        ResponseCopy(_) => "ResponseCopy",
        Responses(_) => "Responses",
        WitnessRequest(_) => "WitnessRequest",
        WitnessAnswer(_) => "WitnessAnswer",
        BackPointers(_) => "BackPointers",
        Proposal(_) => "Proposal",
        Signed(_) => "Signed",
        Record(_) => "Record",
        ClaimPrior(_) => "ClaimPrior",
        RecoveryResponse(_) => "RecoveryResponse",
        RecoveryProposal { .. } => "RecoveryProposal",
        AdoptionBody(_) => "AdoptionBody",
        Seal(_) => "Seal",
        PublishBundle(_) => "PublishBundle",
        StockOneTime(_) => "StockOneTime",
        PrekeyRequest(_) => "PrekeyRequest",
        CatalogQuery(_) => "CatalogQuery",
        PrekeyReply(_) => "PrekeyReply",
        PoolExhausted => "PoolExhausted",
        Payload { .. } => "Payload",
        Relay { .. } => "Relay",
        Transport(_) => "Transport",
    }
}

/// The field-test renderer: every event as one JSON line, handed to a
/// sink. Installed by the FFI at start on the `Diagnostics` platform
/// object, and by `tests/diag.rs` on a collector.
#[cfg(feature = "fieldtest")]
pub mod json {
    use serde_json::Value;
    use tracing::field::{Field, Visit};
    use tracing::span::{Attributes, Id};
    use tracing::{Event, Subscriber};
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::Context;
    use tracing_subscriber::registry::LookupSpan;

    /// A `tracing_subscriber` layer rendering each event as one JSON
    /// object on one line: `ms` since process start, `level`, `layer` (the
    /// event's target), `event` (its message, which is its name), the
    /// event's fields in the order recorded, and, inside a span, `span`
    /// with the span's name and fields.
    pub struct JsonLines<F> {
        sink: F,
    }

    impl<F: Fn(String) + Send + Sync + 'static> JsonLines<F> {
        /// A layer writing each rendered line to `sink`.
        pub fn new(sink: F) -> JsonLines<F> {
            JsonLines { sink }
        }
    }

    /// A span's fields, kept in its extensions from `on_new_span`.
    struct SpanFields(Vec<(String, Value)>);

    /// Collects an event's or a span's fields in recording order.
    struct Fields<'a>(&'a mut Vec<(String, Value)>);

    impl Fields<'_> {
        fn put(&mut self, field: &Field, v: Value) {
            self.0.push((field.name().to_string(), v));
        }
    }

    impl Visit for Fields<'_> {
        fn record_f64(&mut self, field: &Field, value: f64) {
            self.put(field, Value::from(value));
        }
        fn record_i64(&mut self, field: &Field, value: i64) {
            self.put(field, Value::from(value));
        }
        fn record_u64(&mut self, field: &Field, value: u64) {
            self.put(field, Value::from(value));
        }
        fn record_i128(&mut self, field: &Field, value: i128) {
            self.put(field, Value::from(value.to_string()));
        }
        fn record_u128(&mut self, field: &Field, value: u128) {
            self.put(field, Value::from(value.to_string()));
        }
        fn record_bool(&mut self, field: &Field, value: bool) {
            self.put(field, Value::from(value));
        }
        fn record_str(&mut self, field: &Field, value: &str) {
            self.put(field, Value::from(value));
        }
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.put(field, Value::from(format!("{value:?}")));
        }
    }

    fn render(pairs: &[(String, Value)]) -> String {
        let mut out = String::from("{");
        for (i, (k, v)) in pairs.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&serde_json::to_string(k).unwrap_or_default());
            out.push(':');
            out.push_str(&serde_json::to_string(v).unwrap_or_default());
        }
        out.push('}');
        out
    }

    impl<S, F> Layer<S> for JsonLines<F>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
        F: Fn(String) + Send + Sync + 'static,
    {
        fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
            let mut fields = Vec::new();
            attrs.record(&mut Fields(&mut fields));
            if let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(SpanFields(fields));
            }
        }

        fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
            let meta = event.metadata();
            let mut pairs: Vec<(String, Value)> = vec![
                ("ms".into(), Value::from(super::since_start().0)),
                (
                    "level".into(),
                    Value::from(meta.level().as_str().to_ascii_lowercase()),
                ),
                ("layer".into(), Value::from(meta.target())),
            ];
            let mut fields = Vec::new();
            event.record(&mut Fields(&mut fields));
            // the message is the event's name, and goes before its fields
            if let Some(i) = fields.iter().position(|(k, _)| k == "message") {
                let (_, name) = fields.remove(i);
                pairs.push(("event".into(), name));
            }
            pairs.extend(fields);
            if let Some(span) = ctx.event_scope(event).and_then(|mut s| s.next()) {
                let mut inner = vec![("name".to_string(), Value::from(span.name()))];
                if let Some(SpanFields(f)) = span.extensions().get::<SpanFields>() {
                    inner.extend(f.iter().cloned());
                }
                pairs.push(("span".into(), Value::Object(inner.into_iter().collect())));
            }
            (self.sink)(render(&pairs));
        }
    }
}
