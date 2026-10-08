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
 * **From the capture on, the kernel drives and this flow follows.** The
 * ceremony's conversation runs by who the recipient is (`wire-format.md`
 * §7.10.1; `crates/client/src/sequence.rs`): the counterparty's leg over
 * the bearer, as a carriage the shell moves, and a witness's over the
 * end-to-end path, which the courier carries; and the four steps a
 * participant takes are the [Courier]'s: open, the queries, the gathered
 * responses, the body. This flow decides *when*, from the kernel's
 * [Progress] as the shell polls it, and the kernel holds no timer: the
 * proposer proposes when its queries are answered, and a witness that has
 * not answered by then is left out (design §7.1). The kernel reviews and
 * signs the body as it arrives, so by [Step.REVIEW] this device's
 * signature has gone; what the screen owes there is to show what was
 * signed and let the person stop watching.
 *
 * The passive roles — witness, verifier — have no place in this flow at
 * all: they are asked nothing and warned of nothing (design §19.6), and the
 * kernel answers them in its event loop, off any screen.
 */
class Meet(
    /** Who is being met, as the codes say it and not before: the responder
     *  learns it from the bootstrap, the initiator from the first optical
     *  code (`wire-format.md` §14.3.1; design §12.3). Null until then. */
    counterpartyKey: String? = null,
    /** A name for them where one is known; the key's first characters, or
     *  "the other person", otherwise. */
    counterpartyName: String? = null,
    /** Chosen with the intent, never later: meet only, or adopt in a
     *  direction, and whether a backup is being asked for. */
    val kind: Kind,
    /** Which side of the bootstrap: the initiator shows it, the responder
     *  reads it. */
    val role: Role = Role.INITIATOR,
    /** The retention this device declares at its intent, in whole years
     *  (design §7.5.1), as the kernel holds it: the brief states it. */
    val retentionYears: Long = 2,
    /** How long the flow waits on an unanswered query before going on
     *  without it, from the last answer or from the queries going out.
     *  The kernel runs no timer: when to propose is the proposer's own
     *  call (`crates/client/src/sequence.rs`), and this is it. */
    val patienceMs: Long = PATIENCE_MS,
) {
    companion object {
        /** One minute: long enough for a verifier two hops away to be
         *  reached and to answer, short enough to hold two people standing
         *  with their phones. */
        const val PATIENCE_MS = 60_000L

        /**
         * **How long the ceremony may make no progress before it is given
         * up on**, at the optical step and in the conversation. Not a
         * protocol timer — the design gives the proposer the say over when
         * to propose and no node runs a clock — but the shell refusing to
         * leave a person holding a phone that will never move again, which
         * two field runs did. Three minutes is far longer than any step
         * has taken; a figure to adjust if practice disagrees, not a
         * derived one.
         */
        const val STALE_MS = 180_000L

        /**
         * **How long one part of the optical window is shown for.** The
         * camera on the other side delivers about thirty frames a second
         * and decodes a third to three quarters of them (`qr.looks`,
         * 2026-10-07), so a hundred milliseconds — about three frames — is
         * what the worse phone catches most of the time, and a rotation of
         * eight parts takes under a second. Not tuned to the phones that
         * measured it [author, 2026-10-07]: a slower camera needs each
         * part held longer, not shorter (`OpticalExchange.WINDOW`).
         */
        const val TURN_MS = 100L
    }
    /** The adoption this meeting carries, as chosen at D1a. */
    val adopt: Adopt get() = kind.adopt

    private var knownKey: String? = counterpartyKey
    private val givenName: String? = counterpartyName

    /** The counterparty's keyhash as hex, once a code has named them. */
    val counterpartyKey: String? get() = synchronized(lock) { knownKey }

    /** How the screens name the counterparty: a given name, the key's
     *  first eight characters once known, or *the other person*. */
    val counterpartyName: String
        get() = givenName ?: synchronized(lock) { knownKey?.take(8) } ?: "the other person"

    /**
     * **A code named the counterparty.** The first one fixes them; a later
     * one naming anyone else is a party who is not the one in front of
     * you, and is refused with the reason. Null where it was taken.
     */
    fun counterparty(key: String): String? {
        synchronized(lock) {
            val k = knownKey
            if (k != null && k != key) return "that code is someone else's, not ${k.take(8)}'s"
            knownKey = key
            changed()
        }
        return null
    }

    /** The optical exchange under way at D2, the object cut into parts
     *  ([OpticalExchange]); null between the two. Held here so a redrawn
     *  screen finds it rather than starting over. */
    var exchange: OpticalExchange? = null

    /** Told when the person accepts the brief: the kernel's, so the
     *  ceremony begins there, which is the one question it asks. */
    var onAccepted: (() -> Unit)? = null
    private var begun = false

    /** The kernel's ceremony is open: the first code can be shown. */
    fun begun(): Boolean = synchronized(lock) { begun }

    /** The kernel's ceremony opened: redraw, since the first code can now
     *  be shown. */
    fun begunCeremony() {
        synchronized(lock) {
            begun = true
            changed()
        }
    }

    /** Where the meeting stands, as the screens sheet orders the steps. */
    enum class Step {
        /** The invitation: one device shows its code, the other reads it. */
        INTENT,
        /** The brief: what the record will hold, for the person to accept. */
        BRIEF,
        /** The optical exchange, and the intents over the bearer. */
        OPTICAL,
        /** The distance channels. */
        PROXIMITY,
        /** The guided captures. */
        CAPTURE,
        /** The verifier selection and the queries. */
        VERIFIERS,
        /** The proposed body, for every signer to review. */
        REVIEW,
        /** The record is finalised and held. */
        DONE,
        /** The meeting ended without one, and [stopReason] says why. */
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
        /** Which way a patronage action goes, if it is one. */
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
        /** The verifier's keyhash in hex. */
        val key: String,
        /** Why it was eligible. */
        val basis: Basis,
        /** Its verdict, once one has come back. */
        val verdict: Verdict? = null,
    )

    /** What a bound screen is told: re-read the accessors and redraw. */
    interface Ui {
        /** Re-read the accessors and redraw. */
        fun render()
    }

    /**
     * The kernel's conversation on the courier, as this flow drives it:
     * the four steps a participant takes (`crates/client/src/sequence.rs`)
     * and where the conversation stands. Each step answers null where it
     * went and the kernel's refusal otherwise, in the kernel's own words
     * (`Abort`'s debug form: `Waiting("...")`, `NoWitness`, and so on),
     * which this flow reads and the screen shows. **No binding here**: the
     * kernel implements this over `Participant`, and a test with a fake.
     */
    interface Courier {
        /** Ask every nominee to witness and hand over the back-pointers. */
        fun open(): String?

        /** Select the verifiers and put each query to the subject. */
        fun queries(): String?

        /** The responder hands the proposer its gathered responses. */
        fun gathered(): String?

        /** The initiator proposes the body to every signer. */
        fun propose(): String?

        /** Where the conversation stands, or null outside a ceremony. */
        fun progress(): Progress?
    }

    /** The kernel's `Progress`, with identifiers as hex. */
    data class Progress(
        /** Whether this side proposes: the initiator does. */
        val proposer: Boolean,
        /** Queries issued and not yet answered. */
        val queriesOutstanding: Int,
        /** Nominees that will attest. */
        val attesting: List<String> = listOf(),
        /** Nominees that declined. */
        val declined: List<String> = listOf(),
        /** Signers whose back-pointers are held. */
        val backFrom: List<String> = listOf(),
        /** Whether the counterparty's gathered responses are held. */
        val theirResponses: Boolean = false,
        /** Whether the body is out, or has been shown. */
        val proposed: Boolean = false,
        /** Signers whose entries are held, at the proposer. */
        val signed: List<String> = listOf(),
        /** Signers that refused, at the proposer. */
        val refused: List<String> = listOf(),
    )

    /** A nominee's answer to the request to witness, as it lands. */
    enum class Answer {
        ATTESTS,
        DECLINED,
    }

    /** One nominee, whose it was, and what it answered so far. */
    data class Nominee(val key: String, val mine: Boolean, val answer: Answer? = null)

    /**
     * One move of the conversation, as the kernel names it
     * (`Event.Conversed.step`, from `sequence.rs`'s `Conversed`): the
     * three the flow acts on, and the rest, which is the notice line's.
     * The code is `wire-format.md` §7.10.2's `SigningReply` field 2, 1 to
     * 4, and the words are the kernel's.
     */
    sealed class Turn {
        /** This device was shown the body and refused to sign it. */
        data class BodyRefused(val code: Int, val why: String) : Turn()

        /** A signer refused the body this device proposed. */
        data class SignerRefused(val signer: String, val code: Int, val why: String) : Turn()

        /** The record is finalized and held. */
        data class Finalized(val txid: String) : Turn()

        /** Everything else: consent, witness answers, back-pointers, a
         *  stranger's message refused. */
        object Other : Turn()
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
    /** The conversation on the courier, as far as this side has taken it:
     *  `opening` guards the one entry, `opened` is set once the queries are
     *  out, and nothing is polled before that, since a poll between the
     *  two would read no query outstanding and move too early. */
    private var opening = false
    private var opened = false
    private var gathered = false
    private var nomineesMine: List<String> = listOf()
    private var nomineesTheirs: List<String> = listOf()
    private var progress: Progress? = null
    /** What the kernel said it waits on, the last time it refused to
     *  propose; cleared once it does not. */
    private var waitingOn: String? = null
    /** The kernel would propose and no nominee has agreed to attest. */
    private var noWitness = false
    /** When the queries went out and when the last answer landed, on the
     *  caller's clock; what the patience is measured from. */
    private var queriesOutAt = 0L
    /** When the conversation last moved in any way this side can see: the
     *  clock [STALE_MS] runs against. */
    private var lastProgressAt = 0L

    /**
     * **When the optical exchange last gained anything**, which [STALE_MS]
     * runs against too: the longest step of the ceremony had no end until
     * one phone was restarted mid-exchange and the other went on reading a
     * dead screen [2026-10-07]. Any part arriving, either way, is progress.
     */
    private var lastOpticalAt = 0L
    private var lastAnswerAt = 0L
    /** The person said to go on without the unanswered queries. */
    private var goOn = false
    /** The flow went on with queries unanswered, and said so once. */
    private var wentOnWithout = 0
    /** Told when the meeting stops, from any live step: the kernel's, so
     *  its ceremony is abandoned the moment the screen's is. */
    var onStopped: (() -> Unit)? = null

    // ---- binding, same lifecycle discipline as Front --------------------

    fun bind(u: Ui) {
        synchronized(lock) {
            ui = u
            u.render()
        }
    }

    /** Unbind a screen: it is rendered no more. */
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

    /** The meeting's own log lines, oldest first. */
    fun log(): List<String> = synchronized(lock) { log.toList() }

    /** The finalised record's txid in hex, once there is one. */
    fun recordTxid(): String? = synchronized(lock) { recordTxid }

    /** Why the meeting stopped, where it did. */
    fun stopReason(): String? = synchronized(lock) { stopReason }

    // ---- the verifiers this device selected, and what came back ---------

    fun chosen(): List<Chosen> = synchronized(lock) { chosen.toList() }

    /** Whether the selection has been run at all, which is what tells a
     *  screen apart *no verifiers were required* from *not asked yet*. */
    fun selectionRun(): Boolean = synchronized(lock) { selectionRun }

    /** The verifiers queried about **me**, which the subject is owed
     *  (design §7.4.2) and the verifier is not (design §19.6). */
    fun queriesAboutMe(): List<String> = synchronized(lock) { queriesAboutMe.toList() }

    // ---- the conversation on the courier --------------------------------

    /** Where the kernel last said the conversation stood. */
    fun progress(): Progress? = synchronized(lock) { progress }

    /** Whether the conversation is open on the courier: the request to
     *  witness and the queries are out, and the flow polls from here. */
    fun opened(): Boolean = synchronized(lock) { opened }

    /** What the kernel waits on before it will propose, or null. */
    fun waitingOn(): String? = synchronized(lock) { waitingOn }

    /**
     * How long, on the caller's clock, before the flow goes on without the
     * queries still unanswered; zero once it would, or where the person
     * said to. Null while no query is outstanding or the flow is not open.
     */
    fun patienceLeftMs(nowMs: Long = System.currentTimeMillis()): Long? = synchronized(lock) {
        val p = progress ?: return null
        if (!opened || p.queriesOutstanding == 0) return null
        if (goOn) return 0L
        (maxOf(queriesOutAt, lastAnswerAt) + patienceMs - nowMs).coerceAtLeast(0L)
    }

    /** How many queries the flow went on without, or zero. */
    fun wentOnWithout(): Int = synchronized(lock) { wentOnWithout }

    /**
     * **Go on without the unanswered queries**, the person's control at the
     * verifiers: the next poll hands over or proposes with what has
     * answered. A verifier that has not answered by then does not appear
     * in the record (design §7.1), and the pre-sign warnings say how many.
     */
    fun goOn() {
        synchronized(lock) {
            if (step != Step.VERIFIERS) return
            goOn = true
            changed()
        }
    }

    /** Whether the kernel would propose and no nominee has agreed to
     *  attest: surfaced, because the person decides whether to wait. */
    fun noWitness(): Boolean = synchronized(lock) { noWitness }

    /** Every nominee, either side's, with what it has answered. */
    fun witnesses(): List<Nominee> = synchronized(lock) {
        val p = progress
        (nomineesMine.map { Nominee(it, true) } + nomineesTheirs.map { Nominee(it, false) })
            .map { n ->
                when {
                    p == null -> n
                    p.attesting.contains(n.key) -> n.copy(answer = Answer.ATTESTS)
                    p.declined.contains(n.key) -> n.copy(answer = Answer.DECLINED)
                    else -> n
                }
            }
    }

    /** The two nominee lists, as the kernel holds them: each side's from
     *  the other's neighbourhood (design §7.1). */
    fun nominated(mine: List<String>, theirs: List<String>) {
        synchronized(lock) {
            nomineesMine = mine.toList()
            nomineesTheirs = theirs.toList()
            changed()
        }
    }

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
    fun responded(key: String, verdict: Verdict, nowMs: Long = System.currentTimeMillis()) {
        synchronized(lock) {
            val i = chosen.indexOfFirst { it.key == key }
            if (i < 0) return
            chosen = chosen.toMutableList().also { it[i] = it[i].copy(verdict = verdict) }
            lastAnswerAt = nowMs
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
        Diag.event("meet.note", "line" to Diag.scrub(line))
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
                "the other, sealed under a key only that person can derive, for " +
                "the retention this phone declares, $retentionYears " +
                "${if (retentionYears == 1L) "year" else "years"}, and then deleted.",
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
        if (wentOnWithout > 0) {
            out += Item(
                "UX-003",
                "$wentOnWithout of ${chosen.size} verifiers had not answered when this " +
                    "device went on. They do not appear in the record, which carries " +
                    "that many fewer responses.",
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
        val hook = synchronized(lock) {
            check(step == Step.BRIEF) { "the brief is accepted from the brief, not $step" }
            briefAcknowledged = true
            stepped(Step.BRIEF, Step.OPTICAL, "tap")
            changed()
            onAccepted
        }
        // outside the lock: the kernel's begin crosses a thread
        hook?.invoke()
    }

    /** Refuse, from the brief, which is where a person may. */
    fun refuse() = stop("you refused the meeting")

    /** Whether the person has accepted the brief. */
    fun briefAcknowledged(): Boolean = synchronized(lock) { briefAcknowledged }

    /**
     * **The bootstrap crossed**: the initiator's QR was shown and read
     * (D1a/D1b), which is what completes D1 — not an intent arriving, which
     * cannot happen before the optical step whose contribution it echoes.
     */
    fun crossBootstrap() {
        synchronized(lock) {
            // read twice, or tapped after the camera read it: crossed once
            if (bootstrapCrossed || step != Step.INTENT) return
            bootstrapCrossed = true
            // the initiator taps that it was read; the responder's camera read it
            stepped(Step.INTENT, Step.BRIEF, if (role == Role.INITIATOR) "tap" else "camera")
            changed()
        }
    }

    /** Whether the invitation has crossed. */
    fun bootstrapCrossed(): Boolean = synchronized(lock) { bootstrapCrossed }

    private var intentSent = false
    private var intentReceived = false

    /**
     * **The optical step ends when both intents have crossed**: this
     * device's sent over the bearer, and the counterparty's received. On
     * either alone the proximity ladder would start on one phone while the
     * other was still sending, and the ladder holds the kernel's thread
     * for its tap window, so the intent behind it went out half a minute
     * late and the two tap windows never overlapped [2026-10-04]. A
     * mismatch of outcomes between two phones that tapped once is what the
     * kernel then refuses.
     */
    fun intentSent() = intentCrossed(sent = true)

    /** The counterparty's intent arrived over the bearer. */
    fun intentReceived() = intentCrossed(received = true)

    private fun intentCrossed(sent: Boolean = false, received: Boolean = false) {
        val both = synchronized(lock) {
            if (sent) intentSent = true
            if (received) intentReceived = true
            intentSent && intentReceived
        }
        if (both) moved(Step.OPTICAL, Step.PROXIMITY)
    }

    /** The kernel finished the distance channels. */
    fun proximityDone() = advance(Step.PROXIMITY, Step.CAPTURE, "kernel")

    /** Capture is the last hands-off step; the device comes back to the
     *  user for the verifier selection. */
    fun captureDone() = advance(Step.CAPTURE, Step.VERIFIERS, "kernel")

    /**
     * **Open the conversation** (design §7.1 step 6 and step 8), once, on
     * entering [Step.VERIFIERS]: the request to witness and the
     * back-pointers first, so a witness is holding the ceremony before
     * anything else of it arrives, then the queries to the subject. A
     * refusal of either stops the meeting with the kernel's reason: there
     * is no conversation to continue. Ends with one [poll], so a
     * conversation that has nothing to wait on moves at once.
     */
    fun converse(c: Courier, nowMs: Long = System.currentTimeMillis()) {
        synchronized(lock) {
            if (step != Step.VERIFIERS || opening) return
            opening = true
        }
        c.open()?.let { return stop("the conversation could not open: $it") }
        note("the nominees are asked to witness; back-pointers sent.")
        c.queries()?.let { return stop("the queries could not be put: $it") }
        note("the queries are with $counterpartyName for consent.")
        synchronized(lock) {
            opened = true
            queriesOutAt = nowMs
        }
        poll(c, nowMs)
    }

    /**
     * **The optical exchange moved**, which restarts its own staleness
     * clock ([lastOpticalAt]). Called by the screen for every part that
     * lands, either side's.
     */
    fun opticalMoved(nowMs: Long = System.currentTimeMillis()) = synchronized(lock) {
        lastOpticalAt = nowMs
    }

    /**
     * **Whether the optical exchange has stopped moving for good.** The
     * screen asks on each turn of its window; true is the counterparty
     * gone, and the ceremony ends rather than reading a dead screen.
     */
    fun opticalStale(nowMs: Long = System.currentTimeMillis()): Boolean = synchronized(lock) {
        if (step != Step.OPTICAL) return false
        if (lastOpticalAt == 0L) lastOpticalAt = nowMs
        nowMs - lastOpticalAt > STALE_MS
    }

    /**
     * **Where the conversation stands, and the step it earns.** Called on
     * the kernel's event loop, every tick and after every message of the
     * conversation, while the flow is at [Step.VERIFIERS] or [Step.REVIEW].
     *
     * At the verifiers, once no query is outstanding, or once the person
     * said to go on or the patience ran out with some still out: the
     * responder hands over what it gathered, once; the initiator proposes,
     * and is refused while the kernel waits on something (`Waiting`),
     * which is retried next poll, or because no nominee will attest
     * (`NoWitness`), which is the person's to wait out or stop. Any other
     * refusal stops the meeting with its code. The body out, or shown, is
     * what moves to review; a signer's refusal at the proposer arrives as
     * a message, in [conversed].
     *
     * **A verifier that never answers must not hold two people on a
     * step.** The kernel has no path by which an unanswered query becomes
     * `unavailable` at the one who issued it, and the design asks for none:
     * a verifier that does not answer within the ceremony simply does not
     * appear (design §7.1). So the wait is the flow's, measured on
     * `nowMs` from the queries going out or the last answer landing.
     */
    fun poll(c: Courier, nowMs: Long = System.currentTimeMillis()) {
        val live = synchronized(lock) { opened && (step == Step.VERIFIERS || step == Step.REVIEW) }
        if (!live) return
        val p = c.progress() ?: return
        val stale = synchronized(lock) {
            // **anything at all moving is progress** — `Progress` compares
            // by value, so this is every figure the conversation exposes
            if (p != progress || lastProgressAt == 0L) lastProgressAt = nowMs
            progress = p
            witnessesMine = p.attesting.count { nomineesMine.contains(it) }
            witnessesTheirs = p.attesting.count { nomineesTheirs.contains(it) }
            changed()
            nowMs - lastProgressAt > STALE_MS
        }
        // **a conversation that will never move again is not left to the
        // person** ([STALE_MS]). The counterparty may have stopped without
        // being able to say so: a carriage that would have told them is
        // the very thing that failed.
        if (stale) {
            return stop("nothing has moved for ${STALE_MS / 1000} s; the other phone may have stopped")
        }
        if (step() != Step.VERIFIERS) return
        if (p.proposed) {
            // the body is out, or has been shown and signed here
            moved(Step.VERIFIERS, Step.REVIEW)
            return
        }
        if (p.queriesOutstanding > 0) {
            val why = synchronized(lock) {
                val patient = nowMs - maxOf(queriesOutAt, lastAnswerAt) < patienceMs
                if (!goOn && patient) return
                val first = wentOnWithout == 0
                wentOnWithout = p.queriesOutstanding
                when {
                    !first -> null
                    goOn -> "you said to go on"
                    else -> "${patienceMs / 1000} s passed with no answer"
                }
            }
            if (why != null) {
                note("going on with ${p.queriesOutstanding} queries unanswered: $why; a verifier that has not answered does not appear.")
            }
        }
        if (!p.proposer) {
            val send = synchronized(lock) { if (gathered) false else { gathered = true; true } }
            if (send) {
                c.gathered()?.let { return stop("the responses could not be handed over: $it") }
                note("my gathered responses are with the proposer.")
            }
            return
        }
        val refused = c.propose()
        when {
            refused == null -> {
                synchronized(lock) { waitingOn = null; noWitness = false }
                note("the body is proposed and shown to every signer.")
                moved(Step.VERIFIERS, Step.REVIEW)
            }
            refused.startsWith("Waiting(") -> {
                val on = refused.removePrefix("Waiting(").removeSuffix(")").trim('"')
                val fresh = synchronized(lock) { (waitingOn != on).also { waitingOn = on } }
                if (fresh) note("not yet proposed: waiting on $on.")
            }
            refused == "NoWitness" -> {
                val fresh = synchronized(lock) { (!noWitness).also { noWitness = true; changed() } }
                if (fresh) note("no nominee has agreed to attest; the body waits on one.")
            }
            else -> stop("the body could not be proposed: $refused")
        }
    }

    /**
     * **A message of the conversation arrived**: the move the kernel names
     * ([Turn], from `Event.Conversed.step`) and its words for the notice
     * line. The record is what ends the flow. A refusal of the body, by
     * this device of the one shown it or by a signer of the one this
     * device proposed, ends the ceremony and the code is shown: a body one
     * signer refuses is a record that will not finalize. The rest is the
     * notice line's.
     */
    fun conversed(what: String, turn: Turn) {
        when (turn) {
            is Turn.Finalized -> finalized(turn.txid)
            is Turn.BodyRefused ->
                stop("this device refused the body: ${turn.why} (refusal ${turn.code})")
            is Turn.SignerRefused ->
                stop("${turn.signer.take(8)} refused to sign: ${turn.why} (refusal ${turn.code})")
            Turn.Other -> note(what)
        }
    }

    /**
     * **The record is finalized and held**: the flow is done. From the
     * verifiers or the review, since the body and the record can land in
     * one tick; ignored once stopped, which is terminal.
     */
    fun finalized(txid: String) {
        synchronized(lock) {
            if (step == Step.DONE || step == Step.STOPPED) return
            check(step == Step.VERIFIERS || step == Step.REVIEW) { "a record finalizes from the conversation, not $step" }
            recordTxid = txid
            stepped(step, Step.DONE, "kernel")
            changed()
        }
    }

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

    /** Stop for a reason — a refusal, a denial, the counterparty leaving.
     *  Honest and terminal, from any live step. */
    fun stop(reason: String) {
        val hook = synchronized(lock) {
            if (step == Step.DONE || step == Step.STOPPED) return
            Diag.warn("meet.stop", "from" to step, "reason" to Diag.scrub(reason))
            stopReason = reason
            step = Step.STOPPED
            changed()
            onStopped
        }
        // outside the lock: the kernel's abandon crosses a thread
        hook?.invoke()
    }

    /** `trigger` is who moved it: a tap, the camera, or the kernel. */
    private fun advance(from: Step, to: Step, trigger: String) {
        synchronized(lock) {
            check(step == from) { "cannot go $from -> $to from $step" }
            stepped(from, to, trigger)
            changed()
        }
    }

    /**
     * The kernel's move, from its own thread: taken where the flow is at
     * `from`, and nothing otherwise, since a person may have stopped the
     * meeting from the screen between the kernel's reading and its move.
     */
    private fun moved(from: Step, to: Step): Boolean = synchronized(lock) {
        if (step != from) return false
        stepped(from, to, "kernel")
        changed()
        true
    }

    /** Under [lock]: every transition, as the diagnostics see it. */
    private fun stepped(from: Step, to: Step, trigger: String) {
        step = to
        Diag.event("meet.step", "from" to from, "to" to to, "trigger" to trigger)
    }
}
