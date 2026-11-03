//! **Why a launched machine's node is not answering** [author,
//! 2026-11-02].
//!
//! ## The defect this is written against
//!
//! Two nodes were launched from a phone and neither served. What the
//! client said was *"the surface at 104.248.71.30:7449 did not answer
//! within eight minutes; the machine is running and can be enrolled by
//! hand or destroyed"* — which was true, named no cause, and offered a
//! hand enrolment that cannot work when there is no node to enrol. The
//! operator was left with a rented machine, no node, and nothing to read.
//!
//! **The cause was two levels away from anything the client said.**
//! `cloud-init` had failed at `podman pull`, because the registry's token
//! endpoint was behind a challenge no container engine can pass, so no
//! container was ever created. Finding that took an ssh session, and an
//! expired root password was blocking even that — **so for the operator
//! this path exists for, whose only window is the app, it was not findable
//! at all.**
//!
//! ## What a client can honestly establish, and what it cannot
//!
//! It cannot read the machine's log: that wants a shell, and the app has
//! none. What it *can* do is open two sockets and look at a clock, which
//! is enough to separate the cases that call for different actions:
//!
//! | ssh | surface | after the window | what it means |
//! |---|---|---|---|
//! | no | no | no | still coming: a first boot installs an engine and pulls the image |
//! | no | no | yes | the machine is not answering at all — the provider's, not ours |
//! | yes | no | — | **the machine is up and its node did not start** |
//! | — | yes | — | it is serving; there was no silence |
//!
//! **The third row is the one that was silent**, and it is the one with a
//! short list of causes. So this says them, in the order they happen, and
//! says the one place the truth is — which the operator can reach, because
//! the key for it is derived from their own seed and their client already
//! shows it (`light-client-requirements.md` §10).
//!
//! **It names what is likely and does not claim to know.** A client that
//! said "the image could not be pulled" would be asserting something it
//! did not look at. What it knows is that the node did not start and what
//! the three reasons are; which one it was is on the machine.

/// What the client reached at the machine, and how long it waited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probed {
    /// Whether the machine's ssh port answered.
    pub ssh: bool,
    /// Whether the node's administration surface answered.
    pub surface: bool,
    /// How long the client has been waiting, in seconds.
    pub waited_secs: u64,
    /// Whether the window the client was willing to wait has run out.
    pub gave_up: bool,
}

/// **How long a first boot may reasonably take** before the client stops
/// calling it progress: an engine installed from the distribution's
/// mirrors and an image of some hundreds of megabytes pulled.
///
/// A number for the words rather than for a decision — the waiting is the
/// caller's, and this is what the caller says it is waiting for.
pub const FIRST_BOOT_SECS: u64 = 240;

/// What the silence was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Silence {
    /// Nothing is wrong yet: too early to say.
    StillComing,
    /// The machine itself is not answering.
    MachineSilent,
    /// **The machine is up and its node did not start.**
    NodeDidNotStart,
    /// There was no silence.
    Serving,
}

/// Read the probe.
pub fn why(p: &Probed) -> Silence {
    if p.surface {
        return Silence::Serving;
    }
    if p.ssh {
        // **ssh answering settles it, whatever the clock says**: the
        // machine has booted and is running sshd, so what is missing is
        // the node and not the machine. Saying so early is the whole
        // point — the operator stops waiting for something that is not
        // coming.
        return Silence::NodeDidNotStart;
    }
    if p.gave_up {
        Silence::MachineSilent
    } else {
        Silence::StillComing
    }
}

/// **What to tell the operator**, as lines.
///
/// `host` is the machine's address and `id` the provider's own id for it,
/// because every one of these ends in something they may want to do to
/// that machine.
pub fn said(p: &Probed, host: &str, provider: &str, id: &str) -> Vec<String> {
    match why(p) {
        Silence::Serving => vec![format!("{host} is serving")],
        Silence::StillComing => vec![format!(
            "waiting for {host}: a first boot installs a container engine and pulls the \
             node's image, which takes a few minutes"
        )],
        Silence::MachineSilent => vec![
            format!(
                "{host} is not answering at all after {}s — neither its node nor its ssh port.",
                p.waited_secs
            ),
            format!(
                "That is the machine rather than the node: it may still be starting, or \
                 {provider} may have given it an address it is not reachable at. Machine {id} \
                 is running and is yours to destroy."
            ),
        ],
        Silence::NodeDidNotStart => vec![
            format!(
                "{host} answers ssh and its node has not opened a surface in {}s, so the \
                 machine is up and the node did not start.",
                p.waited_secs
            ),
            "Three things stop it, in the order they happen: the image could not be pulled; \
             the configuration was refused; the container started and exited."
                .into(),
            // **the one place the truth is, and the operator can reach
            // it**: the key is derived from their own seed and this client
            // shows it (`light-client-requirements.md` §10)
            format!(
                "Which of them it was is in the machine's own log, and your node-control ssh \
                 key opens it: `ssh root@{} 'cloud-init status --long; podman ps -a; podman \
                 logs --tail 20 rhtnd'`. This client cannot read it for you.",
                host.rsplit_once(':').map_or(host, |(h, _)| h)
            ),
            format!(
                "Nothing is enrolled, so there is nothing to undo here. Machine {id} at \
                 {provider} is running and is yours to destroy."
            ),
        ],
    }
}

/// **The label the instance is remembered under**, so the list says what
/// is wrong afterwards and not only while the operator is watching.
pub fn label(p: &Probed) -> &'static str {
    match why(p) {
        Silence::Serving => "serving",
        Silence::StillComing => "launching",
        Silence::MachineSilent => "launched, not answering",
        Silence::NodeDidNotStart => "launched, its node did not start",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probed(ssh: bool, surface: bool, waited_secs: u64, gave_up: bool) -> Probed {
        Probed {
            ssh,
            surface,
            waited_secs,
            gave_up,
        }
    }

    // acceptance: PRD-23
    #[test]
    fn a_machine_that_answers_ssh_without_a_node_is_said_to_be_exactly_that() {
        let p = probed(true, false, 300, true);
        assert_eq!(why(&p), Silence::NodeDidNotStart);
        let lines = said(&p, "203.0.113.9:7449", "digitalocean", "123");
        let whole = lines.join("\n");
        // it says what is up and what is not
        assert!(whole.contains("answers ssh"), "{whole}");
        assert!(whole.contains("the node did not start"), "{whole}");
        // **the three causes, in the order they happen**, since which one
        // it was is not this client's to know
        assert!(whole.contains("could not be pulled"), "{whole}");
        assert!(whole.contains("configuration was refused"), "{whole}");
        assert!(whole.contains("started and exited"), "{whole}");
        // the one place the truth is, reachable with the key the operator
        // already holds, and the host without the surface's port on it
        assert!(whole.contains("ssh root@203.0.113.9 "), "{whole}");
        assert!(!whole.contains("root@203.0.113.9:7449"), "{whole}");
        assert!(whole.contains("cloud-init status"), "{whole}");
        assert!(whole.contains("logs --tail 20 rhtnd"), "{whole}");
        // and it does not claim to know which it was
        assert!(whole.contains("cannot read it for you"), "{whole}");
        // the machine is named, since every one of these ends in a choice
        // about that machine
        assert!(
            whole.contains("123") && whole.contains("digitalocean"),
            "{whole}"
        );
        assert_eq!(label(&p), "launched, its node did not start");
    }

    // acceptance: PRD-23
    #[test]
    fn ssh_answering_settles_it_whatever_the_clock_says() {
        // **the operator stops waiting for something not coming**: the
        // machine has booted and runs sshd, so the engine and the pull
        // have had their chance whether the window elapsed or not
        for waited in [30, 120, FIRST_BOOT_SECS, 900] {
            for gave_up in [false, true] {
                assert_eq!(
                    why(&probed(true, false, waited, gave_up)),
                    Silence::NodeDidNotStart,
                    "waited {waited}, gave_up {gave_up}"
                );
            }
        }
    }

    // acceptance: PRD-23
    #[test]
    fn a_machine_still_coming_is_not_a_machine_that_failed() {
        let early = probed(false, false, 45, false);
        assert_eq!(why(&early), Silence::StillComing);
        let whole = said(&early, "203.0.113.9:7449", "digitalocean", "123").join("\n");
        // what it is waiting for, so a wait is not a blank screen
        assert!(whole.contains("installs a container engine"), "{whole}");
        assert!(whole.contains("pulls"), "{whole}");
        // nothing is called a failure yet
        assert!(!whole.to_lowercase().contains("did not start"), "{whole}");
        assert_eq!(label(&early), "launching");

        // and once the window is out with nothing answering at all, it is
        // the machine and not the node
        let out = probed(false, false, 600, true);
        assert_eq!(why(&out), Silence::MachineSilent);
        let whole = said(&out, "203.0.113.9:7449", "digitalocean", "123").join("\n");
        assert!(whole.contains("not answering at all"), "{whole}");
        assert!(
            whole.contains("the machine rather than the node"),
            "{whole}"
        );
        assert_eq!(label(&out), "launched, not answering");
    }

    // acceptance: PRD-23
    #[test]
    fn a_surface_that_answers_is_no_silence_at_all() {
        for ssh in [false, true] {
            for gave_up in [false, true] {
                let p = probed(ssh, true, 90, gave_up);
                assert_eq!(why(&p), Silence::Serving);
                assert_eq!(label(&p), "serving");
            }
        }
    }
}
