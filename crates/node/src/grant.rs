//! **The predicate language, and the grants an operator writes in it.**
//!
//! `infra-client-requirements.md` §10.2 fixes the shape: the role table is
//! held, and a predicate is *a macro over it*. So nothing here runs at
//! request time — `Gateway::serve` reads a row (§10.1: "from the row, not
//! by evaluating a predicate"), and what this module does is write rows,
//! at §10.2's four moments, none of which is a request.
//!
//! **The vocabulary is closed, and that is the point.** §10.3 lists the
//! affordances an operator must have and `resource-requirements.md` §7.1
//! says why the list ends there: assignment is "a query language over
//! existing data, and that bounds what it can express, which for a
//! security-critical component is a feature". Every clause below is one of
//! §10.3's, evaluated against topology this node already holds plus its
//! own standing — no new state, nothing fetched, nothing on the wire
//! (§10.2: "Your predicate language is yours, and so is the table").
//!
//! **`resource-requirements.md` §7.2.1 names the classes, and all five are
//! here**, which is how to tell the vocabulary is complete rather than
//! merely short. They differ in "what an assignment depends on besides the
//! member, which is the whole of their security difference":
//!
//! | §7.2.1's class | Depends on | Here |
//! |---|---|---|
//! | Structural | the member's position relative to you | [`Clause::Clients`], [`Clause::Grandclients`], [`Clause::AtDistance`] |
//! | Tenure | the member's own history | [`Clause::JoinedBefore`] |
//! | Named | nothing but your choice | [`Clause::Named`] |
//! | Absolute rank | the member and those above them | [`Clause::MostTrusted`] |
//! | Relative rank | **the size of the population** | [`Clause::TopFraction`] |
//!
//! Only the last lets a third party move the answer without out-ranking
//! anybody, and §7.2.1 is explicit about what contains that: "the size and
//! character of the org, not a rule" — the table is small and its members
//! are people the operator recognises, so "statistical manoeuvring is a
//! weak attack against a population you recognise individually, which is
//! the reason the predicate language is allowed to stay this simple".

use crate::resources::{MAX_ROLES, RESERVED_ROLES};
use rhtn_archive::Keyhash;
use rhtn_archive::topology::Table;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// At most this many clauses in one grant.
pub const MAX_CLAUSES: usize = 8;

/// At most this many grants on one resource.
pub const MAX_GRANTS: usize = 32;

/// **One clause of §10.3's vocabulary**, read relative to the resource's
/// owner. A clause narrows; it never widens, because membership in the
/// owner's horizon is the outer gate every clause sits behind (§10.1) and
/// no clause can reach past it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clause {
    /// §10.3's "All direct clients": the owner's subordinates over open
    /// bindings. **Directed**, unlike the horizon's own walk.
    Clients,
    /// §10.3's "All clients and grand-clients": those, and theirs.
    Grandclients,
    /// §10.3's "Nodes at a given relative tier", read as a distance by the
    /// walk that defines the horizon (design §15.1.1: "the same walk that
    /// defines the region measures inside it").
    ///
    /// **Structural, in §7.2.1's sense**: it "depends on the member's
    /// position relative to you", and "nobody else's arrival or departure
    /// changes the answer for a given member" — which is true of a
    /// distance and would not be of a rank. The table carries no level
    /// number, so there is nothing else for a *relative* tier to measure
    /// against. Note that this walk crosses patron, subordinate and
    /// sibling edges alike, so distance 1 is a patron, a client **or** a
    /// sibling; [`Clause::Clients`] is the directed question.
    AtDistance {
        /// How many edges away, by that walk.
        edges: usize,
    },
    /// §10.3's "Nodes above a trust rank", as an **absolute** rank: the
    /// owner's `n` most trusted. §10.3 prefers this to a quantile where
    /// the operator's intent is absolute, because "my ten most trusted"
    /// "depends on nobody but the member and those above them".
    MostTrusted {
        /// How many of them.
        n: usize,
    },
    /// The same as a **quantile**: the most trusted `percent` of the
    /// population scored. §10.3 permits it and warns what it costs — the
    /// line moves with the size of the org, so a join changes rows
    /// belonging to nobody who joined (`resource-requirements.md` §7.2.1).
    TopFraction {
        /// The fraction, 1 to 100.
        percent: u8,
    },
    /// §10.3's "Nodes joined before a date": the earliest open adoption of
    /// this member opened before `when`.
    ///
    /// **A reissue does not reset it** — the earliest is taken, so moving
    /// a series does not re-date a tenure. The time is the adopting
    /// patron's own clock (`topology::Binding::from`), so this compares
    /// clocks that were never synchronised; it is the operator's
    /// affordance and reads as they meant it within one subnet.
    JoinedBefore {
        /// The date, in seconds.
        when: u64,
    },
    /// §10.3's "Named individuals, inside the membership gate": one
    /// member, by keyhash. `resource-requirements.md` §7.1.2 has these
    /// cover "the contractor who fits no group".
    Named {
        /// The member.
        who: Keyhash,
    },
}

/// **What an operator granted, and to whom.** The roles, and the clauses a
/// member must satisfy to hold them — a conjunction.
///
/// **Conjunction only.** §7.1's own list contains one compound affordance,
/// "Every node at [relative tier] with [trust above threshold]", and that
/// is an *and*. A second grant is the *or*, since a member matched by two
/// grants holds the union of their roles. There is no negation: no
/// affordance asks for one, and a language that can say *everyone except*
/// invites a grant whose meaning changes when somebody else joins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// What it grants. Application roles only, from those the package
    /// declared.
    pub roles: BTreeSet<String>,
    /// What a member must satisfy, all of it. **Empty admits every member
    /// of the owner's horizon** — §10.1's outer gate and nothing further,
    /// which is the simplest predicate there is.
    pub clauses: Vec<Clause>,
}

/// **Why a grant was refused**, at configuration time rather than at a
/// request (§10.2: "the operator hears about it at configuration time,
/// never a requester at request time").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantError {
    /// It names a role the package did not declare.
    Undeclared(String),
    /// It names `connect` or `discover`, which are the node's own
    /// (design §11.4).
    Reserved(String),
    /// Wider than a row can carry (`resource-requirements.md` §3).
    TooWide(usize),
    /// More clauses than one grant may hold.
    TooMany(usize),
    /// A clause whose bound is empty of meaning.
    Empty(&'static str),
}

impl fmt::Display for GrantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GrantError::Undeclared(r) => write!(f, "`{r}` is not a role this package declared"),
            GrantError::Reserved(r) => {
                write!(f, "`{r}` is reserved for the node's own evaluation")
            }
            GrantError::TooWide(n) => write!(f, "a row carries at most {MAX_ROLES} roles, not {n}"),
            GrantError::TooMany(n) => {
                write!(f, "a grant carries at most {MAX_CLAUSES} clauses, not {n}")
            }
            GrantError::Empty(why) => write!(f, "{why}"),
        }
    }
}

impl Grant {
    /// Every member of the owner's horizon, holding `roles`.
    pub fn standing(roles: BTreeSet<String>) -> Self {
        Grant {
            roles,
            clauses: Vec::new(),
        }
    }

    /// **Whether this grant is one the node will hold at all**, asked once
    /// when the operator writes it rather than once per member: the
    /// answer cannot differ between them.
    pub fn refused(&self, declared: &BTreeSet<String>) -> Option<GrantError> {
        if self.roles.len() > MAX_ROLES {
            return Some(GrantError::TooWide(self.roles.len()));
        }
        if self.clauses.len() > MAX_CLAUSES {
            return Some(GrantError::TooMany(self.clauses.len()));
        }
        for r in &self.roles {
            if RESERVED_ROLES.contains(&r.as_str()) {
                return Some(GrantError::Reserved(r.clone()));
            }
            if !declared.contains(r) {
                return Some(GrantError::Undeclared(r.clone()));
            }
        }
        for c in &self.clauses {
            match c {
                Clause::MostTrusted { n: 0 } => {
                    return Some(GrantError::Empty("`most-trusted` of nobody grants nothing"));
                }
                Clause::TopFraction { percent } if *percent == 0 || *percent > 100 => {
                    return Some(GrantError::Empty(
                        "`top-fraction` is a percentage, 1 to 100",
                    ));
                }
                Clause::AtDistance { edges } if *edges > 2 => {
                    return Some(GrantError::Empty(
                        "nothing sits further than two edges from the owner",
                    ));
                }
                _ => {}
            }
        }
        None
    }

    /// Whether any clause reads a rank, so a caller knows whether it has
    /// to compute one (design §16.2: standing is computed on demand).
    pub fn reads_rank(&self) -> bool {
        self.clauses
            .iter()
            .any(|c| matches!(c, Clause::MostTrusted { .. } | Clause::TopFraction { .. }))
    }

    /// Whether `who` satisfies every clause. **Membership is not asked
    /// here** — the caller has already established it, because it is the
    /// gate the clauses sit behind rather than one of them (§10.1).
    pub fn admits(
        &self,
        owner: &Keyhash,
        who: &Keyhash,
        table: &Table,
        standing: &Standing,
    ) -> bool {
        self.clauses
            .iter()
            .all(|c| c.admits(owner, who, table, standing))
    }
}

impl Clause {
    fn admits(&self, owner: &Keyhash, who: &Keyhash, table: &Table, standing: &Standing) -> bool {
        match self {
            Clause::Clients => table.subordinates(owner).contains(who),
            Clause::Grandclients => {
                let kids = table.subordinates(owner);
                kids.contains(who) || kids.iter().any(|k| table.subordinates(k).contains(who))
            }
            Clause::AtDistance { edges } => table.distance(owner, who, 2) == Some(*edges),
            // **a rank nobody computed admits nobody** rather than
            // everybody: a clause the node cannot evaluate is one it
            // declines to satisfy, since the alternative hands out a role
            // because a score was missing
            Clause::MostTrusted { n } => standing.rank(who).is_some_and(|r| r < *n),
            Clause::TopFraction { percent } => {
                let line = (standing.population() * (*percent as usize)).div_ceil(100);
                standing.rank(who).is_some_and(|r| r < line)
            }
            Clause::JoinedBefore { when } => table
                .bindings()
                .iter()
                .filter(|b| b.node == *who && b.open())
                .map(|b| b.from)
                .min()
                .is_some_and(|t| t < *when),
            Clause::Named { who: n } => n == who,
        }
    }
}

/// **The ranking a rank clause reads**, as of one computation.
///
/// design §16.2 has standing computed on demand and kept nowhere, so this
/// is not state: it is passed in at a re-score and dropped. It is built
/// once for the whole candidate set rather than once per member, which is
/// what the policy's own interface wants (`Policy::evaluate`, design
/// §16.2: "answers for a set in one computation").
#[derive(Debug, Clone, Default)]
pub struct Standing {
    place: BTreeMap<Keyhash, usize>,
}

impl Standing {
    /// Rank a scored set, most trusted first. Ties are broken by keyhash,
    /// so the order is total and a re-score of the same scores produces
    /// the same ranking.
    pub fn of(scores: &[(Keyhash, f64)]) -> Self {
        let mut v: Vec<(Keyhash, f64)> = scores.to_vec();
        v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        Standing {
            place: v
                .into_iter()
                .enumerate()
                .map(|(i, (k, _))| (k, i))
                .collect(),
        }
    }

    /// No ranking at all: every rank clause refuses.
    pub fn unknown() -> Self {
        Standing::default()
    }

    /// Where `who` placed, if it was scored.
    pub fn rank(&self, who: &Keyhash) -> Option<usize> {
        self.place.get(who).copied()
    }

    /// How many were scored, which is what a quantile's line is a fraction
    /// of.
    pub fn population(&self) -> usize {
        self.place.len()
    }
}

/// **What the operator reads back.** §10.4 requires a template to be
/// legible "before the click, in the vocabulary they use elsewhere", which
/// is only true if the vocabulary prints.
impl fmt::Display for Clause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Clause::Clients => write!(f, "my direct clients"),
            Clause::Grandclients => write!(f, "my clients and grand-clients"),
            Clause::AtDistance { edges } => write!(f, "nodes {edges} edges from me"),
            Clause::MostTrusted { n } => write!(f, "my {n} most trusted"),
            Clause::TopFraction { percent } => write!(f, "my most trusted {percent}%"),
            Clause::JoinedBefore { when } => write!(f, "nodes that joined before {when}"),
            Clause::Named { who } => write!(f, "the node {}", crate::diag::id8(who)),
        }
    }
}

impl fmt::Display for Grant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let roles = if self.roles.is_empty() {
            "nothing".to_string()
        } else {
            self.roles
                .iter()
                .cloned()
                .collect::<Vec<String>>()
                .join(", ")
        };
        if self.clauses.is_empty() {
            return write!(f, "{roles} to every member of my trust horizon");
        }
        let who: Vec<String> = self.clauses.iter().map(|c| c.to_string()).collect();
        write!(f, "{roles} to {}", who.join(", and "))
    }
}

/// **A grant the package shipped**, named and labelled so an operator can
/// take it in one click.
///
/// §10.4 requires exactly this and requires it to be legible: templates
/// "ship with the package", because "the author knows what *typical* means
/// for their application", and they are "expressed in the predicate
/// language, not as opaque configuration, so that an operator can see what
/// a one-click choice grants **before the click, in the vocabulary they use
/// elsewhere**". So a template is a [`Grant`] and nothing else — the same
/// object the operator would have written, which is what makes it
/// printable and editable rather than a black box with a button.
///
/// **Taking one is still the operator's decision.** §10.4: "a permission
/// default is a security decision, and the package author's incentive runs
/// toward breadth. Shrink-wrapping means trusting the author's judgment
/// about access, not merely their code." Nothing here is applied on
/// installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// Its name: `[a-z0-9_-]`, as a role name is.
    pub name: String,
    /// What the operator reads.
    pub label: String,
    /// What it would grant, in the operator's own vocabulary.
    pub grant: Grant,
}

/// At most this many templates in one manifest.
pub const MAX_TEMPLATES: usize = 16;

impl Template {
    /// Why the node will not hold it, where it will not.
    pub fn refused(&self, declared: &BTreeSet<String>) -> Option<String> {
        if !crate::resources::namelike(&self.name) {
            return Some(format!(
                "`{}`: a template name is [a-z0-9_-], 1 to 32 bytes",
                self.name
            ));
        }
        if self.label.is_empty() || self.label.len() > crate::resources::MAX_LABEL {
            return Some(format!(
                "`{}`: a template carries a label of 1 to {} characters",
                self.name,
                crate::resources::MAX_LABEL
            ));
        }
        if self.grant.roles.is_empty() {
            return Some(format!("`{}`: a template that grants nothing", self.name));
        }
        self.grant
            .refused(declared)
            .map(|e| format!("`{}`: {e}", self.name))
    }
}
