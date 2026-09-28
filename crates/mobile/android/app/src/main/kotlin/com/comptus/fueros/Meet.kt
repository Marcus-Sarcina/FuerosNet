package com.comptus.fueros

/**
 * The Meet flow: the ceremony's steps and the gating the screens sheet
 * ruled (section D, author 2026-09-27). Pure state — no Android, no
 * binding, no participant — so a JVM test can hold it, and process-scoped
 * (the kernel owns the live one) so a font change mid-ceremony does not
 * lose it.
 *
 * The order is design §7.1's, and two rules shape it:
 *
 *  - **Everything a person reads or decides is front-loaded.** The intent
 *    and its adoption choice come at [Step.INTENT]; every warning and every
 *    disclosure choice at [Step.BRIEF]. Past the brief the device faces the
 *    counterparty and takes no input — [Step.OPTICAL] through [Step.CAPTURE]
 *    are **hands-off** — so nothing that needs the user may live there.
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
     *  direction. */
    val adopt: Adopt,
) {

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

    interface Ui {
        fun render()
    }

    private val lock = Any()
    private var ui: Ui? = null
    private var step = Step.INTENT
    private var briefAcknowledged = false
    private val log = mutableListOf<String>()
    private var recordTxid: String? = null
    private var stopReason: String? = null

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

    // ---- progress: each step hands to the next -------------------------

    /** A line for the screen to show as the flow moves. */
    fun note(line: String) {
        synchronized(lock) {
            log.add(line)
            changed()
        }
    }

    /**
     * Leave the brief for the hands-off phase. **This is the last gate the
     * user passes before the device faces away**, so it is the one place
     * that must have shown every warning and taken every choice.
     */
    fun acknowledgeBrief() {
        synchronized(lock) {
            check(step == Step.BRIEF) { "the brief is acknowledged from the brief, not $step" }
            briefAcknowledged = true
            step = Step.OPTICAL
            changed()
        }
    }

    fun briefAcknowledged(): Boolean = synchronized(lock) { briefAcknowledged }

    /** The intent was carried and the counterparty's taken: to the brief. */
    fun intentExchanged() = advance(Step.INTENT, Step.BRIEF)

    fun opticalDone() = advance(Step.OPTICAL, Step.PROXIMITY)

    fun proximityDone() = advance(Step.PROXIMITY, Step.CAPTURE)

    /** Capture is the last hands-off step; the device comes back to the
     *  user for the verifier selection. */
    fun captureDone() = advance(Step.CAPTURE, Step.VERIFIERS)

    fun verifiersDone() = advance(Step.VERIFIERS, Step.REVIEW)

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
