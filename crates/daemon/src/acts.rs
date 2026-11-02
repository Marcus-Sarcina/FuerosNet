//! **What this instance declares it can be asked for**, so something
//! other than its own page can draw it [author, 2026-11-02].
//!
//! ## The problem this answers
//!
//! `infra-client-requirements.md` §8.3 gives the node its own
//! administration pages and the client the frame, and the reason is
//! durable: *"each implementation administers itself with a surface that
//! matches its software and stays in step with it as that software
//! changes"*, with the client shipping *"the provisioning pages and no
//! others"*. A client holding native screens for the node's acts would be
//! shipping other pages, and would have to ship one per node version and
//! one per third-party package besides.
//!
//! **But a form cannot prove itself.** An act that does not arrive on the
//! loopback carries a MAC over its own body
//! (`crate::administration::Surface::authorised`), and HTML in a frame
//! cannot compute one. So either the page's acts stop working from a
//! phone, or something outside the page makes them.
//!
//! ## The mechanism is the one a package already uses
//!
//! A package does not ship its administration page either: it **declares**
//! its operations — a name, a label, and parameters from a closed set —
//! and the node draws them, *"so an operator learns one idiom rather than
//! one per package"* (`rhtn_node::resources::Operation`). **This is the
//! same contract one level up.** The node declares its own acts in that
//! same vocabulary, a client draws them natively and proves each
//! invocation, and nothing about layout or behaviour is agreed between the
//! two — only the field types, which were already a closed set because the
//! node had to render every one of them.
//!
//! So the division §8.3 draws survives intact, and sharpens:
//!
//! | | Whose | How it travels |
//! |---|---|---|
//! | **what is shown** — the status, the rows, the planes, the counts | the node's | its own page, in the client's frame, read-only |
//! | **what may be asked** | the node's, declared | [`Act`], drawn by whoever is asking |
//! | **the asking** | the client's | one proved invocation on one route |
//!
//! ## Three things a client needs and the page cannot give it
//!
//! **A version**, so a client and an instance that were built apart can
//! tell how far apart ([`SURFACE`]). A client meeting a version it does
//! not know draws what it understands and says plainly that the rest needs
//! a newer client; it does not guess.
//!
//! **What an act is about** ([`About`]), so a client pre-fills the
//! resource an operator is looking at rather than asking them to paste a
//! keyhash they are already reading.
//!
//! **What the client must bring** ([`Brings`]), which is the whole reason
//! the acts had to leave the page: a package's install needs the
//! restricted ssh key for the machine, derived from the operator's signing
//! seed, and **a seed is the one thing the frame is isolated from** (§8.3:
//! *"isolated from the presenting client's keys"*; design §23.3). The node
//! says it needs one; the client is where one can be had.
//!
//! ## The declared names are the field names
//!
//! An act's parameters are named exactly as the implementing route reads
//! them, so an invocation of `grant` is the body `/node/grant` already
//! takes. **Nothing is translated**, which is what keeps a declaration
//! from drifting from the act it describes: a parameter nobody reads would
//! have to be wired to be read.
//!
//! **`key` is never a declared parameter.** What a person answers is
//! declared; what their client supplies is [`Brings`]. A page that could
//! name the key field could ask for a key.

use rhtn_node::resources::{Kind, Operation, Parameter};

/// **The administration vocabulary's version**, as the host surface is
/// versioned (`rhtn_node::resources::HOST_VERSIONS`).
///
/// It names the *vocabulary*, not the acts: an act added or withdrawn is
/// an ordinary change a client reads out of the declaration, and only a
/// change to what a declaration means — a new parameter kind, a new
/// field on an act — is a new version. **Bindings are only ever added
/// within a version**, which is §9.3's rule for the host surface and is
/// this one's for the same reason.
pub const SURFACE: &str = "rhtn/administration@1";

/// What an act is about, so a client knows what to fill in and what to
/// ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum About {
    /// The instance itself.
    Node,
    /// One resource it hosts, named by the `resource` parameter.
    Resource,
}

impl About {
    /// Its name in the declaration.
    pub fn name(self) -> &'static str {
        match self {
            About::Node => "node",
            About::Resource => "resource",
        }
    }
}

/// **What the client must supply that neither the person nor the page
/// can.**
///
/// Closed, and short on purpose: every member is something the instance
/// cannot hold and the frame must not see. A client meeting one it does
/// not know refuses the act rather than sending it without.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Brings {
    /// Nothing; the parameters are the whole of it.
    Nothing,
    /// **The restricted ssh key for this instance's host**, derived from
    /// the operator's signing seed and good for one act
    /// (`rhtn_crypto::ssh::host_runner_key`, [`crate::runner`]). The
    /// client sends it as `key`.
    HostKey,
}

impl Brings {
    /// Its name in the declaration.
    pub fn name(self) -> &'static str {
        match self {
            Brings::Nothing => "nothing",
            Brings::HostKey => "host-key",
        }
    }
}

/// One act this instance declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act {
    /// Its name, label, help and parameters, in the vocabulary a package
    /// declares its own operations in.
    pub operation: Operation,
    /// What it is about.
    pub about: About,
    /// What the client brings.
    pub brings: Brings,
}

/// A parameter, shortly.
fn p(name: &str, label: &str, kind: Kind) -> Parameter {
    Parameter {
        name: name.into(),
        label: label.into(),
        kind,
    }
}

/// An act, shortly.
fn act(
    name: &str,
    label: &str,
    help: &str,
    about: About,
    brings: Brings,
    parameters: Vec<Parameter>,
) -> Act {
    Act {
        operation: Operation {
            name: name.into(),
            label: label.into(),
            help: help.into(),
            parameters,
        },
        about,
        brings,
    }
}

/// **The acts about the instance itself**, which every serving node
/// declares.
///
/// `acknowledging` is the policy in force, because the label of a toggle
/// is the state it moves to and a client drawing "turn it on" over a node
/// that is already acknowledging would be lying about what the act does.
pub fn node_acts(acknowledging: bool) -> Vec<Act> {
    vec![
        act(
            "acknowledge",
            if acknowledging {
                "Stop countersigning adoptions"
            } else {
                "Countersign every adoption beneath this node"
            },
            "The standing acknowledgement policy (design §11.2.1). It is in force until the \
             next start, which takes what the configuration says: this act does not rewrite \
             the configuration, which is a file its operator owns.",
            About::Node,
            Brings::Nothing,
            vec![p(
                "acknowledge",
                "on or off",
                Kind::Choice {
                    of: vec!["on".into(), "off".into()],
                },
            )],
        ),
        act(
            "reload",
            "Re-read the hosting file",
            "What this node hosts and who may reach it, re-read without a restart.",
            About::Node,
            Brings::Nothing,
            Vec::new(),
        ),
        act(
            "stop",
            "Stop this node",
            "Writing back what is held and losing nothing in flight: the same path a SIGTERM \
             takes.",
            About::Node,
            Brings::Nothing,
            Vec::new(),
        ),
        act(
            "install",
            "Install a package on this machine",
            "The image is fetched before anything is written and this node re-reads its own \
             configuration before the package is started, so a manifest it will not admit \
             leaves nothing running. Nobody may reach the package until a role is granted on \
             it.",
            About::Node,
            Brings::HostKey,
            vec![
                p("resource", "the resource it is hosted as", Kind::Keyhash),
                p("owner", "whose horizon reaches it", Kind::Keyhash),
                p(
                    "authority",
                    "the authority it answers under",
                    Kind::Text { max: 253 },
                ),
                p(
                    "image",
                    "the image the machine pulls",
                    Kind::Text { max: 512 },
                ),
                p(
                    "port",
                    "where it listens on this machine's loopback",
                    Kind::Number {
                        low: 1,
                        high: 65_535,
                    },
                ),
                p(
                    "manifest",
                    "the package's own manifest",
                    Kind::Text {
                        max: rhtn_node::resources::MAX_TEXT,
                    },
                ),
            ],
        ),
        act(
            "running",
            "What the machine says is running",
            "The packages this node installed, as its host reports them.",
            About::Node,
            Brings::HostKey,
            Vec::new(),
        ),
    ]
}

/// **The acts about one resource**, drawn from what that resource is: the
/// roles its package declared, the grants that stand on it now, and the
/// templates it shipped.
///
/// **So the declaration is of this instant and not of this build.** A
/// client that asks again after a grant gets a different one, which is
/// what lets `at` be bounded by the grants that exist rather than by a
/// number somebody guessed.
pub fn resource_acts(view: &crate::operator::ResourceView, installed: bool) -> Vec<Act> {
    let roles: Vec<String> = view.roles.iter().map(|(r, _)| r.clone()).collect();
    let clauses = || {
        let mut of: Vec<String> = rhtn_node::grant::VOCABULARY
            .iter()
            .map(|n| n.of.to_string())
            .collect();
        of.sort();
        of
    };
    let mut whole = clauses();
    whole.insert(0, "everyone".into());
    let mut out = vec![act(
        "grant",
        "Grant a role",
        "A grant takes effect when it is made and is written to this node's own table, so a \
         restart takes it. `everyone` is the absence of a clause: every member of the owner's \
         horizon and nothing further.",
        About::Resource,
        Brings::Nothing,
        vec![
            p("resource", "the resource", Kind::Keyhash),
            p("roles", "the roles it carries", Kind::Several { of: roles }),
            p("of", "who it reaches", Kind::Choice { of: whole }),
            p(
                "with",
                "that clause's parameter, where it takes one",
                Kind::Text { max: 64 },
            ),
        ],
    )];
    // **narrowing and withdrawing exist only where there is something to
    // narrow**, and the grant is named by its place in the list the page
    // shows, so the bound is the list's
    if !view.grants.is_empty() {
        let high = (view.grants.len() - 1) as i64;
        out.push(act(
            "grant-narrow",
            "Narrow a grant",
            "Every clause on a grant must hold, which is the conjunction \
             `resource-requirements.md` §7.1 asks for.",
            About::Resource,
            Brings::Nothing,
            vec![
                p("resource", "the resource", Kind::Keyhash),
                p("at", "which grant", Kind::Number { low: 0, high }),
                p("of", "and also", Kind::Choice { of: clauses() }),
                p(
                    "with",
                    "that clause's parameter, where it takes one",
                    Kind::Text { max: 64 },
                ),
            ],
        ));
        out.push(act(
            "grant-drop",
            "Withdraw a grant",
            "The rows it wrote go, and the sessions they were carrying end with them (§10.5).",
            About::Resource,
            Brings::Nothing,
            vec![
                p("resource", "the resource", Kind::Keyhash),
                p("at", "which grant", Kind::Number { low: 0, high }),
            ],
        ));
    }
    if !view.templates.is_empty() {
        out.push(act(
            "grant-template",
            "Take a grant this package offers",
            "Offered, not applied: §10.4 has a permission default be a security decision, and \
             the package author's incentive runs toward breadth. Taking one is the operator's \
             act.",
            About::Resource,
            Brings::Nothing,
            vec![
                p("resource", "the resource", Kind::Keyhash),
                p(
                    "name",
                    "which grant",
                    Kind::Choice {
                        of: view.templates.iter().map(|(n, ..)| n.clone()).collect(),
                    },
                ),
            ],
        ));
    }
    // **and the two that need the machine**, declared only for a package
    // this node installed itself: a resource its operator wrote into the
    // hosting file by hand is theirs to stop and start
    if installed {
        out.push(act(
            "logs",
            "What this package wrote",
            "The package's own process output, as the machine's engine kept it. What the \
             instance wrote through the host's log binding is on this page already.",
            About::Resource,
            Brings::HostKey,
            vec![
                p("resource", "the resource", Kind::Keyhash),
                p(
                    "lines",
                    "how many lines",
                    Kind::Number { low: 1, high: 500 },
                ),
            ],
        ));
        out.push(act(
            "remove",
            "Remove this package",
            "The package is stopped and the entry dropped. The grants, the storage block, the \
             log and the jobs are kept, because a removal and an install of one resource is how \
             a package is upgraded.",
            About::Resource,
            Brings::HostKey,
            vec![p("resource", "the resource", Kind::Keyhash)],
        ));
    }
    out
}

/// A parameter as the declaration carries it.
fn parameter_json(p: &Parameter) -> serde_json::Value {
    let mut v = serde_json::json!({ "name": p.name, "label": p.label });
    let o = v.as_object_mut().expect("an object");
    match &p.kind {
        Kind::Flag => {
            o.insert("kind".into(), "flag".into());
        }
        Kind::Number { low, high } => {
            o.insert("kind".into(), "number".into());
            o.insert("low".into(), (*low).into());
            o.insert("high".into(), (*high).into());
        }
        Kind::Text { max } => {
            o.insert("kind".into(), "text".into());
            o.insert("max".into(), (*max).into());
        }
        Kind::Choice { of } => {
            o.insert("kind".into(), "choice".into());
            o.insert("of".into(), of.clone().into());
        }
        Kind::Several { of } => {
            o.insert("kind".into(), "several".into());
            o.insert("of".into(), of.clone().into());
        }
        Kind::Keyhash => {
            o.insert("kind".into(), "keyhash".into());
        }
    }
    v
}

/// One act as the declaration carries it.
pub fn act_json(a: &Act) -> serde_json::Value {
    serde_json::json!({
        "name": a.operation.name,
        "label": a.operation.label,
        "help": a.operation.help,
        "about": a.about.name(),
        "brings": a.brings.name(),
        "parameters": a.operation.parameters.iter().map(parameter_json).collect::<Vec<_>>(),
    })
}

/// **The whole declaration**: the vocabulary's version, the acts about the
/// node, and per resource the acts about it — including the ones its own
/// package declared, which travel in the same shape so a client draws one
/// idiom for both.
///
/// A package's own operation brings nothing and is about that resource;
/// invoking one is not wired yet (`rhtn_node::resources::RESERVED_ROLES`),
/// and it is declared here so that a client drawing this page draws what
/// the node draws rather than a shorter list.
pub fn declaration(
    acknowledging: bool,
    resources: &[(crate::operator::ResourceView, bool)],
) -> serde_json::Value {
    let mut acts: Vec<serde_json::Value> = node_acts(acknowledging).iter().map(act_json).collect();
    let mut per: Vec<serde_json::Value> = Vec::new();
    for (view, installed) in resources {
        let mut theirs: Vec<serde_json::Value> = resource_acts(view, *installed)
            .iter()
            .map(act_json)
            .collect();
        for op in &view.admin {
            theirs.push(act_json(&Act {
                operation: op.clone(),
                about: About::Resource,
                brings: Brings::Nothing,
            }));
        }
        per.push(serde_json::json!({
            "resource": view.resource.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "authority": view.authority,
            "installed": installed,
            "acts": theirs,
        }));
    }
    acts.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    serde_json::json!({
        "surface": SURFACE,
        "node": acts,
        "resources": per,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // acceptance: DMN-40
    #[test]
    fn every_declared_act_is_one_a_client_could_draw_and_the_node_would_read() {
        // the admission check a package's operation meets, met by the
        // node's own: a declaration that would render an unanswerable
        // field is the same fault whoever wrote it
        let mut names: Vec<String> = Vec::new();
        for a in node_acts(false).iter().chain(node_acts(true).iter()) {
            assert_eq!(
                a.operation.refused(),
                None,
                "{} is not admissible",
                a.operation.name
            );
            names.push(a.operation.name.clone());
            // **`key` is never declared**: what a person answers is
            // declared, what their client supplies is `brings`
            assert!(
                !a.operation.parameters.iter().any(|p| p.name == "key"),
                "{} declares the key as a field",
                a.operation.name
            );
        }
        // an act that needs the machine says so, and one that does not
        // does not
        let acts = node_acts(false);
        let brings = |n: &str| acts.iter().find(|a| a.operation.name == n).expect(n).brings;
        assert_eq!(brings("install"), Brings::HostKey);
        assert_eq!(brings("running"), Brings::HostKey);
        assert_eq!(brings("reload"), Brings::Nothing);
        assert_eq!(brings("stop"), Brings::Nothing);
        assert_eq!(brings("acknowledge"), Brings::Nothing);
    }

    // acceptance: DMN-40
    #[test]
    fn a_toggle_is_labelled_by_what_it_does_rather_than_by_what_is_true() {
        let on = node_acts(true);
        let off = node_acts(false);
        let label = |v: &[Act]| {
            v.iter()
                .find(|a| a.operation.name == "acknowledge")
                .expect("the toggle")
                .operation
                .label
                .clone()
        };
        assert!(label(&on).starts_with("Stop"), "{}", label(&on));
        assert!(label(&off).starts_with("Countersign"), "{}", label(&off));
    }

    // acceptance: DMN-40
    #[test]
    fn the_declaration_carries_its_version_and_every_kind_in_the_closed_set() {
        let d = declaration(false, &[]);
        assert_eq!(d["surface"], SURFACE);
        assert!(d["resources"].as_array().expect("an array").is_empty());
        let node = d["node"].as_array().expect("the node's acts");
        // sorted, so a client reading them twice reads them in one order
        let names: Vec<&str> = node.iter().map(|a| a["name"].as_str().unwrap()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
        // every parameter carries its kind and that kind's own bounds
        for a in node {
            for p in a["parameters"].as_array().unwrap() {
                let kind = p["kind"].as_str().expect("a kind");
                assert!(
                    ["flag", "number", "text", "choice", "several", "keyhash"].contains(&kind),
                    "{kind} is not in the closed set"
                );
                match kind {
                    "number" => {
                        assert!(p["low"].is_i64() && p["high"].is_i64());
                    }
                    "text" => assert!(p["max"].is_u64()),
                    "choice" | "several" => assert!(p["of"].is_array()),
                    _ => {}
                }
            }
        }
    }
}
