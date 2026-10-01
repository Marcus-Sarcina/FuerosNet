package com.comptus.fueros

/**
 * The Meet flow: the ceremony's steps and the gating the screens sheet
 * ruled (section D, author 2026-09-27). Pure state — no Android, no
 * binding, no participant — so a JVM test can hold it, and process-scoped
 * (the kernel owns the live one) so a font change mid-ceremony does not
 * lose it.
 *
 * The order is design §7.1's, and three rules shape it:
 *
 *  - **Everything a person reads or decides is front-loaded.** The kind of
 *    transaction is chosen at [Step.INTENT] by the initiator; every warning
 *    and every disclosure choice at [Step.BRIEF]. Past the brief the device
 *    faces the counterparty and takes no input — [Step.OPTICAL] through
 *    [Step.CAPTURE] are **hands-off** — so nothing that needs the user may
 *    live there.
 *  - **D1 is two halves and two cameras** [author, 2026-09-29]. A
 *    **bootstrap** QR carries only this device's identifier and the kind of
 *    transaction, shown by the initiator and read by the responder with the
 *    **rear** camera. Nothing of the ceremony's own anchor crosses there.
 *    The **anchor** exchange is [Step.OPTICAL], mutual, on the **selfie**
 *    cameras. D1 completes on the bootstrap being crossed and both briefs
 *    being shown — not on an intent arriving, which cannot happen before
 *    the optical step it echoes.
 *  - **Review precedes signing.** [Step.REVIEW] shows the counterparty's
 *    selection and the pre-sign warnings; only from there is the record
 *    signed.
 *
 * The passive roles — witness, verifier — have no place in this flow at
 * all: they are asked nothing and warned of nothing (design §19.6), and the
 * kernel answers them in its event loop, off any screen.
 */
class Meet(
    val counterpartyKey: String,
    val counterpartyName: String,
    /** Chosen with the intent, never later: meet only, or adopt in a
     *  direction, and whether a backup is being asked for. */
    val kind: Kind,
    /** Which side of the bootstrap: the initiator shows it, the responder
     *  reads it. */
    val role: Role = Role.INITIATOR,
) {
    /** The adoption this meeting carries, as chosen at D1a. */
    val adopt: Adopt get() = kind.adopt

    enum class Step {
        INTENT,
        BRIEF,
        OPTICAL,
        PROXIMITY,
        CAPTURE,
        VERIFIERS,
        REVIEW,
        DONE,
        STOPPED,
    }

    /** The adoption the intent carries, if any. */
    enum class Adopt {
        NONE,
        THEM_UNDER_ME,
        ME_UNDER_THEM,
    }

    /**
     * Which side of the bootstrap this device is on, and therefore which
     * camera it uses and whether it chose the kind of transaction.
     *
     * **The initiator chooses the kind before the responder has seen
     * anything**, so the responder's brief is the first place they can
     * refuse it [author, 2026-09-29]. The role says nothing about the
     * record: §3.2 names two participants and no initiator.
     */
    enum class Role {
        INITIATOR,
        RESPONDER,
    }

    /**
     * What the meeting is for, as the author's dialogue puts it: a regular
     * meeting by default, optionally asking the other to hold a backup, or
     * a patronage action in a named direction.
     */
    data class Kind(
        val adopt: Adopt = Adopt.NONE,
        /** *"Ask this person to backup my user data"* — the checkbox. */
        val askBackup: Boolean = false,
    )

    /**
     * One thing a person must be told, with the row it answers to.
     *
     * **A warning is a fact about this ceremony, not a template.** Each is
     * emitted only where it holds, so a screen cannot show a reassurance it
     * has not earned, and a test can say which should appear.
     */
    data class Item(val row: String, val text: String)

    /**
     * Why this verifier was selected — `wire-format.md` §5.5's
     * `selection_basis`, in the order the tiers rank (design §8.1.2).
     * **The selector's own claim and nobody's to audit**, which is why the
     * screen shows it as a claim rather than as a credential.
     */
    enum class Basis {
        MET,
        IN_HORIZON,
        REACHABLE,
        DISCRETIONARY,
    }

    /** What a verifier answered about the counterparty, or nothing yet. */
    enum class Verdict {
        MATCH,
        NO_MATCH,
        INCONCLUSIVE,
        UNAVAILABLE,
    }

    /**
     * One verifier this device selected of the counterparty's pool, and
     * what has come back. A verifier is **asked nothing and told nothing**
     * (design §19.6); this is the selector's side of the exchange.
     */
    data class Chosen(
        val key: String,
        val basis: Basis,
        val verdict: Verdict? = null,
    )

    interface Ui {
        fun render()
    }

    private val lock = Any()
    private var ui: Ui? = null
    private var step = Step.INTENT
    private var briefAcknowledged = false
    private var bootstrapCrossed = false
    /** What the counterparty proposed, where they proposed the opposite. */
    private var theirAdopt: Adopt? = null
    private var direction: Adopt? = null
    /** Facts the pre-sign warnings are drawn from, as the flow learns them. */
    private var witnessesMine = 0
    private var witnessesTheirs = 0
    private var strongest: String? = null
    private var familiarOffered = false
    private val log = mutableListOf<String>()
    private var recordTxid: String? = null
    private var stopReason: String? = null
    private var chosen: List<Chosen> = listOf()
    private var selectionRun = false
    private val queriesAboutMe = mutableListOf<String>()

    // ---- binding, same lifecycle discipline as Front --------------------

    fun bind(u: Ui) {
        synchronized(lock) {
            ui = u
            u.render()
        }
    }

    fun unbind(u: Ui) {
        synchronized(lock) {
            if (ui === u) ui = null
        }
    }

    private fun changed() {
        ui?.render()
    }

    // ---- accessors ------------------------------------------------------

    fun step(): Step = synchronized(lock) { step }

    fun log(): List<String> = synchronized(lock) { log.toList() }

    fun recordTxid(): String? = synchronized(lock) { recordTxid }

    fun stopReason(): String? = synchronized(lock) { stopReason }

    /**
     * Whether this step takes user input. The hands-off phase — optical
     * exchange, proximity, capture — does not: the device faces away, so a
     * screen there shows what is happening and offers nothing to tap.
     */
    fun acceptsInput(): Boolean = synchronized(lock) {
        step == Step.INTENT ||
            step == Step.BRIEF ||
            step == Step.VERIFIERS ||
            step == Step.REVIEW
    }

    fun handsOff(): Boolean = synchronized(lock) {
        step == Step.OPTICAL || step == Step.PROXIMITY || step == Step.CAPTURE
    }

    // ---- the verifiers this device selected, and what came back ---------

    fun chosen(): List<Chosen> = synchronized(lock) { chosen.toList() }

    /** Whether the selection has been run at all, which is what tells a
     *  screen apart *no verifiers were required* from *not asked yet*. */
    fun selectionRun(): Boolean = synchronized(lock) { selectionRun }

    /** The verifiers queried about **me**, which the subject is owed
     *  (design §7.4.2) and the verifier is not (design §19.6). */
    fun queriesAboutMe(): List<String> = synchronized(lock) { queriesAboutMe.toList() }

    /**
     * The selection the kernel computed, in the order it returned. Empty is
     * a real answer and not a failure: `wire-format.md` §5.2 requires a
     * verifier only where the counterparty handed over records enough to
     * oblige one, so a thin bundle obliges none.
     */
    fun selected(v: List<Chosen>) {
        synchronized(lock) {
            chosen = v.toList()
            selectionRun = true
            changed()
        }
    }

    /**
     * A verifier answered. Recorded against the verifier it names and
     * ignored where that verifier was never selected here — a response to
     * a query this device did not issue is not this ceremony's business.
     */
    fun responded(key: String, verdict: Verdict) {
        synchronized(lock) {
            val i = chosen.indexOfFirst { it.key == key }
            if (i < 0) return
            chosen = chosen.toMutableList().also { it[i] = it[i].copy(verdict = verdict) }
            changed()
        }
    }

    /**
     * A query about me reached a verifier and was consented to on my
     * behalf. **Surfaced, not asked**: consent is bound to the ceremony
     * this person is standing in, so the client gives it without a
     * question (design §7.4.2) and owes them the telling.
     */
    fun querySurfaced(verifier: String) {
        synchronized(lock) {
            queriesAboutMe.add(verifier)
            changed()
        }
    }

    // ---- progress: each step hands to the next -------------------------

    /** A line for the screen to show as the flow moves. */
    fun note(line: String) {
        synchronized(lock) {
            log.add(line)
            changed()
        }
    }

    /**
     * **The brief, every item of it** (D1.5; UX-001, UX-002, PRD-02).
     *
     * This is the last screen before the device faces away, so it is the
     * one place that can tell the person anything. The list is what the
     * screen must render, and a test asserts the items this ceremony earns:
     * the durable fields and who reads them, the capture's own terms, and —
     * only where D1a chose one — the authority an adoption moves and the
     * resources whose predicates reach upward.
     */
    fun brief(): List<Item> = synchronized(lock) {
        val out = mutableListOf<Item>()
        // UX-001: what becomes durable, and who may later read it. At the
        // action, not in a policy document (`light-client-requirements.md`
        // §1.5).
        out += Item(
            "UX-001",
            "Your identity, the time, and that you and $counterpartyName met " +
                "become a durable record, readable by anyone either of you shows it to.",
        )
        out += Item(
            "UX-001",
            "The fields you disclose are chosen here and not later. What you " +
                "withhold is visible as withheld, never as absent.",
        )
        // UX-002: what the record contains and who can read it, at capture
        out += Item(
            "UX-002",
            "No image goes into the record. Each phone keeps its own capture of " +
                "the other, sealed under a key only that person can derive, for the " +
                "retention stated here and then deleted.",
        )
        out += Item(
            "UX-002",
            "From the next screen this phone faces $counterpartyName and takes " +
                "nothing from you until the capture is done.",
        )
        if (kind.askBackup) {
            out += Item(
                "UX-001",
                "You are asking $counterpartyName to hold an encrypted backup of " +
                    "your contacts and records — not your biometrics and not your " +
                    "secrets, which never leave this device.",
            )
        }
        // PRD-02: an adoption moves authority, and the resources whose
        // predicates reach upward are named before it is agreed
        when (kind.adopt) {
            Adopt.NONE -> {}
            Adopt.ME_UNDER_THEM -> {
                out += Item(
                    "PRD-02",
                    "$counterpartyName becomes your patron. They will hold authority " +
                        "over your position, and ending it later is your act alone — " +
                        "but what it costs you is listed below before you agree.",
                )
                out += Item(
                    "PRD-02",
                    "Resources whose access reaches upward through a patron will reach " +
                        "through $counterpartyName. None are known to this device yet, " +
                        "and a release build names each one here.",
                )
            }
            Adopt.THEM_UNDER_ME -> {
                out += Item(
                    "PRD-02",
                    "You become $counterpartyName's patron. Your outage cascades to " +
                        "them and to anyone beneath them, which is their risk and not " +
                        "only yours.",
                )
            }
        }
        out
    }

    /**
     * **The pre-sign warnings** (D6; UX-003): what is weak in this
     * ceremony, each only where it is true.
     *
     * **A degraded ceremony is presented as degraded and never as
     * malformed.** A record with no witnesses, a thin verifier set or a
     * weak channel is a legitimate record that says less; a screen calling
     * it broken would teach the person to distrust the honest thing.
     */
    fun presign(): List<Item> = synchronized(lock) {
        val out = mutableListOf<Item>()
        if (chosen.isEmpty() && selectionRun) {
            out += Item(
                "UX-003",
                "No verifier was required: $counterpartyName handed over no prior " +
                    "records, so there was no one to ask. The record carries that.",
            )
        }
        if (chosen.isNotEmpty() && !familiarOffered) {
            out += Item(
                "UX-003",
                "None of the verifiers you could ask is someone you have met. Their " +
                    "answers are worth what an unfamiliar voice is worth.",
            )
        }
        val unavailable = chosen.count { it.verdict == Verdict.UNAVAILABLE }
        if (unavailable > 0) {
            out += Item(
                "UX-003",
                "$unavailable of ${chosen.size} verifiers answered nothing. Silence " +
                    "counts neither for nor against, and the record shows the slot empty.",
            )
        }
        if (witnessesMine == 0 && witnessesTheirs == 0) {
            out += Item(
                "UX-003",
                "No witness attested this meeting. That makes it a formation record: " +
                    "real, and visibly uncorroborated.",
            )
        } else if (witnessesMine == 0 || witnessesTheirs == 0) {
            out += Item(
                "UX-003",
                "The witnesses are one-sided — $witnessesMine nominated by you and " +
                    "$witnessesTheirs by $counterpartyName. A reader can see the imbalance.",
            )
        }
        if (strongest == null) {
            out += Item(
                "UX-003",
                "No proximity channel passed. The meeting rests on what the two of " +
                    "you saw on each other's screens and nothing else.",
            )
        } else if (strongest != "UWB") {
            out += Item(
                "UX-003",
                "The strongest channel that passed was $strongest, not UWB. It is " +
                    "the strongest this hardware has, and the record says which it was.",
            )
        }
        out
    }

    /** Facts the pre-sign warnings read, as the flow learns them. */
    fun learned(
        mine: Int = witnessesMine,
        theirs: Int = witnessesTheirs,
        channel: String? = strongest,
        familiar: Boolean = familiarOffered,
    ) {
        synchronized(lock) {
            witnessesMine = mine
            witnessesTheirs = theirs
            strongest = channel
            familiarOffered = familiar
            changed()
        }
    }

    /**
     * Accept, which is what reveals the intention QR and the
     * turn-the-phone instruction [author, 2026-09-29].
     */
    fun accept() {
        synchronized(lock) {
            check(step == Step.BRIEF) { "the brief is accepted from the brief, not $step" }
            briefAcknowledged = true
            step = Step.OPTICAL
            changed()
        }
    }

    /** Refuse, from the brief, which is where a person may. */
    fun refuse() = stop("you refused the meeting")

    fun briefAcknowledged(): Boolean = synchronized(lock) { briefAcknowledged }

    /**
     * **The bootstrap crossed**: the initiator's QR was shown and read
     * (D1a/D1b), which is what completes D1 — not an intent arriving, which
     * cannot happen before the optical step whose contribution it echoes.
     */
    fun crossBootstrap() {
        synchronized(lock) {
            bootstrapCrossed = true
        }
        advance(Step.INTENT, Step.BRIEF)
    }

    fun bootstrapCrossed(): Boolean = synchronized(lock) { bootstrapCrossed }

    fun opticalDone() = advance(Step.OPTICAL, Step.PROXIMITY)

    fun proximityDone() = advance(Step.PROXIMITY, Step.CAPTURE)

    /** Capture is the last hands-off step; the device comes back to the
     *  user for the verifier selection. */
    fun captureDone() = advance(Step.CAPTURE, Step.VERIFIERS)

    fun verifiersDone() = advance(Step.VERIFIERS, Step.REVIEW)

    /**
     * **Two people proposing to adopt each other** (D7; PRD-05).
     *
     * Each chose a direction at their own D1a, before either had seen the
     * other's, so both choosing *"I will be the Patron"* is an ordinary
     * thing for two people to do and **not a protocol failure**. Nothing
     * malformed has happened: no adoption has been proposed yet, and the
     * two directions are two intentions that cannot both hold.
     *
     * The resolution is the one the author named: **ask the two to choose a
     * direction.** Until one is chosen the meeting stands — the presence
     * record is untouched by this, and a record without an adoption is a
     * complete record.
     */
    fun theirAdopt(theirs: Adopt) {
        synchronized(lock) {
            theirAdopt = theirs
            changed()
        }
    }

    /** Whether both sides proposed to adopt the other. */
    fun opposedAdoptions(): Boolean = synchronized(lock) {
        val theirs = theirAdopt ?: return false
        (kind.adopt == Adopt.THEM_UNDER_ME && theirs == Adopt.THEM_UNDER_ME) ||
            (kind.adopt == Adopt.ME_UNDER_THEM && theirs == Adopt.ME_UNDER_THEM)
    }

    /** The direction the two settled on, once they have. */
    fun direction(): Adopt? = synchronized(lock) { direction }

    /**
     * The direction chosen out of an opposition. Choosing [Adopt.NONE] is a
     * real answer: the two met and left the authority question alone, which
     * costs them nothing they had.
     */
    fun chooseDirection(d: Adopt) {
        synchronized(lock) {
            check(opposedAdoptions()) { "there is no opposition to resolve" }
            direction = d
            changed()
        }
    }

    /** Signed and finalized: the record's id, and the flow is done. */
    fun signed(txid: String) {
        synchronized(lock) {
            check(step == Step.REVIEW) { "a record is signed from review, not $step" }
            recordTxid = txid
            step = Step.DONE
            changed()
        }
    }

    /** Stop for a reason — a refusal, a denial, the counterparty leaving.
     *  Honest and terminal, from any live step. */
    fun stop(reason: String) {
        synchronized(lock) {
            if (step == Step.DONE || step == Step.STOPPED) return
            stopReason = reason
            step = Step.STOPPED
            changed()
        }
    }

    private fun advance(from: Step, to: Step) {
        synchronized(lock) {
            check(step == from) { "cannot go $from -> $to from $step" }
            step = to
            changed()
        }
    }
}
