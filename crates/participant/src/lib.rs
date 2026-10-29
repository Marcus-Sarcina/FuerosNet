//! `rhtnp`: a participant an experiment can drive.
//!
//! **This is an instrument and not a product.** `Robot/implementation-plan.md`
//! puts the shells at milestone 14 and they wait on two decisions and a
//! toolchain; the client itself has been finished and tested for longer
//! than that, in a process that nothing outside a test ever started. What
//! was missing was never a library — it was something a person can run, so
//! that the chain from a ceremony to an adoption to a payload can be
//! *observed* crossing real sockets between separately configured
//! processes rather than asserted inside one.
//!
//! It therefore claims none of the product entries. PRD-01 to PRD-09 are
//! obligations about what a user is shown and when they are asked, and a
//! command read from standard input is not a person; an instrument that
//! marked them would be marking them falsely.
//!
//! **Nothing is kept.** There is no store on disk beyond the identity it
//! was started from: a run is a run. That is a limit worth stating rather
//! than working around, because a client that persisted here would be
//! making decisions about what a client persists, and those belong to the
//! shells.
//!
//! One command a line on standard input, one or more lines out, each
//! beginning with a word that says what it is. Everything a script needs
//! to tell the two apart is in that first word.

// Every public item carries its own documentation: the source is the
// fourth reading of the design, and a bare name is not one.
#![warn(missing_docs)]

#[cfg(all(feature = "releasable", feature = "fieldtest"))]
compile_error!(
    "rhtn-participant: `releasable` and `fieldtest` are two flavours of one build; \
     build the field-test flavour with `--no-default-features --features fieldtest`"
);

pub mod carry;
pub mod diag;
pub mod terminal;

use rhtn_ffi::client::Participant;
use rhtn_ffi::client::platform;
use rhtn_ffi::net::Wake;
use std::path::Path;
use std::sync::Arc;
use terminal::{Terminal, hex, unhex};

/// The seeds an identity is derived from: the classical one and the
/// post-quantum one, in that order, as every other reader of one takes
/// them (design §5.1).
pub const IDENTITY_BYTES: usize = 64;

/// A running participant and the terminal it reaches the world through.
pub struct Instrument {
    client: Participant,
    shell: Arc<Terminal>,
}

/// What running one command came to.
pub enum Outcome {
    /// Lines to print, in order.
    Said(Vec<String>),
    /// The instrument was told to stop.
    Done,
}

impl Instrument {
    /// Start from an identity file and the peers it authenticates.
    ///
    /// **The identity is read and never minted**, for the reason the
    /// daemon's is: a participant that generates a key when its file is
    /// missing runs under an identity nobody has met, and the person
    /// running it would not know. `rhtn keys mint` is where one comes
    /// from.
    pub fn start(
        identity: &Path,
        peers: Option<&Path>,
        log: Option<&Path>,
    ) -> Result<Instrument, String> {
        let seeds = read_identity(identity)?;
        // **this participant's own key is in the lookup, and not because
        // the peers file listed it.**  A participant signs the records it
        // is a party to, and one that cannot verify its own signature
        // holds them unverifiable for want of a key it is holding
        // (`wire-format.md` §3.4).  The daemon learned this about its own
        // identity; nothing had asked it of a client, because nothing had
        // ever started one outside a test that passed every key in.
        let (ed, pq): ([u8; 32], [u8; 32]) = (
            seeds[..32].try_into().expect("64 bytes"),
            seeds[32..].try_into().expect("64 bytes"),
        );
        let mut known = vec![
            rhtn_crypto::SigningIdentity::from_seeds(&ed, &pq)
                .public
                .key_material(),
        ];
        if let Some(p) = peers {
            known.extend(read_peers(p)?);
        }
        let shell = Arc::new(Terminal::default());
        // the eighth object, diagnostics: the file `--log` named, in a
        // field-test build, and nothing otherwise
        // (Robot/field-test-diagnostics.md, section 4)
        let sink = diag::sink(log)?;
        let p = platform(
            shell.clone(),
            shell.clone(),
            shell.clone(),
            shell.clone(),
            shell.clone(),
            shell.clone(),
            shell.clone(),
            sink,
            // the instrument has no camera worth an engine: the stand-in
            Vec::new(),
            Vec::new(),
            0,
            0,
        );
        let client =
            Participant::start(seeds, known, Arc::new(p)).map_err(|e| e.reason().to_string())?;
        // the wall-clock anchor the merge tool places this file by, raised
        // once the FFI has installed its renderer
        diag::anchor();
        Ok(Instrument { client, shell })
    }

    /// What this participant is called.
    pub fn me(&self) -> String {
        hex(&self.client.me())
    }

    /// Run one command.  Anything the client said on the way — a question
    /// it put to the person, a notice it raised — follows the answer, so a
    /// transcript reads in the order the work happened.
    pub fn run(&self, line: &str) -> Outcome {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return Outcome::Said(Vec::new());
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f[0] == "quit" {
            return Outcome::Done;
        }
        let mut out = match self.dispatch(&f) {
            Ok(lines) => lines,
            Err(e) => vec![format!("error {e}")],
        };
        out.extend(self.shell.drain());
        Outcome::Said(out)
    }

    fn dispatch(&self, f: &[&str]) -> Result<Vec<String>, String> {
        match f {
            ["help"] => Ok(HELP.lines().map(str::to_string).collect()),
            ["me"] => Ok(vec![format!("me {}", self.me())]),
            // the introduction another party takes to hold this one by
            // locator (`wire-format.md` §2.3): what the bench hands a phone
            // for the nominee it names, since a nominee is offered to the
            // counterparty's prune by locator and material alone cannot be
            // a party held by material alone, for the direct path's gate
            // (design §12.6.3): the bench's stand-in for the meeting a witness
            // and its nominator would have had
            ["contact", material] => {
                let m = bytes(material)?;
                let who = self
                    .client
                    .hold_contact(m)
                    .map_err(|e| e.reason().to_string())?;
                Ok(vec![format!("contact {}", hex(&who))])
            }
            ["introduction"] => Ok(vec![format!(
                "introduction {}",
                hex(&self
                    .client
                    .own_introduction()
                    .map_err(|e| e.reason().to_string())?)
            )]),

            ["attached"] => Ok(vec![format!("attached {}", self.client.attached())]),

            ["attach", node, addresses, population @ ..] => {
                let node = id(node)?;
                let addrs: Vec<String> = addresses.split(',').map(str::to_string).collect();
                let pop: Result<Vec<Vec<u8>>, String> = population.iter().map(|p| id(p)).collect();
                let a = self
                    .client
                    .attach(node, addrs, pop?)
                    .map_err(|e| e.reason().to_string())?;
                Ok(vec![format!(
                    "attached serving={} primary={} queued={}",
                    hex(&a.serving),
                    a.primary,
                    a.queued
                )])
            }

            ["maintain"] => {
                self.client.maintain().map_err(|e| e.reason().to_string())?;
                Ok(vec!["maintained".into()])
            }

            ["send", to, kind, body] => {
                let k: u64 = kind
                    .parse()
                    .map_err(|_| format!("`{kind}` is not a payload kind"))?;
                let bytes = unhex(body).ok_or_else(|| format!("`{body}` is not hex"))?;
                self.client
                    .send(id(to)?, k, bytes)
                    .map_err(|e| e.reason().to_string())?;
                Ok(vec!["sent".into()])
            }

            ["wake", "off"] => {
                self.client.wake(None).map_err(|e| e.reason().to_string())?;
                Ok(vec!["wake off".into()])
            }
            ["wake", url, key, rest @ ..] => {
                let lapses_at = match rest {
                    [] => None,
                    [t] => Some(
                        t.parse::<u64>()
                            .map_err(|_| format!("`{t}` is not a time"))?,
                    ),
                    _ => return Err("wake <url> <key-hex> [<lapses-at>]".into()),
                };
                let key = unhex(key).ok_or_else(|| format!("`{key}` is not hex"))?;
                self.client
                    .wake(Some(Wake {
                        url: (*url).to_string(),
                        key,
                        lapses_at,
                    }))
                    .map_err(|e| e.reason().to_string())?;
                Ok(vec!["wake set".into()])
            }

            // what this client holds of its own neighbourhood
            ["horizon"] => {
                let mut out = vec![format!("records {}", self.client.records())];
                out.extend(self.client.places().iter().map(|p| {
                    format!(
                        "place {} anchor={} path={} nibbles={}",
                        hex(&p.node),
                        hex(&p.position.anchor),
                        hex(&p.position.path),
                        p.position.nibbles
                    )
                }));
                Ok(out)
            }
            ["resolvable"] => Ok(vec![format!(
                "resolvable {}",
                joined(&self.client.resolvable())
            )]),
            ["reachable"] => Ok(self
                .client
                .instances()
                .iter()
                .map(|r| {
                    let (n, a) = (&r.keyhash, &r.addresses);
                    format!(
                        "reachable {} at={}",
                        hex(n),
                        if a.is_empty() {
                            "-".into()
                        } else {
                            a.join(",")
                        }
                    )
                })
                .collect()),
            ["distance", node] => Ok(vec![match self.client.distance(id(node)?) {
                None => "distance none".into(),
                Some(d) => format!("distance {d}"),
            }]),
            ["holds", txid] => Ok(vec![format!("holds {}", self.client.holds(id(txid)?))]),
            ["prune"] => Ok(vec![format!("pruned {}", self.client.prune())]),

            ["events"] => self.events(0),
            ["events", ms] => self.events(
                ms.parse::<u64>()
                    .map_err(|_| format!("`{ms}` is not a count of milliseconds"))?,
            ),

            ["answer"] => Ok(vec![format!(
                "answer {}",
                if self.shell.answering() { "yes" } else { "no" }
            )]),
            ["answer", "yes"] => {
                self.shell.answers(true);
                Ok(vec!["answer yes".into()])
            }
            ["answer", "no"] => {
                self.shell.answers(false);
                Ok(vec!["answer no".into()])
            }

            _ => self.ceremony(f),
        }
    }

    /// The ceremony, a step a command.
    ///
    /// **Each step's product is a token this instrument's operator carries
    /// to the other device.** design §7 has the two parties in each other's
    /// presence over a channel neither the protocol nor this fixes; a
    /// harness copying a token between two processes is the analogue of a
    /// screen and a camera, and is as much of the presence as a machine
    /// with no camera can have.
    fn ceremony(&self, f: &[&str]) -> Result<Vec<String>, String> {
        let c = &self.client;
        match f {
            ["begin", counterparty, rest @ ..] => {
                let (nominees, initiator) = match rest {
                    [] => (Vec::new(), false),
                    ["initiator"] => (Vec::new(), true),
                    [n] => (carry_ids(n)?, false),
                    [n, "initiator"] => (carry_ids(n)?, true),
                    _ => return Err("begin <counterparty> [<nominee>,...] [initiator]".into()),
                };
                c.begin(Some(id(counterparty)?), nominees, initiator)
                    .map_err(|e| e.reason().to_string())?;
                Ok(vec!["begun".into()])
            }
            // THE OPTICAL EXCHANGE (`wire-format.md` §14.3.1): two QRs each
            // way, and the ceremony-id both devices compute.  Each command
            // prints bytes to carry and takes bytes carried.
            ["optical"] => Ok(vec![format!(
                "optical {}",
                hex(&c
                    .optical_contribution()
                    .map_err(|e| e.reason().to_string())?)
            )]),
            ["take-optical", blob] => {
                let who = c
                    .take_optical(carry::blob(blob)?)
                    .map_err(|e| e.reason().to_string())?;
                Ok(vec![format!("showed-by {}", hex(&who))])
            }
            ["transcript"] => Ok(vec![format!(
                "transcript {}",
                hex(&c.transcript_confirm().map_err(|e| e.reason().to_string())?)
            )]),
            ["take-transcript", blob] => {
                let id = c
                    .take_transcript(carry::blob(blob)?)
                    .map_err(|e| e.reason().to_string())?;
                Ok(vec![format!("ceremony {}", hex(&id))])
            }
            // and the bearer's own load (§14.3.2)
            ["intent"] => Ok(vec![format!(
                "intent {}",
                carry::pack_carriage(&c.intent_carriage().map_err(|e| e.reason().to_string())?)
            )]),
            ["intent", from, blob] => {
                let n = c
                    .take_intent_carriage(id(from)?, carry::take_carriage(blob)?)
                    .map_err(|e| e.reason().to_string())?;
                Ok(vec![format!("continuations-taken {n}")])
            }
            ["ceremony"] => {
                Ok(vec![c.ceremony().map_or("ceremony none".into(), |i| {
                    format!("ceremony {}", hex(&i))
                })])
            }

            ["candidates", blob] => Ok(vec![format!(
                "handover {}",
                hex(&c
                    .candidate_carriage(carry::blob(blob)?)
                    .map_err(|e| e.reason().to_string())?)
            )]),
            ["take-candidates", blob] => Ok(vec![format!(
                "candidates {}",
                hex(&c
                    .take_candidate_carriage(carry::blob(blob)?)
                    .map_err(|e| e.reason().to_string())?)
            )]),

            ["capture-key"] => Ok(vec![format!(
                "capture-key {}",
                hex(&c.capture_key().map_err(|e| e.reason().to_string())?)
            )]),
            ["capture", key] => {
                c.capture(bytes(key)?).map_err(|e| e.reason().to_string())?;
                Ok(vec!["captured".into()])
            }

            ["verifiers"] => Ok(c
                .select_verifiers()
                .map_err(|e| e.reason().to_string())?
                .iter()
                .map(|s| format!("verifier {} basis={}", hex(&s.verifier), s.basis))
                .collect()),
            ["query", verifier] => Ok(vec![format!(
                "query {}",
                hex(&c
                    .query_for(id(verifier)?)
                    .map_err(|e| e.reason().to_string())?)
            )]),
            ["consent", query] => Ok(
                match c
                    .consent(bytes(query)?)
                    .map_err(|e| e.reason().to_string())?
                {
                    None => vec!["consent none".into()],
                    Some(s) => {
                        let mut out = vec![format!("consent {}", hex(&s.consent))];
                        // the grant leaves with the consent, one more token to
                        // carry — to the query's verifier and to nobody else
                        if let Some(g) = s.grant {
                            out.push(format!("grant {}", hex(&g)));
                        }
                        out
                    }
                },
            ),
            ["request", query, consent, basis] => {
                let b: u32 = basis
                    .parse()
                    .map_err(|_| format!("`{basis}` is not a selection basis"))?;
                Ok(vec![format!(
                    "request {}",
                    hex(&c
                        .request(bytes(query)?, bytes(consent)?, b)
                        .map_err(|e| e.reason().to_string())?)
                )])
            }
            ["take-query", from, blob] => Ok(vec![answered(
                c.take_query(id(from)?, bytes(blob)?)
                    .map_err(|e| e.reason().to_string())?,
            )]),
            ["take-grant", from, blob] => Ok(vec![answered(
                c.take_grant(id(from)?, bytes(blob)?)
                    .map_err(|e| e.reason().to_string())?,
            )]),
            ["take-response", blob] => {
                c.take_response(bytes(blob)?)
                    .map_err(|e| e.reason().to_string())?;
                Ok(vec!["response taken".into()])
            }
            ["gathered"] => Ok(vec![format!("gathered {}", joined(&c.gathered()))]),
            ["responses"] => Ok(c
                .responses()
                .iter()
                .map(|r| {
                    format!(
                        "response verifier={} subject={} answer={:?}",
                        hex(&r.verifier),
                        hex(&r.subject),
                        r.answer
                    )
                })
                .collect()),

            ["nominees"] => {
                let n = c.nominees();
                let (mine, theirs) = (n.mine, n.theirs);
                Ok(vec![format!(
                    "nominees mine={} theirs={}",
                    joined(&mine),
                    joined(&theirs)
                )])
            }
            // THE ENGAGEMENT (kind 20) [author, 2026-10-19]: what a
            // nominee is sent with the intent, carried here as one line,
            // and taken by the nominee so the later ask has an engagement
            // to close
            ["engagement"] => Ok(vec![format!(
                "engagement {}",
                carry::pack_engagement(&c.engagement().map_err(|e| e.reason().to_string())?)
            )]),
            ["take-engagement", from, blob] => {
                let e = carry::take_engagement(blob)?;
                Ok(vec![match c
                    .take_engagement(id(from)?, e)
                    .map_err(|e| e.reason().to_string())?
                {
                    true => "engaged".into(),
                    false => "declined".into(),
                }])
            }
            // the window a witness will not sign inside, in seconds: the
            // instrument's to set, since a process test cannot wait four
            // minutes and a bench run wants it short [author, 2026-10-21]
            ["window", s] => {
                let seconds: u64 = s.parse().map_err(|_| "window <seconds>".to_string())?;
                c.set_witness_window_s(seconds)
                    .map_err(|e| e.reason().to_string())?;
                Ok(vec![format!("window {seconds}")])
            }
            ["witness-ask"] => Ok(vec![format!(
                "witness-ask {}",
                carry::pack_ask(&c.witness_ask().map_err(|e| e.reason().to_string())?)
            )]),
            ["take-witness-ask", blob] => {
                Ok(vec![match c.take_witness_ask(carry::take_ask(blob)?) {
                    None => "declined".into(),
                    Some(flags) => format!("witnessing {flags}"),
                }])
            }

            ["back-pointers"] => Ok(vec![format!(
                "back-pointers {}",
                joined(&c.back_pointers())
            )]),
            ["propose", theirs, witnesses] => {
                let theirs: Result<Vec<Vec<u8>>, String> =
                    carry_list(theirs).iter().map(|x| bytes(x)).collect();
                let w = carry::take_witnesses(witnesses)?;
                let proposal = c.propose(theirs?, w).map_err(|e| e.reason().to_string())?;
                let (p, set) = (proposal.proposed, proposal.revealed);
                Ok(vec![
                    format!("proposed {}", carry::pack_proposed(&p)),
                    format!("disclosures {}", carry::pack_revealed(&set)),
                ])
            }
            ["signers", proposed] => Ok(vec![format!(
                "signers {}",
                joined(&carry::take_proposed(proposed)?.signers())
            )]),
            ["body", proposed, back] => Ok(vec![format!(
                "body {}",
                hex(&c
                    .body(carry::take_proposed(proposed)?, carry::take_back(back)?)
                    .map_err(|e| e.reason().to_string())?)
            )]),
            ["review-and-sign", proposed, set, back] => Ok(vec![format!(
                "signed {}",
                hex(&c
                    .review_and_sign(
                        carry::take_proposed(proposed)?,
                        carry::take_revealed(set)?,
                        carry::take_back(back)?
                    )
                    .map_err(|e| e.reason().to_string())?)
            )]),
            ["witness-sign", proposed, back] => Ok(vec![format!(
                "signed {}",
                hex(&c
                    .witness_sign(carry::take_proposed(proposed)?, carry::take_back(back)?)
                    .map_err(|e| e.reason().to_string())?)
            )]),
            ["envelope", body, entries] => Ok(vec![format!(
                "envelope {}",
                hex(&rhtn_ffi::client::presence_envelope(
                    bytes(body)?,
                    carry::take_entries(entries)?
                )
                .map_err(|e| e.reason().to_string())?)
            )]),

            // ending an authority, and what this client's policy makes
            // of a party: `light-client-requirements.md` §§4.2, 6
            ["relationships"] => Ok(c
                .lines()
                .iter()
                .map(|r| format!("relationship {} series {}", hex(&r.patron), r.series))
                .collect()),
            // what leaving costs, before leaving: §6's warning, in the
            // lines a test can read
            ["cost-of-leaving", patron] => {
                let k = c
                    .cost_of_leaving(id(patron)?)
                    .map_err(|e| e.reason().to_string())?;
                Ok(std::iter::once(format!("keeping {}", k.keeping))
                    .chain(k.leaving.iter().map(|w| format!("leaving {}", hex(w))))
                    .chain(k.losing.iter().map(|l| {
                        format!(
                            "losing {} through {} because {:?}",
                            hex(&l.resource),
                            hex(&l.node),
                            l.reason
                        )
                    }))
                    .chain(k.limits.iter().map(|v| format!("limit {v:?}")))
                    .collect())
            }
            ["depart", patron] => Ok(vec![format!(
                "departed {}",
                hex(&c
                    .depart(id(patron)?, None)
                    .map_err(|e| e.reason().to_string())?)
            )]),
            ["standing", subject] => Ok(vec![format!("standing {:.6}", c.standing(id(subject)?))]),
            ["evaluate", candidates] => {
                let who: Result<Vec<Vec<u8>>, String> =
                    carry_list(candidates).iter().map(|x| id(x)).collect();
                let e = c.evaluate(who?);
                Ok(std::iter::once(format!("joint {:.6}", e.joint))
                    .chain(
                        e.individual
                            .iter()
                            .map(|i| format!("individual {} {:.6}", hex(&i.who), i.score)),
                    )
                    .chain(e.admitted.iter().map(|a| format!("admitted {}", hex(a))))
                    .collect())
            }

            // the adoption, on the record the ceremony produced
            ["where"] => Ok(c
                .subnets()
                .iter()
                .map(|s| format!("subnet {}", hex(&s.id)))
                .chain(std::iter::once(format!("position {}", hex(&c.position()))))
                .collect()),
            ["where", subnet] => Ok(vec![match c.position_in(id(subnet)?) {
                None => "position none".into(),
                Some(p) => format!("position {}", hex(&p)),
            }]),
            // the subnet, or `-` for the one under this client's own key
            ["adopt", subnet, node, presence, series, back] => {
                let s: u32 = series
                    .parse()
                    .map_err(|_| format!("`{series}` is not a series"))?;
                let back: Result<Vec<Vec<u8>>, String> =
                    carry_list(back).iter().map(|x| bytes(x)).collect();
                Ok(vec![format!(
                    "adoption {}",
                    hex(&c
                        .propose_adoption(
                            (*subnet != "-").then(|| id(subnet)).transpose()?,
                            id(node)?,
                            id(presence)?,
                            s,
                            back?,
                            // the CLI's party brings no scope
                            None,
                        )
                        .map_err(|e| e.reason().to_string())?)
                )])
            }
            ["sign", body] => Ok(vec![format!(
                "signed {}",
                hex(&c
                    .sign_body(bytes(body)?)
                    .map_err(|e| e.reason().to_string())?)
            )]),
            ["adoption-envelope", body, entries] => Ok(vec![format!(
                "envelope {}",
                hex(&rhtn_ffi::client::adoption_envelope(
                    bytes(body)?,
                    carry::take_entries(entries)?
                )
                .map_err(|e| e.reason().to_string())?)
            )]),
            ["take-adoption", envelope] => Ok(vec![format!(
                "adopted {}",
                hex(&c
                    .take_adoption(bytes(envelope)?)
                    .map_err(|e| e.reason().to_string())?)
            )]),
            ["finalize", envelope] => Ok(vec![format!(
                "finalized {}",
                hex(&c
                    .finalize(bytes(envelope)?, None)
                    .map_err(|e| e.reason().to_string())?)
            )]),
            ["finalize", envelope, set] => Ok(vec![format!(
                "finalized {}",
                hex(&c
                    .finalize(bytes(envelope)?, Some(carry::take_revealed(set)?))
                    .map_err(|e| e.reason().to_string())?)
            )]),

            _ => Err(format!(
                "`{}` is not a command; `help` lists them",
                f.join(" ")
            )),
        }
    }

    fn events(&self, ms: u64) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        while let Some(e) = self.client.next_event(ms) {
            out.push(match e {
                rhtn_ffi::net::Event::Connection(status) => {
                    use rhtn_ffi::net::Status;
                    match status {
                        Status::Detached => "connection detached".into(),
                        Status::Attached { serving, primary } => format!(
                            "connection attached serving={} primary={primary}",
                            hex(&serving)
                        ),
                        Status::Reconnecting { from } => {
                            format!("connection reconnecting from={}", hex(&from))
                        }
                        Status::Lost { from, reason } => {
                            format!("connection lost from={} reason={reason}", hex(&from))
                        }
                    }
                }
                // which nouns of the model changed, in the model's order
                rhtn_ffi::net::Event::Changed { nouns } => format!(
                    "changed {}",
                    nouns
                        .iter()
                        .map(|n| format!("{n:?}").to_lowercase())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                rhtn_ffi::net::Event::Payload { from, kind, bytes } => {
                    format!(
                        "payload from={} kind={kind} bytes={}",
                        hex(&from),
                        hex(&bytes)
                    )
                }
                rhtn_ffi::net::Event::ResponseCopy {
                    from,
                    query,
                    refused,
                } => {
                    format!(
                        "response-copy from={} query={} refused={}",
                        hex(&from),
                        query.map_or("-".into(), |q| hex(&q)),
                        refused.unwrap_or_else(|| "-".into())
                    )
                }
                rhtn_ffi::net::Event::Late {
                    from,
                    record,
                    refused,
                } => {
                    format!(
                        "late from={} record={} refused={}",
                        hex(&from),
                        record.map_or("-".into(), |r| hex(&r)),
                        refused.unwrap_or_else(|| "-".into())
                    )
                }
                rhtn_ffi::net::Event::Conversed {
                    from,
                    step,
                    what,
                    record,
                } => {
                    format!(
                        "conversed from={} step={step:?} what={what} record={}",
                        hex(&from),
                        record.map_or("-".into(), |r| hex(&r))
                    )
                }
                rhtn_ffi::net::Event::Peer { from, what } => {
                    format!("peer from={} what={what}", hex(&from))
                }
                rhtn_ffi::net::Event::Answered {
                    from,
                    query,
                    answer,
                    refused,
                } => {
                    format!(
                        "answered from={} query={} answer={} refused={}",
                        hex(&from),
                        query.map_or("-".into(), |q| hex(&q)),
                        answer.map_or("-".into(), |a| format!("{a:?}")),
                        refused.unwrap_or_else(|| "-".into())
                    )
                }
            });
            // one pass over what is already waiting: a second wait would
            // make `events` a blocking command with no way back out
            if ms > 0 {
                break;
            }
        }
        out.push("events done".into());
        Ok(out)
    }
}

fn answered(a: Option<rhtn_ffi::client::Answered>) -> String {
    match a {
        None => "awaiting-grant".into(),
        Some(a) => format!(
            "answered query={} querier={} subject={}",
            hex(&a.query),
            hex(&a.to_querier),
            a.to_subject.map_or("-".into(), |c| format!(
                "{}:{}",
                hex(&c.subject),
                hex(&c.bytes)
            ))
        ),
    }
}

fn joined(v: &[Vec<u8>]) -> String {
    if v.is_empty() {
        "-".into()
    } else {
        v.iter().map(|x| hex(x)).collect::<Vec<_>>().join(",")
    }
}

fn carry_list(s: &str) -> Vec<&str> {
    if s == "-" {
        Vec::new()
    } else {
        s.split(',').collect()
    }
}

fn carry_ids(s: &str) -> Result<Vec<Vec<u8>>, String> {
    carry_list(s).iter().map(|x| id(x)).collect()
}

fn bytes(s: &str) -> Result<Vec<u8>, String> {
    unhex(s).ok_or_else(|| format!("`{s}` is not hex"))
}

fn id(s: &str) -> Result<Vec<u8>, String> {
    let b = unhex(s).ok_or_else(|| format!("`{s}` is not hex"))?;
    if b.len() != 32 {
        return Err(format!("`{s}` is {} bytes, and an identity is 32", b.len()));
    }
    Ok(b)
}

/// Read the seeds an identity is derived from.
pub fn read_identity(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() != IDENTITY_BYTES {
        return Err(format!(
            "{} is {} bytes, not {IDENTITY_BYTES}",
            path.display(),
            bytes.len()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "{} is readable beyond its owner (mode {:o})",
                path.display(),
                mode & 0o777
            ));
        }
    }
    Ok(bytes)
}

/// Read the parties this participant authenticates: one hex `KeyMaterial`
/// blob a line, in the shape the daemon's peers file has, because it is
/// the same fact about the same kind of party (`wire-format.md` §9.1).
pub fn read_peers(path: &Path) -> Result<Vec<Vec<u8>>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        out.push(unhex(line).ok_or_else(|| format!("line {}: not hex", i + 1))?);
    }
    Ok(out)
}

/// The `help` command's own output: every command this driver takes, one
/// per line.
pub const HELP: &str = "\
help                      these lines
me                        this participant's keyhash
introduction              this participant's introduction: its material and signed locator
contact <material>        hold a party as a contact by its key material
attach <node> <addr>[,<addr>] [<party>...]
                          attach to a serving node, sweeping the parties named
attached                  whether a session is held
maintain                  rotate, replenish and ask for what is due
send <to> <kind> <hex>    payload, over the direct path or through the node
wake <url> <key> [<at>]   where to be rung; `wake off` withdraws it
events [<ms>]             what arrived; waits <ms> for the first
horizon                   what this client holds of its neighbourhood
resolvable                everybody it can place without asking anyone
reachable                 the infrastructure it can reach, and where
distance <node>           adoption or sibling edges away, inside the horizon
holds <txid>              whether it holds that transaction
prune                     drop what has left the horizon

The ceremony, a step a command. Each step's product is one token, to be
carried to the other device by whatever the two have between them:

begin <party> [<nominee>,...] [initiator]
                          open a ceremony; prints the intent to carry
intent <from> <intent>    take theirs; prints the ceremony id
ceremony                  the ceremony id, once both intents have crossed
capture-key               the key your captures will be sealed under
capture <key>             capture them under the key they sent
verifiers                 the verifiers selected, and the basis of each
query <verifier>          the query to put to one of them
consent <query>           the subject's consent, or none; where a grant is
                          owed it follows, to carry to the query's verifier
request <query> <consent> <basis>
                          the request that carries all three
take-query <from> <request>       as a verifier
take-grant <from> <grant>         as a verifier
take-response <response>  a verifier's answer about the counterparty
gathered                  the responses this client holds, to carry
responses                 the same, as a screen would show them
nominees                  who each side nominated
window <seconds>          the window this client will not sign inside
engagement                the nomination to carry to a nominee
take-engagement <from> <e> hold one, as a nominee
witness-ask               what to ask a nominee
take-witness-ask <ask>    answer one, as a nominee
back-pointers             this signer's back-pointers
propose <responses> <witnesses>
                          the record; prints the proposal and disclosures
signers <proposal>        everybody who signs it, in order
body <proposal> <back>    the body every signer signs over
review-and-sign <proposal> <disclosures> <back>     as a participant
witness-sign <proposal> <back>                      as a witness
envelope <body> <signer>:<signature>,...
                          the envelope the record travels in
finalize <envelope> [<disclosures>]
                          take the finished record

And the adoption on it:

where [<anchor>]          the subnets this client is in, or where it sits in one
adopt <anchor> <node> <presence> <series> <back>
                          as a patron: the adoption body, in that subnet
sign <body>               sign a body proposed or shown
adoption-envelope <body> <signer>:<signature>,...
take-adoption <envelope>  take it; it is also what says where you now sit
answer [yes|no]           the standing answer to a question put to the person
quit                      stop";
