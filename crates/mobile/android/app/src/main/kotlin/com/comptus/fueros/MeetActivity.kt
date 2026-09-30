package com.comptus.fueros

import android.app.Activity
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.view.Gravity
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * Meet: the ceremony flow (screens sheet section D). One Activity walks the
 * whole sequence, because a ceremony is one continuous flow and not a set
 * of places you navigate between. The state lives in [Kernel]'s [Meet], so
 * a recreation mid-flow rejoins it.
 *
 * **What this build carries and what it cannot.** The flow, its front-
 * loaded brief, its hands-off phase and its review-before-sign gate are all
 * here and enforced by [Meet]. The steps that need the world are not
 * faked: the optical exchange needs a counterparty's screen in the camera,
 * and the bearer that carries the intent past the handshake is the shell's
 * to build (`wire-format.md` §14.3 fixed the encoding; the carriage is not
 * yet wired here); proximity needs the radios; capture needs a face in front
 * of the camera. Where a step needs reality, the screen says so. A dim
 * **walkthrough** control advances the flow for inspection — it is
 * scaffolding, not the ceremony, and it is labelled as standing in for a
 * signal a real device would raise.
 */
class MeetActivity : Activity() {

    private lateinit var body: LinearLayout
    private lateinit var scroll: ScrollView

    private val sink = object : Meet.Ui {
        override fun render() = runOnUiThread { redraw() }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        body = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        scroll = ScrollView(this).apply {
            addView(body)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.MATCH_PARENT,
            )
        }
        setContentView(
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                fitsSystemWindows = true
                setPadding(48, 48, 48, 48)
                addView(scroll)
            },
        )
    }

    override fun onStart() {
        super.onStart()
        Consent.host(this)
        Kernel.meet()?.bind(sink) ?: redraw()
    }

    override fun onStop() {
        Consent.release(this)
        Kernel.meet()?.unbind(sink)
        super.onStop()
    }

    private fun redraw() {
        body.removeAllViews()
        val m = Kernel.meet()
        if (m == null) {
            entry()
            return
        }
        title(
            when (m.step()) {
                Meet.Step.INTENT -> "Meet ${m.counterpartyName}"
                Meet.Step.BRIEF -> "Before you begin"
                Meet.Step.OPTICAL -> "Exchanging (1/3)"
                Meet.Step.PROXIMITY -> "Ranging (2/3)"
                Meet.Step.CAPTURE -> "Capturing (3/3)"
                Meet.Step.VERIFIERS -> "Verifiers"
                Meet.Step.REVIEW -> "Review and sign"
                Meet.Step.DONE -> "Done"
                Meet.Step.STOPPED -> "Stopped"
            },
        )
        when (m.step()) {
            Meet.Step.INTENT -> intent(m)
            Meet.Step.BRIEF -> brief(m)
            Meet.Step.OPTICAL -> handsOff(m, "the optical channel", "a counterparty's screen in the camera", m::opticalDone)
            Meet.Step.PROXIMITY -> handsOff(m, "the proximity radios", "UWB or NFC hardware", m::proximityDone)
            // leaving capture is what asks for the selection: once, at the
            // transition, so a recreated screen redraws without re-running it
            Meet.Step.CAPTURE -> handsOff(m, "the guided capture", "a face in front of the camera") {
                m.captureDone()
                Kernel.selectVerifiers()
            }
            Meet.Step.VERIFIERS -> verifiers(m)
            Meet.Step.REVIEW -> review(m)
            Meet.Step.DONE -> done(m)
            Meet.Step.STOPPED -> stopped(m)
        }
        logLines(m)
    }

    // ---- the entry, before a ceremony is live --------------------------

    private fun entry() {
        val peer = Kernel.peerKey()
        if (peer == null) {
            para("A ceremony is with someone you are provisioned to. None yet.")
            return
        }
        para("A meeting establishes a presence record with the other person, in person. Choose what this meeting is for — it cannot be changed once it begins.")
        button("Meet only") { start(Meet.Adopt.NONE) }
        button("Meet and adopt them under me") { start(Meet.Adopt.THEM_UNDER_ME) }
        button("Meet and be adopted under them") { start(Meet.Adopt.ME_UNDER_THEM) }
    }

    private fun start(adopt: Meet.Adopt) {
        Kernel.startMeet(adopt)?.bind(sink) ?: para("could not begin: unprovisioned")
        redraw()
    }

    // ---- D1 intent -----------------------------------------------------

    private fun intent(m: Meet) {
        para("A short code goes screen-to-screen with ${m.counterpartyName} — each phone's contribution, then the ceremony id both compute and check. That handshake is what a person verifies by looking.")
        para("The intent and the records behind it then cross on a bearer the shell picks (wire-format §14.3: a direct radio first, a network fetch last). The encoding is fixed; the carriage is not wired in this build.")
        walkthrough("the counterparty's intent arrived") { m.intentExchanged() }
    }

    // ---- D1.5 the brief: everything front-loaded -----------------------

    private fun brief(m: Meet) {
        para("From here the phone faces ${m.counterpartyName} and takes nothing from you until the capture is done. Read this now.")
        heading("What the record will hold")
        para("• Your identity, the time, and that you met — durable and readable by those you show it to.")
        para("• The fields you disclose (chosen here, not later). What you withhold is visible as withheld, never as absent.")
        para("• No image. Captures stay on each device, sealed under the other's key, for a stated retention and then deleted.")
        heading("What may be weak in this meeting")
        para("• Nominated witnesses: none, until your horizon can offer them. The record carries that it had none.")
        para("• Verifiers and proximity are weighed, not required; a thin meeting is honest, not malformed.")
        button("I have read this — begin") { m.acknowledgeBrief() }
    }

    // ---- D2–D4 hands-off ------------------------------------------------

    private fun handsOff(m: Meet, what: String, needs: String, advance: () -> Unit) {
        para("$what is running. The phone is facing ${m.counterpartyName}; there is nothing to do here.")
        para("This step needs $needs, which this device does not provide, so it cannot truly complete here.")
        walkthrough("$what finished") { advance() }
    }

    // ---- D5 verifiers --------------------------------------------------

    private fun verifiers(m: Meet) {
        para("Your device picks ${m.counterpartyName}'s verifiers from the records they handed over, preferring people you have met. The choice is computed, not offered: there is nothing here to pick.")
        if (!m.selectionRun()) {
            // the selection is asked for once, when the flow enters this
            // step, and never from inside a draw
            para("Selecting…")
            return
        }
        val chosen = m.chosen()
        heading("Selected")
        if (chosen.isEmpty()) {
            para("• None required. ${m.counterpartyName} handed over no records, so there is no pool to draw from and none is owed — a meeting with fewer verifiers is thinner, not malformed.")
        } else {
            chosen.forEach { c ->
                val answer = when (c.verdict) {
                    null -> "awaiting an answer"
                    Meet.Verdict.MATCH -> "answered: a match"
                    Meet.Verdict.NO_MATCH -> "answered: no match"
                    Meet.Verdict.INCONCLUSIVE -> "answered: inconclusive"
                    Meet.Verdict.UNAVAILABLE -> "unavailable — silence counts for nothing either way"
                }
                para("• ${c.key.take(16)}… — ${basisWords(c.basis)}; $answer")
            }
            para("Each query waits on the counterparty's consent, which crosses the local bearer this build does not yet carry. Once consented, the kernel carries it to its verifier over the network and the answer lands above.")
        }
        val mine = m.queriesAboutMe()
        if (mine.isNotEmpty()) {
            heading("Queries about you")
            para("${m.counterpartyName} asked these verifiers about you. Your device consented on each — consent is bound to the ceremony you are standing in, so it is given without stopping to ask, and you are told instead.")
            mine.forEach { para("• $it…") }
        }
        button("Continue to review") { m.verifiersDone() }
    }

    /** `wire-format.md` §5.5's basis, in words, and each says whose claim
     *  it is: nobody audits a selector's tier. */
    private fun basisWords(b: Meet.Basis): String = when (b) {
        Meet.Basis.MET -> "you have met them"
        Meet.Basis.IN_HORIZON -> "in your horizon"
        Meet.Basis.REACHABLE -> "one edge beyond it"
        Meet.Basis.DISCRETIONARY -> "a stranger, taken at your discretion"
    }

    // ---- D6 review and sign --------------------------------------------

    private fun review(m: Meet) {
        para("You review ${m.counterpartyName}'s selection of your verifiers before signing — a party who signs unseen may be vouching for strangers.")
        para("Warnings would appear here: missing familiar verifiers, witness imbalance, weak proximity. A degraded meeting is shown as degraded, never as broken.")
        para("Signing needs the artifacts the steps above would have produced; this build has none, so it cannot finalize a real record.")
        walkthrough("signed (no real record)") { m.signed("(walkthrough, no record)") }
    }

    // ---- D7 after ------------------------------------------------------

    private fun done(m: Meet) {
        para("The record: ${m.recordTxid()}")
        if (m.adopt != Meet.Adopt.NONE) {
            para("The adoption you chose at the start would now be proposed and taken.")
        }
        button("Done") { Kernel.stopMeet("done"); finish() }
    }

    private fun stopped(m: Meet) {
        para("Stopped: ${m.stopReason()}")
        button("Back") { Kernel.stopMeet("dismissed"); finish() }
    }

    // ---- view helpers --------------------------------------------------

    private fun title(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 24f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 0, 0, 24)
        },
    )

    private fun heading(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 16f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 20, 0, 8)
        },
    )

    private fun para(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 15f
            setPadding(0, 8, 0, 8)
        },
    )

    private fun button(label: String, onClick: () -> Unit) = body.addView(
        Button(this).apply {
            text = label
            setPadding(0, 16, 0, 16)
            setOnClickListener { onClick() }
        },
    )

    /** A scaffold control, not the ceremony: it stands in for a signal a
     *  real device would raise, and is dim and labelled so. */
    private fun walkthrough(label: String, onClick: () -> Unit) = body.addView(
        TextView(this).apply {
            text = "▸ walkthrough: $label"
            textSize = 13f
            setTextColor(Color.GRAY)
            gravity = Gravity.END
            setPadding(0, 28, 0, 12)
            isClickable = true
            setOnClickListener { onClick() }
        },
    )

    private fun logLines(m: Meet) {
        m.log().forEach { line ->
            body.addView(
                TextView(this).apply {
                    text = line
                    textSize = 12f
                    typeface = Typeface.MONOSPACE
                    setTextColor(Color.GRAY)
                    setPadding(0, 2, 0, 2)
                },
            )
        }
        scroll.post { scroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }
}
