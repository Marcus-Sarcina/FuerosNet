package com.comptus.fueros

/**
 * **Another device of this identity, brought to the dataset** — the pure
 * part, so a test can hold it, as [Backup] and [Engagement] are.
 *
 * The protocol is the kernel's (`wire-format.md` §14.3.3; APP-014): two
 * contributions cross as a meeting's codes do, the pairing's key and id
 * derive from them inside the kernel, each holder is asked on their own
 * device, and the sealed dataset crosses the local radio and nothing
 * else. What this file owns is the shell's side of that: which device
 * offers the radio link and which seeks it, the order of the steps, and
 * the words the person reads.
 *
 * **The roles are the person's choice and nothing else decides them.**
 * The kernel does not know which device is behind — a fork is two heads
 * either way — so the person says which hands over and which takes, on
 * each device, and the two choices have to agree for a link to form at
 * all: the handing device advertises, the taking one scans.
 */
object Pairing {
    /** What this device does in the hand-over. */
    enum class Role {
        /** Hands everything it holds of the identity to the other. */
        HAND,

        /** Takes what the other holds into itself. */
        TAKE,
    }

    /** The steps, in the order they run. */
    enum class Step {
        /** The person chooses this device's role. */
        ROLE,

        /** The contributions cross: this device shows its own code and reads the other's. */
        CODES,

        /** The radio link under the pairing's id. */
        LINK,

        /** The sealed dataset crosses the link. */
        CARRIAGE,

        /** Done, with what it came to. */
        DONE,

        /** Stopped, with the reason. */
        STOPPED,
    }

    /** The one phase the dataset crosses under, on a link of its own. */
    const val PHASE = Carriage.Phase.INTENT

    /** How long the link is waited for once both devices hold the pairing. */
    const val LINK_READY_MS = 30_000L

    /** The handing device advertises; the taking device seeks. */
    fun offers(role: Role): Boolean = role == Role.HAND

    /** The step after `step`; the two ends stay where they are. */
    fun next(step: Step): Step = when (step) {
        Step.ROLE -> Step.CODES
        Step.CODES -> Step.LINK
        Step.LINK -> Step.CARRIAGE
        Step.CARRIAGE -> Step.DONE
        Step.DONE, Step.STOPPED -> step
    }

    /** What the person reads at each step. */
    fun narration(step: Step, role: Role?): String = when (step) {
        Step.ROLE ->
            "Bring another device of this identity to everything this one holds, " +
                "with the two side by side. Choose what this device does."
        Step.CODES ->
            "Hold the two screens facing each other. Each shows a code the other reads; " +
                "the pairing is made from both, on each device."
        Step.LINK -> when (role) {
            Role.HAND -> "Pairing made. Offering the radio link to the other device…"
            Role.TAKE -> "Pairing made. Looking for the other device's radio link…"
            null -> "Pairing made."
        }
        Step.CARRIAGE -> when (role) {
            Role.HAND -> "Linked. You will be asked before anything leaves this device."
            Role.TAKE -> "Linked. You will be asked before anything is taken into this device."
            null -> "Linked."
        }
        Step.DONE -> when (role) {
            Role.HAND -> "Handed over. The other device holds everything this one did."
            Role.TAKE -> "Taken."
            null -> "Done."
        }
        Step.STOPPED -> "Stopped."
    }

    /** What a take came to, as the person is told it. */
    fun taken(records: ULong, contacts: ULong, other: ULong, refused: ULong): String {
        val parts = listOf(
            "$records record${if (records == 1uL) "" else "s"}",
            "$contacts contact${if (contacts == 1uL) "" else "s"}",
            "$other other item${if (other == 1uL) "" else "s"}",
        )
        val base = "Taken: " + parts.joinToString(", ") + "."
        return if (refused == 0uL) {
            base
        } else {
            "$base $refused record${if (refused == 1uL) "" else "s"} did not verify or did not chain and " +
                (if (refused == 1uL) "was" else "were") + " left out."
        }
    }
}
