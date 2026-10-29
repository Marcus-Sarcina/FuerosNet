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
 *    faces the counterparty and takes no input — [Step.OPTICAL] and
 *    [Step.WINDOW] are **hands-off** — so nothing that needs the user may
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
    /** How long [converse] waits between asking the kernel again for a
     *  window it refused with `Waiting` — the counterparty's keep of this
     *  side's witness set, still crossing the bearer. */
    val openRetryMs: Long = OPEN_RETRY_MS,
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

        /**
         * **How often [converse] asks again for a window the kernel
         * refused with `Waiting`.** The thing it waits on is the
         * counterparty's keep of this side's witness set, which crosses
         * the bearer like any other leg: nodeless on the bench the witness
         * sets took **29 seconds** to cross between two TCLs, landing long
         * after the channels were done (A6, 2026-10-23). A second between
         * asks covers that inside [PATIENCE_MS] at sixty asks, rather than
         * the six hundred a [TURN_MS] cadence would make — and a client
         * review has already found this shell asking the kernel 28 times
         * in 3 s to be too much [2026-10-11].
         */
        const val OPEN_RETRY_MS = 1_000L
    }
    /** The adoption this meeting carries, as chosen at D1a. */
    val adopt: Adopt get() = kind.adopt

    private var knownKey: String? = counterpartyKey
    private val givenName: String? = counterpartyName
    private var heardName: String? = null
    private var helloSent = false

    /** The counterparty's keyhash as hex, once a code has named them. */
    val counterpartyKey: String? get() = synchronized(lock) { knownKey }

    /** How the screens name the counterparty: a given name, **what they
     *  call themselves** where that has crossed — on the invitation for
     *  the party that scanned it, on the channel for the other [author,
     *  2026-10-29] — the key's first eight characters once known, or
     *  *the other person*. */
    val counterpartyName: String
        get() = givenName
            ?: synchronized(lock) { heardName ?: knownKey?.take(8) }
            ?: "the other person"

    /** The counterparty said what they call themselves. Blank is nothing. */
    fun heard(alias: String?) {
        val a = alias?.trim()?.takeIf { it.isNotEmpty() } ?: return
        synchronized(lock) { heardName = a }
        changed()
    }

    /** Say this device's name once the channel is there, once. */
    private fun sayHello(c: Courier) {
        if (synchronized(lock) { helloSent.also { helloSent = true } }) return
        c.hello()?.let { note("the name could not be sent: $it") }
    }

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
    /** The exchange whose object has gone to the kernel ([handOff]). */
    private var handedOff: OpticalExchange? = null

    /**
     * **An exchange's object goes to the kernel once.** Two paths can
     * see it complete — a read and the tracking rows — and the second
     * must not hand it over again: on the bench a transcript handed twice
     * opened two bearers, and the intent went to the one that was not
     * ready (2026-10-11). True for the first caller, false after.
     */
    fun handOff(x: OpticalExchange): Boolean = synchronized(lock) {
        if (handedOff === x) false else { handedOff = x; true }
    }

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
        /** **The window** (design §7.1 items 4–6): the capture, the verifier
         *  selection and queries, and the conversation, concurrently, from
         *  the request that opens it to the body that ends it. */
        WINDOW,
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
     * **A subnet this device holds a position in**, as the patron's
     * chooser lists it (`light-client-requirements.md` §7): its id, its
     * alias where held, and this device's own alias there. [OWN] is the
     * one under this device's own key where no adoption has named it yet.
     */
    data class Scope(val id: String, val alias: String?, val ownAlias: String?) {
        /** What the chooser shows. */
        fun label(): String = when {
            this == OWN -> "a new subnet under your own key"
            alias != null -> alias
            else -> "subnet ${id.take(8)}…"
        }

        companion object {
            /** A new subnet under this device's own key. */
            val OWN = Scope("", null, null)

            /** Joining as an individual: no scope brought. */
            val INDIVIDUAL = Scope("-", null, null)
        }
    }

    /**
     * **The one question a scope raises before anything is shown**
     * [author, 2026-10-15]: *adopt into which scope?* for the would-be
     * patron, *submit as which scope?* for the would-be subordinate.
     * Asked between the choice of transaction and the invitation for the
     * initiator, and between the scan and the acceptance for the party
     * that read it; asked of nobody who has nothing to choose.
     */
    enum class Choice {
        ADOPT_INTO,
        SUBMIT_AS,
    }

    /**
     * **What an invitation claimed about where it leads** [author,
     * 2026-10-15]: the subnet, and the inviter's alias and the subnet's as
     * the inviter states them. **A claim until the signed profile replaces
     * it**, and the brief says so.
     */
    data class Claim(val subnet: String?, val patronAlias: String?, val subnetAlias: String?) {
        /** "*[patron alias]*'s *[subnet alias]*", with what is missing said:
         *  a subnet with no alias by its id, and a lone root's by "a new
         *  subnet under …" [bench, 2026-10-15: "X's a new subnet" read badly]. */
        fun label(counterpartyName: String): String {
            val who = patronAlias ?: counterpartyName
            return when {
                subnetAlias != null -> "$who's $subnetAlias"
                subnet != null -> "$who's subnet ${subnet.take(8)}…"
                else -> "a new subnet under $who"
            }
        }
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
     * meeting by default with a peering offered beside it, or a patronage
     * action in a named direction.
     */
    data class Kind(
        /** Which way a patronage action goes, if it is one. */
        val adopt: Adopt = Adopt.NONE,
        /**
         * **Whether this meeting also carries a peering** — the checkbox
         * on the regular meeting (design §6.3; A3).
         *
         * It was `askBackup`, a flag that put one line in the brief and
         * did nothing else [bench, 2026-10-18]: the box asked the other
         * person to hold a backup, which design §3.4 makes **what a
         * peering record entitles the two of you to**, so the box was
         * promising a transaction the flow never made. It asks for the
         * peering now, and the backup is what follows from it.
         */
        val peering: Boolean = false,
        /** Which subnet the other is being invited into, where this
         *  device is the patron and chose at D1a; the responder as patron
         *  chooses at its brief ([chooseScope]). */
        val scope: Scope? = null,
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
     * (`Abort`'s debug form: `Waiting("...")` and so on),
     * which this flow reads and the screen shows. **No binding here**: the
     * kernel implements this over `Participant`, and a test with a fake.
     */
    interface Courier {
        /** **Engage this side's own nominees** (kind 20) [author,
         *  2026-10-19]: sent with the intent, before the counterparty has
         *  accepted anything, and retried while a nominee's session
         *  material is fetched. */
        fun engage(): String?

        /** **One event of the optical exchange's shape** to this side's
         *  nominees (kind 21), as it happens and never batched: the
         *  arrival pace on their clocks is the evidence. */
        fun trace(e: OpticalExchange.Event): String?

        /** Open the window with the request, its field 5 closing each
         *  engagement, and hand over the back-pointers. */
        fun open(): String?

        /** This device's own name to the counterparty, on the channel
         *  the handover left, once the window is open. */
        fun hello(): String?

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
        /** Surviving witnesses, of either side, whose answer has not
         *  arrived: what a patient proposer waits on inside the window
         *  [author, 2026-10-19].  Never a gate — there is no floor — but
         *  the count the flow paces its proposal by. */
        val witnessesOutstanding: Int = 0,
        /** Whether this side has opened the conversation already. */
        val opened: Boolean = false,
        /** Whether it has handed the proposer what it gathered already. */
        val handed: Boolean = false,
        /** Whether proposing would now produce a body: asked, never found
         *  out by proposing [client review, 2026-10-11]. */
        val proposable: Boolean = false,
        /** Whether a proposal is otherwise ready and no nominee has
         *  attested. */
        val noWitness: Boolean = false,
        /** What a proposal waits on, to show a person. */
        val waitingOn: String? = null,
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

        /** The record is finalized and held, naming these witnesses. */
        data class Finalized(val txid: String, val witnesses: List<String> = listOf()) : Turn()

        /** Everything else: consent, witness answers, back-pointers, a
         *  stranger's message refused. */
        object Other : Turn()
    }

    private val lock = Any()
    private var ui: Ui? = null
    private var step = Step.INTENT
    private var briefAcknowledged = false
    private var bootstrapCrossed = false
    private var scope: Scope? = kind.scope
    private var claim: Claim? = null
    private var carrying: Scope? = null
    /** What the counterparty proposed, where they proposed the opposite. */
    private var theirAdopt: Adopt? = null
    private var theirPeering = false
    private var direction: Adopt? = null
    /** Facts the pre-sign warnings are drawn from, as the flow learns them. */
    private var witnessesMine = 0
    private var witnessesTheirs = 0
    private var familiarOffered = false
    private val log = mutableListOf<String>()
    private var recordTxid: String? = null
    /** The witnesses the finalized record names, from its signed body:
     *  what the Done screen reads, since the conversation's last progress
     *  snapshot can predate the witness's signature (bench, 2026-10-25). */
    private var recordWitnesses: List<String>? = null
    private var stopReason: String? = null
    private var chosen: List<Chosen> = listOf()
    private var selectionRun = false
    private val queriesAboutMe = mutableListOf<String>()
    /** **This side's own entry to the conversation**, which is not what
     *  the kernel's `opened` means: the kernel knows the conversation is
     *  open once `open()` has gone, and the flow must not act between
     *  that and the queries — a poll in the gap reads no query
     *  outstanding and moves too early. `opening` guards the one entry,
     *  `entered` says both calls went.
     *
     *  What the flow used to track and no longer does: handed over,
     *  waiting on, and no witness, all of which are the conversation's
     *  and arrive in [Progress] [client review, 2026-10-11]. */
    private var opening = false
    private var entered = false
    private var nomineesMine: List<String> = listOf()
    private var nomineesTheirs: List<String> = listOf()
    private var progress: Progress? = null
    /** Whether an arrival has come in since the last poll: what makes the
     *  flow try again, rather than the turn of a timer. */
    private var arrived = false
    /** The last thing said about waiting, and whether the witnessless
     *  notice has gone out: so a state that holds for a minute is said
     *  once rather than every time the flow looks. */
    private var said: String? = null
    private var saidNoWitness = false
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

    /** Render whatever is bound: what something beside the flow — the
     *  adoption at D7 — calls when its own state moved. */
    fun rendered() = synchronized(lock) { changed() }

    // ---- accessors ------------------------------------------------------

    fun step(): Step = synchronized(lock) { step }

    /**
     * The meeting's own log lines, oldest first.
     *
     * **No screen draws these** [author, 2026-10-18]. Every line goes to
     * the diagnostic stream as it is made ([note]), which is where a bench
     * run is read; the list is here so a test can assert that something
     * was noted once and not twice. Drawing it put the controls above the
     * fold on every render, which is why it came off the screen.
     */
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
    fun opened(): Boolean = synchronized(lock) { entered }

    /** What the kernel waits on before it will propose, or null. */
    fun waitingOn(): String? = synchronized(lock) { progress?.waitingOn }

    /**
     * How long, on the caller's clock, before the flow goes on without the
     * queries still unanswered; zero once it would, or where the person
     * said to. Null while no query is outstanding or the flow is not open.
     */
    fun patienceLeftMs(nowMs: Long = System.currentTimeMillis()): Long? = synchronized(lock) {
        val p = progress ?: return null
        if (!entered || p.queriesOutstanding == 0) return null
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
            if (step != Step.WINDOW) return
            goOn = true
            changed()
        }
    }

    /** Whether the kernel would propose and no nominee has agreed to
     *  attest: surfaced, because the person decides whether to wait. */
    fun noWitness(): Boolean = synchronized(lock) { progress?.noWitness == true }

    /**
     * **Who attested in the end, and whose side they were nominated by**
     * [author, 2026-10-19]: the survivors of the prune that signed.  The
     * `nominated_by` split is the record's own, and a person is owed it —
     * a record attested only by the counterparty's survivors is one this
     * side of the story did not witness, which design §6.1.1 leaves
     * visible rather than mandatory.  Empty where nobody attested, which
     * is a valid record saying so permanently (there is no floor).
     */
    fun attested(): List<Nominee> = synchronized(lock) {
        // the record's own word once it is held; the snapshot before
        recordWitnesses?.let { named ->
            return@synchronized named.map { k -> Nominee(k, k in nomineesMine, Answer.ATTESTS) }
        }
        val signed = progress?.signed.orEmpty().toSet()
        witnessesLocked().filter { it.answer == Answer.ATTESTS && it.key in signed }
    }

    /** Every nominee, either side's, with what it has answered. */
    fun witnesses(): List<Nominee> = synchronized(lock) { witnessesLocked() }

    private fun witnessesLocked(): List<Nominee> = run {
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
            arrived = true
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
        if (peeringAsked()) {
            out += Item(
                "UX-001",
                "This meeting also makes a peering between you and " +
                    "$counterpartyName: an edge where neither of you is over the " +
                    "other, and what lets either of you hold an encrypted backup " +
                    "of the other's contacts and records — not your biometrics " +
                    "and not your secrets, which never leave this device.",
            )
        }
        // PRD-02: an adoption moves authority, and the resources whose
        // predicates reach upward are named before it is agreed.
        //
        // **What the two people are being asked about, not what this
        // device chose** [bench, 2026-10-15]: the kind is the initiator's
        // and the responder reads it off the bootstrap, so a responder
        // whose own D1a asked for nothing was shown a regular meeting's
        // brief while the other phone proposed to adopt it. The brief is
        // where that is said or it is said nowhere.
        when (settled()) {
            // both asked to be the patron, which is not settled until D7
            // (PRD-05): the meeting carries one of the two and the
            // consequences of each are what there is to warn about
            null -> {
                out += Item(
                    "PRD-02",
                    "You both asked to be the other's patron. One of you will be, " +
                        "and the two of you choose which after the record — so both " +
                        "of the next two items may turn out to be yours.",
                )
                out += Item(
                    "PRD-02",
                    "If $counterpartyName becomes your patron they hold authority " +
                        "over your position, and resources whose access reaches " +
                        "upward will reach through them.",
                )
                out += Item(
                    "PRD-02",
                    "If you become theirs, your outage cascades to them and to " +
                        "anyone beneath them, which is their risk and not only yours.",
                )
            }
            Adopt.NONE -> {}
            Adopt.ME_UNDER_THEM -> {
                // UX-016: which subnet, by the inviter's claim, said before
                // the person accepts [author, 2026-10-15]
                out += Item(
                    "UX-016",
                    claim?.let { c ->
                        "You are being invited into ${c.label(counterpartyName)} — as " +
                            "$counterpartyName's invitation names it; the names are their " +
                            "claim until their signed profile replaces them."
                    } ?: "You are being invited into a subnet $counterpartyName has not named; " +
                        "their device will say which before you sign.",
                )
                carrying?.let { c ->
                    out += Item(
                        "UX-016",
                        if (c == Scope.INDIVIDUAL) {
                            "You join as an individual: any scope you root stands as it is."
                        } else {
                            "You bring your scope ${c.label()} with you: everyone in it comes " +
                                "to sit in the subnet you are joining, and it stands alone again " +
                                "if you leave."
                        },
                    )
                }
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
                    "UX-016",
                    scope?.let { s ->
                        "You are inviting $counterpartyName into " +
                            (if (s == Scope.OWN) "a new subnet under your own key" else s.label()) + "."
                    } ?: "Which of your subnets $counterpartyName joins is asked before you accept.",
                )
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
        out
    }

    /** Facts the pre-sign warnings read, as the flow learns them. */
    fun learned(
        mine: Int = witnessesMine,
        theirs: Int = witnessesTheirs,
        familiar: Boolean = familiarOffered,
    ) {
        synchronized(lock) {
            witnessesMine = mine
            witnessesTheirs = theirs
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

    /** Whether the request has gone and the window is open. */
    private var windowOpen = false
    private var intentSent = false
    private var intentReceived = false

    /**
     * **The optical step ends when both intents have crossed**: this
     * device's sent over the bearer, and the counterparty's received. On
     * either alone the next step would start on one phone while the
     * other was still sending (when that step was the tap's ladder it held
     * the kernel's thread for its window, so the intent behind it went out
     * half a minute late and the two windows never overlapped
     * [2026-10-04]). A
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
        if (both) moved(Step.OPTICAL, Step.WINDOW)
    }

    /**
     * **The capture is sealed** [author, 2026-10-24]: the window has elapsed
     * on this device's clock and the frames are sealed under the
     * counterparty's key. Not a step — the capture ran inside the window
     * alongside the verification (design §7.1 item 5) — but the fact the
     * proposal and the handover wait on, since a party whose capture is
     * not sealed has nothing to hold of the counterparty.
     */
    fun captureSealed() {
        synchronized(lock) {
            if (captureSealed) return
            captureSealed = true
            Diag.event("meet.capture", "sealed" to true)
            changed()
        }
    }

    /** Whether this device's capture is sealed. */
    fun isCaptureSealed(): Boolean = synchronized(lock) { captureSealed }

    private var captureSealed = false

    /**
     * **Open the four-minute window** (design §7.1 item 4) [author,
     * 2026-10-19], once the optical exchange is through and the
     * counterparty's prune has answered: the request goes to this side's
     * surviving nominees, carrying the ceremony-id and closing the
     * engagement each has been watching, and this signer's back-pointers
     * go with it.
     *
     * **Everything after it runs inside the window** — the capture and the
     * verification, concurrently — so each leg reaches the survivors on
     * their own clocks as it is sent, which is what a witness attests
     * from. Refused with the kernel's `Waiting` while the keep has not
     * arrived, and retried on the next turn; any other refusal ends the
     * meeting. Whether the window is open.
     */
    fun openWindow(c: Courier): Boolean {
        synchronized(lock) {
            if (windowOpen) return true
            if (opening) return false
            opening = true
        }
        val why = c.open()
        if (why != null) {
            synchronized(lock) { opening = false }
            // the prune has not answered yet: the kernel says so, and the
            // next turn asks again
            if (why.startsWith("Waiting(")) return false
            stop("the window could not open: $why")
            return false
        }
        synchronized(lock) {
            windowOpen = true
            opening = false
            changed()
        }
        sayHello(c)
        synchronized(lock) {
        }
        note("the window is open: the witnesses that survived the prune are asked, back-pointers sent.")
        return true
    }

    /**
     * **Everything the turn loop's heartbeat needs to say where this flow
     * is and why it is not polling**, in one line.
     *
     * A6 went silent at the verifiers three times and each diagnosis cost
     * a round of forensics over a log that recorded the last thing that
     * happened and nothing about the waiting (2026-10-23). What was
     * missing was never the state — every field below was already held —
     * but a line that said it while nothing was happening. `due` is
     * [pollDue]'s own answer, so the heartbeat and the decision cannot
     * drift apart.
     */
    fun turnState(nowMs: Long = System.currentTimeMillis()): String = synchronized(lock) {
        val p = progress
        val since = maxOf(lastProgressAt, queriesOutAt)
        "step=$step entered=$entered opening=$opening window=$windowOpen " +
            "arrived=$arrived due=${pollDue(nowMs)} " +
            "patienceLeftMs=${patienceLeftMs(nowMs)} " +
            "sinceProgressMs=${if (since == 0L) -1L else nowMs - since} " +
            "outstanding=${p?.queriesOutstanding} proposer=${p?.proposer} " +
            "proposed=${p?.proposed} handed=${p?.handed} " +
            "attesting=${p?.attesting?.size} waitingOn=${p?.waitingOn}"
    }

    /**
     * **A carriage of the conversation was taken from the bearer.**
     * Nodeless there are no kernel events for the conversation: its
     * messages arrive as carriages and are ingested by the shell's own
     * drain, so nothing else tells this flow that anything landed, and
     * [pollDue] stayed false for ever. A6 reached the verifiers with the
     * counterparty's responses *taken and held* and never proposed, on a
     * conversation that was not stuck at all (2026-10-23).
     */
    fun carried() = synchronized(lock) {
        arrived = true
        changed()
    }

    /** Whether the window is open: the request has gone, and the capture
     *  and the verification run inside it. */
    fun windowOpen(): Boolean = synchronized(lock) { windowOpen }

    /**
     * **Put the queries, inside the window** (design §7.1 item 6), once,
     * as the window opens at [Step.WINDOW] ([openWindow]); where a caller
     * drove the flow without it — a test — it is opened here, since there
     * is no conversation without the request. A refusal stops the
     * meeting with the kernel's reason. Ends with one [poll], so a
     * conversation that has nothing to wait on moves at once.
     */
    fun converse(c: Courier, nowMs: Long = System.currentTimeMillis()) {
        synchronized(lock) {
            if (step != Step.WINDOW || opening || entered) return
            opening = true
        }
        if (!synchronized(lock) { windowOpen }) {
            // **`Waiting` is the kernel's "not yet", not a refusal.** What
            // it waits on is the counterparty's keep of this side's witness
            // set, and that crosses the bearer like any other leg — so it
            // can land after the channels are done, which is where
            // [openWindow] has stopped being called. This fallback used to
            // end the meeting on the first `Waiting`, and a nodeless run
            // between two TCLs died on exactly that: the sets took 29 s to
            // cross and the keeps were given 48 ms (A6, 2026-10-23).
            // Waited out here, on this thread — the keep arrives on the
            // kernel's own — and bounded by the flow's patience, because a
            // keep that never comes must not hold two people on a step
            // either. Any other refusal still ends the meeting at once.
            val deadline = nowMs + patienceMs
            var why = c.open()
            while (why != null && why.startsWith("Waiting(") &&
                System.currentTimeMillis() < deadline
            ) {
                Thread.sleep(openRetryMs)
                why = c.open()
            }
            if (why != null) {
                synchronized(lock) { opening = false }
                return stop("the conversation could not open: $why")
            }
            synchronized(lock) { windowOpen = true }
            sayHello(c)
        }
        note("the nominees are asked to witness; back-pointers sent.")
        // **the queries wait on the capture's template** the same way the
        // open waits on the keep: the hold yields it on its own clock, and
        // a keep that arrived first put the queries before the template
        // was there — refused as NotActive then, and the meeting ended with
        // a verifier selected (run 8bc1291-b1-node-1, GXV, 2026-10-25).
        // Bounded by the flow's patience, as the open is
        val queriesDeadline = nowMs + patienceMs
        var put = c.queries()
        while (put != null && put.startsWith("Waiting(") &&
            System.currentTimeMillis() < queriesDeadline
        ) {
            Thread.sleep(openRetryMs)
            put = c.queries()
        }
        put?.let { return stop("the queries could not be put: $it") }
        note("the queries are with $counterpartyName for consent.")
        synchronized(lock) {
            entered = true
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
     * conversation, while the flow is at [Step.WINDOW] or [Step.REVIEW].
     *
     * At the verifiers, once no query is outstanding, or once the person
     * said to go on or the patience ran out with some still out: the
     * responder hands over what it gathered, once; the initiator proposes,
     * and is refused while the kernel waits on something (`Waiting`),
     * which is retried next poll. **A missing witness refuses nothing**
     * [author, 2026-10-19]: there is no floor, and a record nobody
     * attests finalises saying so. Any other
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
        val live = synchronized(lock) { entered && (step == Step.WINDOW || step == Step.REVIEW) }
        if (!live) return
        val p = c.progress() ?: return
        synchronized(lock) { arrived = false }
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
        if (step() != Step.WINDOW) return
        // **said before any branch can return**, on both sides, once.
        // This sat in the `when` at the end, after the proposer branch —
        // which proposes and returns in the same poll that first sees the
        // body proposable, so the statement was unreachable there, and a
        // responder never had `noWitness` at all (A6, 2026-10-24).
        // Developer wording until the UX pass [author, 2026-10-24]: the
        // state, not a message.
        if (p.proposed) {
            // the body is out, or has been shown and signed here
            moved(Step.WINDOW, Step.REVIEW)
            return
        }
        // **nothing leaves this device for the record before its capture is
        // sealed** [author, 2026-10-24]: the handover and the proposal both
        // wait on the window elapsing here, said once
        if (!synchronized(lock) { captureSealed }) {
            val fresh = synchronized(lock) { (said != "the capture").also { said = "the capture" } }
            if (fresh) note("not yet proposed: the capture's window is still open on this device.")
            return
        }
        // **said once the window has closed, not before**: a witness's
        // answer can land during the window, and this line said *none*
        // seven milliseconds before one attested, then stood for the rest
        // of a meeting a witness signed (run 8bc1291-b1-node-1, 2026-10-25)
        if (p.noWitness) {
            val fresh = synchronized(lock) { (!saidNoWitness).also { saidNoWitness = true; changed() } }
            if (fresh) note("state: no witness attests this meeting — witnesses=0, formation=false; record field 4 absent, permanent.")
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
            if (!p.handed) {
                c.gathered()?.let { return stop("the responses could not be handed over: $it") }
                note("my gathered responses are with the proposer.")
            }
            return
        }
        // **the kernel says whether a body would come of it.** Asking by
        // proposing and reading the refusal had the proposer calling 28
        // times in 3 s on the bench [client review, 2026-10-11]; what it
        // waits on is in [Progress] for the person to read, not for this
        // to parse.
        if (p.proposable) {
            val refused = c.propose()
            if (refused != null) {
                // between the two calls an arrival can move the
                // conversation past proposing, which is not an error
                if (!refused.startsWith("Waiting(")) {
                    return stop("the body could not be proposed: $refused")
                }
                return
            }
            note("the body is proposed and shown to every signer.")
            moved(Step.WINDOW, Step.REVIEW)
            return
        }
        // what it waits on is the kernel's word and the screen shows
        // it; said once, not once per turn
        if (p.waitingOn != null) {
            val on = p.waitingOn
            val fresh = synchronized(lock) { (said != on).also { said = on } }
            if (fresh) note("not yet proposed: waiting on $on.")
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
        synchronized(lock) { arrived = true }
        when (turn) {
            is Turn.Finalized -> finalized(turn.txid, turn.witnesses)
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
    fun finalized(txid: String, witnesses: List<String> = listOf()) {
        synchronized(lock) {
            if (step == Step.DONE || step == Step.STOPPED) return
            check(step == Step.WINDOW || step == Step.REVIEW) { "a record finalizes from the conversation, not $step" }
            recordTxid = txid
            recordWitnesses = witnesses
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

    /**
     * **Whether the inviter asked for a peering**, as its bootstrap said
     * (`Bootstrap.Read.peering`). Only a responder is told this; an
     * initiator knows its own answer.
     */
    fun theirPeering(theirs: Boolean) {
        synchronized(lock) {
            theirPeering = theirs
            changed()
        }
    }

    /**
     * **Whether this meeting carries a peering: the inviter's choice.**
     *
     * The box is on the inviter's screen and the other person scans
     * (§7), so the kind is the initiator's and the responder reads it off
     * the bootstrap — the same rule the adoption's direction follows.
     * **The responder's own `kind` is not the answer** [bench,
     * 2026-10-19]: it was consulted, so a responder never learned that a
     * peering had been asked for, its brief said nothing about one, and
     * its screen showed nothing after the record while the inviter's
     * device waited for back-pointers that were never coming.
     */
    fun peeringAsked(): Boolean = synchronized(lock) {
        if (role == Role.INITIATOR) kind.peering else theirPeering
    }

    /**
     * **What the two people's choices came to**, in this device's own
     * frame, or null while an opposition is still to be resolved.
     *
     * **The responder's question to answer**, since it is the only side
     * holding both choices: its own from its D1a and the initiator's from
     * the bootstrap it read. The initiator is told the answer
     * ([Adoption]'s direction frame) rather than working it out, which it
     * could not do.
     */
    fun settled(): Adopt? = synchronized(lock) {
        direction?.let { return it }
        if (opposedAdoptions()) return null
        // **the kind is the initiator's choice and the responder's to
        // refuse** [author, 2026-09-29], and the brief is where it was
        // accepted: so what the two came to is the initiator's, mirrored
        // into this device's frame. Where no bootstrap was read this side
        // *is* the initiator, and its own choice is the one.
        when (theirAdopt) {
            Adopt.THEM_UNDER_ME -> Adopt.ME_UNDER_THEM
            Adopt.ME_UNDER_THEM -> Adopt.THEM_UNDER_ME
            Adopt.NONE -> Adopt.NONE
            null -> kind.adopt
        }
    }

    /** The subnet this device, as patron, invites the other into, once
     *  chosen. */
    fun scope(): Scope? = synchronized(lock) { scope }

    /** As the patron: which subnet. The responder chooses at its brief,
     *  the initiator at D1a (`light-client-requirements.md` §7). */
    fun chooseScope(s: Scope) {
        synchronized(lock) {
            scope = s
            changed()
        }
    }

    /** The scope this device brings under the patron, once chosen:
     *  [Scope.INDIVIDUAL] where it joins with none. */
    fun carrying(): Scope? = synchronized(lock) { carrying }

    /** As the party being adopted: which scope, if any, it submits as. */
    fun chooseCarrying(s: Scope) {
        synchronized(lock) {
            carrying = s
            changed()
        }
    }

    /**
     * **Which choice this device owes before it goes on**, given the
     * scopes it holds (`rooted`: those it could bring; `held`: those it
     * could adopt into), or null where none is owed. A device with
     * nothing to choose is answered for it here, so the screen never
     * asks a question with one answer.
     */
    fun pendingChoice(held: List<Scope>, rooted: List<Scope>): Choice? = synchronized(lock) {
        val side = settled() ?: return null
        when (side) {
            Adopt.THEM_UNDER_ME -> {
                if (scope != null) return null
                if (held.isEmpty()) {
                    scope = Scope.OWN
                    return null
                }
                Choice.ADOPT_INTO
            }
            Adopt.ME_UNDER_THEM -> {
                if (carrying != null) return null
                if (rooted.isEmpty()) {
                    carrying = Scope.INDIVIDUAL
                    return null
                }
                Choice.SUBMIT_AS
            }
            Adopt.NONE -> null
        }
    }

    /** What the invitation claimed about where it leads, where one was
     *  read. */
    fun claim(): Claim? = synchronized(lock) { claim }

    /** The responder read the initiator's claim off the bootstrap. */
    fun invitedBy(c: Claim) {
        synchronized(lock) {
            claim = c
            changed()
        }
    }

    /**
     * **The label of the subnet this adoption is into**, as this device
     * can say it: as patron, the scope it chose; as the party being
     * adopted, the inviter's claim; null where neither is known yet.
     */
    fun subnetLabel(): String? = synchronized(lock) {
        when (settled()) {
            Adopt.THEM_UNDER_ME -> scope?.let { s ->
                if (s == Scope.OWN) "a new subnet under your own key" else s.label()
            }
            Adopt.ME_UNDER_THEM -> claim?.label(counterpartyName)
            else -> null
        }
    }

    /**
     * **The same label, in the counterparty's frame** [bench, 2026-10-15]:
     * what the direction frame carries, so the other side reads "a new
     * subnet under *me*" and not "under your own key". `me` is how this
     * device is named to them: its alias in the scope, or its keyhash's
     * first characters.
     */
    fun labelForCounterparty(me: String): String? = synchronized(lock) {
        when (settled()) {
            Adopt.THEM_UNDER_ME -> scope?.let { s ->
                val id = s.id.takeIf { it.length == 64 }
                Claim(id, s.ownAlias ?: me, s.alias).label(me)
            }
            Adopt.ME_UNDER_THEM -> claim?.label(counterpartyName)
            else -> null
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
            // the opposition and its answer, as the diagnostics see them: the
            // run that first produced it left only a 22 s gap to read it by
            // (cb53936-a2-opposed-1, 2026-10-24)
            Diag.event("meet.direction", "mine" to kind.adopt, "theirs" to theirAdopt, "chosen" to d)
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
        steppedAtMs = System.currentTimeMillis()
        Diag.event("meet.step", "from" to from, "to" to to, "trigger" to trigger)
    }

    /** When the step last changed: the clock the capture step is judged
     *  stale against. */
    private var steppedAtMs = System.currentTimeMillis()

    /**
     * **Whether a step with no clock of its own has stopped moving for
     * good.** The exchange has [opticalStale] and the conversation its
     * progress; the capture waits on the counterparty's carriage
     * with nothing watching — on the bench one phone's client died at
     * capture and the other waited at it for ever (2026-10-11). A step
     * held longer than [STALE_MS] is the other phone gone.
     */
    fun stepStale(nowMs: Long = System.currentTimeMillis()): Boolean = synchronized(lock) {
        step == Step.WINDOW && !windowOpen && nowMs - steppedAtMs > STALE_MS
    }

    /**
     * **Whether the flow has anything to do now.** The conversation is the
     * kernel's and moves when something arrives; a turn of a timer is not
     * a reason to ask it again [client review, 2026-10-11]. So: an arrival
     * since the last look, or the patience run out with queries still
     * unanswered, which is the one deadline that is the flow's own.
     *
     * The staleness deadline is not here: [stepStale] is the caller's to
     * watch on its own clock, and it has to run whether or not anything
     * arrived — that being the case it exists for.
     */
    fun pollDue(nowMs: Long = System.currentTimeMillis()): Boolean = synchronized(lock) {
        if (!entered || (step != Step.WINDOW && step != Step.REVIEW)) return false
        // **the staleness guard has to be reachable with nothing arriving.**
        // [poll] is what notices that nothing has moved for [STALE_MS] and
        // ends the meeting, so gating every poll on an arrival or on a
        // patience put the watchdog behind the very condition it exists to
        // catch: with no verifier selected there are no queries, so
        // `patienceLeftMs` is null, and this reduced to `arrived` alone.
        // A6 sat on the Verifiers screen indefinitely rather than stopping
        // after three minutes (2026-10-23).
        val since = maxOf(lastProgressAt, queriesOutAt)
        arrived || patienceLeftMs(nowMs) == 0L ||
            (since != 0L && nowMs - since > STALE_MS)
    }
}
