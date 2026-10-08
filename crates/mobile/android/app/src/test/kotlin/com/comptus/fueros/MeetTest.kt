package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The Meet flow's gating, held on the JVM — the rules the screens sheet
 * ruled, as tests. The kernel calls and the hardware are not here; the
 * sequence and its gates are, and they are what the rulings are about.
 */
class MeetTest {

    private fun meet(
        adopt: Meet.Adopt = Meet.Adopt.NONE,
        backup: Boolean = false,
        role: Meet.Role = Meet.Role.INITIATOR,
    ) = Meet("aa", "carol", Meet.Kind(adopt, backup), role)

    @Test
    fun the_flow_begins_at_intent() {
        assertEquals(Meet.Step.INTENT, meet().step())
    }

    @Test
    fun adoption_is_fixed_with_the_intent_not_chosen_later() {
        // the choice is a constructor argument: there is no method to set it
        // after the flow has begun
        assertEquals(Meet.Adopt.THEM_UNDER_ME, meet(Meet.Adopt.THEM_UNDER_ME).adopt)
        assertEquals(Meet.Adopt.NONE, meet().adopt)
        assertTrue(meet(backup = true).kind.askBackup)
    }

    @Test
    fun the_hands_off_phase_is_reached_only_through_the_brief() {
        val m = meet()
        m.crossBootstrap()
        assertEquals(Meet.Step.BRIEF, m.step())
        m.accept()
        // optical, proximity, capture: the device faces away
        assertEquals(Meet.Step.OPTICAL, m.step())
        m.intentSent(); m.intentReceived()
        assertEquals(Meet.Step.PROXIMITY, m.step())
        m.proximityDone()
        assertEquals(Meet.Step.CAPTURE, m.step())
        m.captureDone()
        // capture hands back to the user for verifier selection
        assertEquals(Meet.Step.VERIFIERS, m.step())
    }

    @Test
    fun the_brief_cannot_be_skipped() {
        val m = meet()
        m.crossBootstrap() // at BRIEF
        // there is no way to reach the hands-off phase but through the brief
        m.intentSent(); m.intentReceived()
        assertEquals(Meet.Step.BRIEF, m.step())
        m.accept()
        assertEquals(Meet.Step.OPTICAL, m.step())
    }

    @Test
    fun a_record_finalizes_only_out_of_the_conversation() {
        val m = meet()
        assertThrows(IllegalStateException::class.java) { m.finalized("tx") }
        m.crossBootstrap(); m.accept()
        m.intentSent(); m.intentReceived(); m.proximityDone(); m.captureDone()
        val c = FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 0))
        m.converse(c)
        assertEquals(Meet.Step.REVIEW, m.step())
        m.finalized("tx01")
        assertEquals(Meet.Step.DONE, m.step())
        assertEquals("tx01", m.recordTxid())
    }

    @Test
    fun a_stop_is_terminal_and_carries_its_reason() {
        val m = meet()
        m.crossBootstrap(); m.accept()
        m.stop("the camera was denied")
        assertEquals(Meet.Step.STOPPED, m.step())
        assertEquals("the camera was denied", m.stopReason())
        // a stop does not un-stop, and a later signal is ignored
        m.stop("something else")
        assertEquals("the camera was denied", m.stopReason())
    }

    @Test
    fun a_bound_screen_renders_on_every_step() {
        val m = meet()
        var renders = 0
        m.bind(object : Meet.Ui { override fun render() { renders++ } })
        val atBind = renders
        m.crossBootstrap()
        m.accept()
        assertEquals(atBind + 2, renders)
    }

    // ---- the verifier step -------------------------------------------

    /** The flow, up to the point the verifier step is live. */
    private fun atVerifiers(): Meet = meet().apply {
        crossBootstrap(); accept(); intentSent(); intentReceived(); proximityDone(); captureDone()
    }

    @Test
    fun the_verifier_step_distinguishes_none_required_from_not_yet_asked() {
        val m = atVerifiers()
        assertEquals(Meet.Step.VERIFIERS, m.step())
        // before the selection runs, an empty list means nothing was asked
        assertFalse(m.selectionRun())
        assertTrue(m.chosen().isEmpty())
        // an empty selection is a real answer: a thin pool obliges nobody
        m.selected(listOf())
        assertTrue(m.selectionRun())
        assertTrue(m.chosen().isEmpty())
    }

    @Test
    fun a_selected_verifier_carries_the_basis_claimed_for_it() {
        val m = atVerifiers()
        m.selected(
            listOf(
                Meet.Chosen("bb", Meet.Basis.MET),
                Meet.Chosen("cc", Meet.Basis.DISCRETIONARY),
            ),
        )
        assertEquals(listOf(Meet.Basis.MET, Meet.Basis.DISCRETIONARY), m.chosen().map { it.basis })
        // and no verdict until one answers
        assertTrue(m.chosen().all { it.verdict == null })
    }

    @Test
    fun a_response_lands_against_its_own_verifier_and_others_are_ignored() {
        val m = atVerifiers()
        m.selected(listOf(Meet.Chosen("bb", Meet.Basis.MET), Meet.Chosen("cc", Meet.Basis.MET)))
        m.responded("cc", Meet.Verdict.MATCH)
        assertEquals(null, m.chosen()[0].verdict)
        assertEquals(Meet.Verdict.MATCH, m.chosen()[1].verdict)
        // a response naming a verifier this device never selected is not
        // this ceremony's business
        m.responded("zz", Meet.Verdict.NO_MATCH)
        assertEquals(listOf(null, Meet.Verdict.MATCH), m.chosen().map { it.verdict })
    }

    @Test
    fun a_query_about_me_is_surfaced_rather_than_asked() {
        val m = atVerifiers()
        // the subject is told (design 7.4.2); nothing here asks anything,
        // so there is no answer to give and none is recorded
        m.querySurfaced("bb")
        m.querySurfaced("cc")
        assertEquals(listOf("bb", "cc"), m.queriesAboutMe())
    }

    @Test
    fun the_kernel_hands_the_verifier_step_on_to_review() {
        val m = atVerifiers()
        m.selected(listOf())
        // no tap moves to review: the body going out does
        m.converse(FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 0)))
        assertEquals(Meet.Step.REVIEW, m.step())
    }

    @Test
    fun a_bound_screen_renders_when_the_selection_and_its_answers_land() {
        val m = atVerifiers()
        var renders = 0
        m.bind(object : Meet.Ui { override fun render() { renders++ } })
        val atBind = renders
        m.selected(listOf(Meet.Chosen("bb", Meet.Basis.MET)))
        m.responded("bb", Meet.Verdict.MATCH)
        m.querySurfaced("cc")
        assertEquals(atBind + 3, renders)
        // a response against nobody changes nothing, so it renders nothing
        m.responded("zz", Meet.Verdict.MATCH)
        assertEquals(atBind + 3, renders)
    }
}

/**
 * A kernel that answers what it is told to: the four steps record that
 * they were called and refuse as scripted, and the progress is whatever
 * the test last set.
 */
class FakeCourier(var progress: Meet.Progress? = null) : Meet.Courier {
    val calls = mutableListOf<String>()
    var openRefusal: String? = null
    var queriesRefusal: String? = null
    var gatheredRefusal: String? = null
    /** Each propose's answer in turn; null is success. Exhausted, it succeeds. */
    val proposeAnswers = ArrayDeque<String?>()

    override fun open(): String? { calls += "open"; return openRefusal }
    override fun queries(): String? { calls += "queries"; return queriesRefusal }
    override fun gathered(): String? { calls += "gathered"; return gatheredRefusal }
    override fun propose(): String? { calls += "propose"; return proposeAnswers.removeFirstOrNull() }
    override fun progress(): Meet.Progress? = progress

    fun count(call: String) = calls.count { it == call }
}

/**
 * The conversation on the courier, as the flow drives it against a fake
 * kernel: the kernel's four steps and its progress are the whole of what
 * the flow sees, so the gating is testable to the step.
 */
class MeetConversationTest {

    private fun atVerifiers(role: Meet.Role = Meet.Role.INITIATOR): Meet =
        Meet("aa", "carol", Meet.Kind(), role).apply {
            crossBootstrap(); accept(); intentSent(); intentReceived(); proximityDone(); captureDone()
        }

    /** A meeting with its codes crossing, which is where the optical
     *  deadline applies. */
    private fun atOptical(): Meet =
        Meet("aa", "carol", Meet.Kind()).apply { crossBootstrap(); accept() }

    @Test
    fun the_conversation_opens_once_on_the_verifiers_step_and_not_before() {
        val c = FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 2))
        val early = Meet("aa", "carol", Meet.Kind())
        early.converse(c)
        assertTrue("nothing before the verifiers", c.calls.isEmpty())
        val m = atVerifiers()
        m.converse(c)
        assertEquals(listOf("open", "queries"), c.calls)
        assertTrue(m.opened())
        // a recreated screen or a second tick does not open it again
        m.converse(c)
        assertEquals(listOf("open", "queries"), c.calls)
        assertEquals(Meet.Step.VERIFIERS, m.step())
    }

    @Test
    fun the_initiator_proposes_when_no_query_is_outstanding_and_the_record_ends_it() {
        val m = atVerifiers()
        val c = FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 2))
        m.converse(c)
        m.poll(c)
        assertEquals("queries outstanding: no proposal yet", 0, c.count("propose"))
        assertEquals(0, c.count("gathered"))
        c.progress = Meet.Progress(proposer = true, queriesOutstanding = 0, attesting = listOf("w1"))
        m.poll(c)
        assertEquals(1, c.count("propose"))
        assertEquals(Meet.Step.REVIEW, m.step())
        // the kernel's word on the record is what ends the flow
        m.conversed("the record is finalised: 0a0b0c0d", Meet.Turn.Finalized("0a0b0c0d0e0f"))
        assertEquals(Meet.Step.DONE, m.step())
        assertEquals("0a0b0c0d0e0f", m.recordTxid())
    }

    /**
     * **A conversation that will never move again stops itself**
     * ([Meet.STALE_MS]). Two field runs of 2026-10-07 ended with a phone
     * polling for ever: `aimed-1` left the proposer waiting on a carriage
     * that had been thrown away, and `shutter-1` had the sender stop
     * correctly while the counterparty went on, because the thing that
     * would have told it was the thing that failed.
     */
    @Test
    fun a_conversation_that_stops_moving_is_given_up_on() {
        val m = atVerifiers(Meet.Role.RESPONDER)
        val c = FakeCourier(Meet.Progress(proposer = false, queriesOutstanding = 0))
        val t0 = 1_000_000L
        m.converse(c, t0)
        // polling with the same progress is not progress, however long
        m.poll(c, t0 + Meet.STALE_MS)
        assertEquals("still inside the allowance", Meet.Step.VERIFIERS, m.step())
        m.poll(c, t0 + Meet.STALE_MS + 1)
        assertEquals(Meet.Step.STOPPED, m.step())
        assertTrue(
            "and says why: ${m.stopReason()}",
            m.stopReason()?.contains("nothing has moved") == true,
        )
    }

    /** Anything at all moving restarts the allowance: `Progress` compares
     *  by value, so every figure the conversation exposes counts. */
    @Test
    fun progress_of_any_kind_restarts_the_allowance() {
        val m = atVerifiers(Meet.Role.RESPONDER)
        val c = FakeCourier(Meet.Progress(proposer = false, queriesOutstanding = 0))
        val t0 = 1_000_000L
        m.converse(c, t0)
        // one nominee agrees to attest, well into the allowance
        c.progress = Meet.Progress(proposer = false, queriesOutstanding = 0, attesting = listOf("w1"))
        m.poll(c, t0 + Meet.STALE_MS - 1)
        assertEquals(Meet.Step.VERIFIERS, m.step())
        // the clock runs from there, not from the start
        m.poll(c, t0 + Meet.STALE_MS + 1)
        assertEquals("the allowance began again", Meet.Step.VERIFIERS, m.step())
        m.poll(c, t0 + 2 * Meet.STALE_MS)
        assertEquals(Meet.Step.STOPPED, m.step())
    }

    /**
     * **The optical exchange has an end too.** The staleness deadline
     * covered `VERIFIERS` and `REVIEW` only, leaving the longest step of
     * the ceremony with none: on the bench one phone's app was restarted
     * mid-exchange and the other, holding 41 of 102 parts, went on reading
     * a screen that would never change again [2026-10-07].
     */
    @Test
    fun an_optical_exchange_that_stops_moving_is_given_up_on() {
        val m = atOptical()
        val t0 = 500_000L
        m.opticalMoved(t0)
        assertFalse("inside the allowance", m.opticalStale(t0 + Meet.STALE_MS))
        assertTrue("past it", m.opticalStale(t0 + Meet.STALE_MS + 1))
        // a part landing restarts it
        m.opticalMoved(t0 + Meet.STALE_MS)
        assertFalse("the allowance began again", m.opticalStale(t0 + 2 * Meet.STALE_MS))
        assertTrue(m.opticalStale(t0 + 2 * Meet.STALE_MS + 1))
    }

    /** And it only applies while the codes are crossing: a step that has
     *  moved on has its own deadline, or none. */
    @Test
    fun the_optical_deadline_is_only_the_optical_step_s() {
        val m = atVerifiers(Meet.Role.INITIATOR)
        assertFalse("not at the optical step", m.opticalStale(System.currentTimeMillis() + 10 * Meet.STALE_MS))
    }

    @Test
    fun the_responder_hands_over_once_and_reviews_when_the_body_is_shown() {
        val m = atVerifiers(Meet.Role.RESPONDER)
        val c = FakeCourier(Meet.Progress(proposer = false, queriesOutstanding = 1))
        m.converse(c)
        assertEquals(0, c.count("gathered"))
        c.progress = Meet.Progress(proposer = false, queriesOutstanding = 0)
        m.poll(c)
        m.poll(c)
        assertEquals("gathered goes once, not per tick", 1, c.count("gathered"))
        assertEquals("the responder never proposes", 0, c.count("propose"))
        assertEquals(Meet.Step.VERIFIERS, m.step())
        // the body arrives, is reviewed and signed in the kernel, and the
        // progress says so
        m.conversed("the body reviewed and signed", Meet.Turn.Other)
        c.progress = Meet.Progress(proposer = false, queriesOutstanding = 0, proposed = true)
        m.poll(c)
        assertEquals(Meet.Step.REVIEW, m.step())
        m.conversed("the record is finalised: 0a0b0c0d", Meet.Turn.Finalized("0a0b0c0d"))
        assertEquals(Meet.Step.DONE, m.step())
    }

    @Test
    fun a_proposal_refused_as_waiting_is_retried_and_the_wait_is_said_once() {
        val m = atVerifiers()
        val c = FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 0))
        c.proposeAnswers += "Waiting(\"the counterparty's responses\")"
        c.proposeAnswers += "Waiting(\"the counterparty's responses\")"
        c.proposeAnswers += null
        m.converse(c)
        assertEquals(Meet.Step.VERIFIERS, m.step())
        assertEquals("the counterparty's responses", m.waitingOn())
        m.poll(c)
        assertEquals(Meet.Step.VERIFIERS, m.step())
        assertEquals(1, m.log().count { it.contains("waiting on the counterparty's responses") })
        m.poll(c)
        assertEquals(3, c.count("propose"))
        assertEquals(Meet.Step.REVIEW, m.step())
        assertNull(m.waitingOn())
    }

    @Test
    fun a_witness_that_declined_is_shown_as_declined_and_weighed_in_the_warnings() {
        val m = atVerifiers()
        m.nominated(mine = listOf("w1"), theirs = listOf("w2", "w3"))
        val c = FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 1, attesting = listOf("w2"), declined = listOf("w1")))
        m.converse(c)
        val w = m.witnesses().associate { it.key to it }
        assertEquals(Meet.Answer.DECLINED, w["w1"]?.answer)
        assertEquals(Meet.Answer.ATTESTS, w["w2"]?.answer)
        assertNull("not yet answered", w["w3"]?.answer)
        assertTrue(w["w1"]!!.mine)
        assertFalse(w["w2"]!!.mine)
        // the pre-sign warning reads the same facts: none of mine attest
        m.selected(listOf())
        m.learned(channel = "UWB", familiar = true)
        val text = m.presign().joinToString(" ") { it.text }
        assertTrue(text.contains("one-sided"))
        assertTrue(text.contains("0 nominated by you"))
    }

    @Test
    fun no_witness_is_surfaced_to_the_person_and_the_proposal_kept_trying() {
        val m = atVerifiers()
        val c = FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 0))
        c.proposeAnswers += "NoWitness"
        c.proposeAnswers += "NoWitness"
        m.converse(c)
        assertEquals(Meet.Step.VERIFIERS, m.step())
        assertTrue(m.noWitness())
        assertNull("not a stop: the person decides", m.stopReason())
        m.poll(c)
        assertEquals(2, c.count("propose"))
        // a late nominee answers and the next attempt goes
        c.progress = Meet.Progress(proposer = true, queriesOutstanding = 0, attesting = listOf("w1"))
        m.poll(c)
        assertEquals(Meet.Step.REVIEW, m.step())
        assertFalse(m.noWitness())
    }

    @Test
    fun any_other_refusal_stops_the_meeting_with_its_code_shown() {
        val m = atVerifiers()
        val c = FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 0))
        c.proposeAnswers += "RootMismatch"
        m.converse(c)
        assertEquals(Meet.Step.STOPPED, m.step())
        assertTrue(m.stopReason()!!.contains("RootMismatch"))
        // an open or queries refusal is terminal too, with its reason
        val m2 = atVerifiers()
        val c2 = FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 0))
        c2.openRefusal = "Payload(\"no session to 1a2b3c4d\")"
        m2.converse(c2)
        assertEquals(Meet.Step.STOPPED, m2.step())
        assertTrue(m2.stopReason()!!.contains("no session"))
        assertEquals("nothing after a refused open", listOf("open"), c2.calls)
        assertFalse("never open, so never polled", m2.opened())
    }

    @Test
    fun nothing_is_polled_until_the_queries_are_out() {
        // a poll between the open and the queries would read no query
        // outstanding and move too early; the flow is not open until both
        // calls went
        val m = atVerifiers(Meet.Role.RESPONDER)
        val c = object : Meet.Courier {
            val calls = mutableListOf<String>()
            override fun open(): String? { calls += "open"; m.poll(this); return null }
            override fun queries(): String? { calls += "queries"; return null }
            override fun gathered(): String? { calls += "gathered"; return null }
            override fun propose(): String? { calls += "propose"; return null }
            override fun progress() = Meet.Progress(proposer = false, queriesOutstanding = 0)
        }
        m.converse(c)
        assertEquals(listOf("open", "queries", "gathered"), c.calls)
    }

    @Test
    fun a_refused_body_is_a_stopped_meeting_with_the_code_and_a_stray_refusal_is_a_line() {
        // the responder's kernel refused the body it was shown
        val m = atVerifiers(Meet.Role.RESPONDER)
        m.converse(FakeCourier(Meet.Progress(proposer = false, queriesOutstanding = 0)))
        m.conversed("kind 12 refused: from no party to a ceremony this client is in or witnesses", Meet.Turn.Other)
        assertEquals("a stranger's message is not this meeting's failure", Meet.Step.VERIFIERS, m.step())
        assertTrue(m.log().any { it.contains("kind 12 refused") })
        m.conversed("the body refused: NotVerified", Meet.Turn.BodyRefused(4, "NotVerified"))
        assertEquals(Meet.Step.STOPPED, m.step())
        assertTrue(m.stopReason()!!.contains("NotVerified"))
        assertTrue("the wire code is shown", m.stopReason()!!.contains("refusal 4"))
        // the proposer, told a signer refused: the record will not finalize
        val p = atVerifiers()
        p.converse(FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 0)))
        assertEquals(Meet.Step.REVIEW, p.step())
        p.conversed("1a2b3c4d refused: NotVerified", Meet.Turn.SignerRefused("1a2b3c4d5e6f7a8b", 4, "NotVerified"))
        assertEquals(Meet.Step.STOPPED, p.step())
        assertTrue(p.stopReason()!!.contains("1a2b3c4d"))
        assertTrue(p.stopReason()!!.contains("NotVerified"))
    }

    @Test
    fun a_person_may_stop_at_review_and_the_record_no_longer_moves_the_flow() {
        val m = atVerifiers()
        val c = FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 0))
        m.converse(c)
        assertEquals(Meet.Step.REVIEW, m.step())
        m.stop("you stopped at review")
        assertEquals(Meet.Step.STOPPED, m.step())
        // the kernel goes on without the screen: its word is not an error here
        m.conversed("the record is finalised: 0a0b0c0d", Meet.Turn.Finalized("0a0b0c0d"))
        m.poll(c)
        assertEquals(Meet.Step.STOPPED, m.step())
        assertNull(m.recordTxid())
        assertEquals("you stopped at review", m.stopReason())
    }

    @Test
    fun the_patience_runs_out_and_the_initiator_proposes_with_queries_unanswered() {
        val m = Meet("aa", "carol", Meet.Kind(), Meet.Role.INITIATOR, patienceMs = 1_000).apply {
            crossBootstrap(); accept(); intentSent(); intentReceived(); proximityDone(); captureDone()
        }
        m.selected(listOf(Meet.Chosen("v1", Meet.Basis.MET), Meet.Chosen("v2", Meet.Basis.MET)))
        val c = FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 2))
        m.converse(c, nowMs = 10_000)
        assertEquals(0, c.count("propose"))
        assertEquals(1_000L, m.patienceLeftMs(10_000))
        m.poll(c, nowMs = 10_900)
        assertEquals("still patient", 0, c.count("propose"))
        // an answer landing resets the wait: the other may be a moment behind
        c.progress = Meet.Progress(proposer = true, queriesOutstanding = 1)
        m.responded("v1", Meet.Verdict.MATCH, nowMs = 10_950)
        m.poll(c, nowMs = 11_000)
        assertEquals("the answer bought another second", 0, c.count("propose"))
        assertEquals(950L, m.patienceLeftMs(11_000))
        m.poll(c, nowMs = 11_950)
        assertEquals(1, c.count("propose"))
        assertEquals(Meet.Step.REVIEW, m.step())
        assertEquals(1, m.wentOnWithout())
        assertEquals(1, m.log().count { it.contains("going on with 1 queries unanswered") })
        assertTrue("the thinness is a pre-sign warning", m.presign().any { it.text.contains("had not answered") })
    }

    @Test
    fun the_person_may_go_on_without_the_unanswered_queries_and_the_responder_hands_over() {
        val m = atVerifiers(Meet.Role.RESPONDER)
        val c = FakeCourier(Meet.Progress(proposer = false, queriesOutstanding = 3))
        m.converse(c, nowMs = 0)
        m.poll(c, nowMs = 5_000)
        assertEquals(0, c.count("gathered"))
        assertNull("nothing to go on without before the flow is open", Meet("aa", "carol", Meet.Kind()).patienceLeftMs(0))
        m.goOn()
        assertEquals(0L, m.patienceLeftMs(5_000))
        m.poll(c, nowMs = 5_001)
        assertEquals(1, c.count("gathered"))
        assertEquals(3, m.wentOnWithout())
        assertTrue(m.log().any { it.contains("you said to go on") })
        // and no second note on the next tick
        m.poll(c, nowMs = 5_002)
        assertEquals(1, m.log().count { it.contains("going on with") })
    }

    @Test
    fun a_stop_from_any_live_step_tells_the_kernel_once() {
        val m = atVerifiers()
        var told = 0
        m.onStopped = { told++ }
        m.stop("you stopped at the verifiers")
        m.stop("again")
        assertEquals(1, told)
        // a finished meeting is not abandoned
        val done = atVerifiers()
        done.onStopped = { told++ }
        done.converse(FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 0)))
        done.finalized("tx")
        done.stop("late")
        assertEquals(1, told)
    }

    @Test
    fun the_brief_states_the_retention_this_device_declares() {
        val m = Meet("aa", "carol", Meet.Kind(), retentionYears = 3).apply { crossBootstrap() }
        assertTrue(m.brief().any { it.text.contains("this phone declares, 3 years,") })
        val one = Meet("aa", "carol", Meet.Kind(), retentionYears = 1).apply { crossBootstrap() }
        assertTrue(one.brief().any { it.text.contains("this phone declares, 1 year,") })
    }

    @Test
    fun the_codes_name_the_counterparty_and_a_second_name_is_refused() {
        val m = Meet(kind = Meet.Kind())
        assertNull(m.counterpartyKey)
        assertEquals("the other person", m.counterpartyName)
        assertNull("the first code names them", m.counterparty("ab12cd34ef"))
        assertEquals("ab12cd34ef", m.counterpartyKey)
        assertEquals("ab12cd34", m.counterpartyName)
        assertNull("the same party again is fine", m.counterparty("ab12cd34ef"))
        val refused = m.counterparty("ffffffff00")
        assertTrue(refused != null && refused.contains("someone else's"))
        assertEquals("ab12cd34ef", m.counterpartyKey)
        // a name given beforehand stays the name
        assertEquals("carol", Meet("aa", "carol", Meet.Kind()).counterpartyName)
    }

    @Test
    fun accepting_the_brief_begins_the_ceremony_once_and_the_first_code_waits_for_it() {
        val m = Meet(kind = Meet.Kind()).apply { crossBootstrap() }
        var begun = 0
        m.onAccepted = { begun++ }
        assertFalse(m.begun())
        m.accept()
        assertEquals(1, begun)
        assertEquals(Meet.Step.OPTICAL, m.step())
        assertFalse("the kernel has not said so yet", m.begun())
        m.begunCeremony()
        assertTrue(m.begun())
    }

    @Test
    fun the_optical_step_ends_only_when_both_intents_have_crossed() {
        val m = Meet(kind = Meet.Kind()).apply { crossBootstrap(); accept() }
        assertEquals(Meet.Step.OPTICAL, m.step())
        m.intentReceived()
        assertEquals("theirs alone is not enough", Meet.Step.OPTICAL, m.step())
        m.intentSent()
        assertEquals(Meet.Step.PROXIMITY, m.step())
        // and the other order
        val n = Meet(kind = Meet.Kind()).apply { crossBootstrap(); accept() }
        n.intentSent()
        assertEquals(Meet.Step.OPTICAL, n.step())
        n.intentReceived()
        assertEquals(Meet.Step.PROXIMITY, n.step())
    }

    @Test
    fun every_kernel_move_renders_the_bound_screen() {
        val m = atVerifiers()
        var renders = 0
        m.bind(object : Meet.Ui { override fun render() { renders++ } })
        val atBind = renders
        val c = FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 0))
        m.converse(c)
        assertTrue("the notes, the progress and the step each render", renders > atBind)
        val atReview = renders
        m.finalized("tx")
        assertEquals(atReview + 1, renders)
    }
}

/**
 * The screens' own obligations: the rows the Meet flow answers to, as
 * tests over what the model says each screen must show.
 *
 * **A warning is a fact about this ceremony, not a template**, so these
 * assert both that an earned item appears and that an unearned one does
 * not — a screen that reassures where it has not earned the right is the
 * failure these rows exist against.
 */
class MeetRowsTest {

    private fun meet(
        adopt: Meet.Adopt = Meet.Adopt.NONE,
        backup: Boolean = false,
        role: Meet.Role = Meet.Role.INITIATOR,
    ) = Meet("aa", "carol", Meet.Kind(adopt, backup), role)

    private fun rows(items: List<Meet.Item>) = items.map { it.row }.distinct().sorted()

    // ---- D1: two halves and two cameras -------------------------------

    @Test
    fun d1_completes_on_the_bootstrap_crossing_and_not_on_an_intent() {
        val m = meet()
        assertFalse(m.bootstrapCrossed())
        m.crossBootstrap()
        assertTrue(m.bootstrapCrossed())
        assertEquals(Meet.Step.BRIEF, m.step())
    }

    @Test
    fun the_responder_chose_nothing_and_the_brief_is_where_they_may_refuse() {
        // the initiator chose the kind at D1a before the responder saw
        // anything, so the responder's own Kind is empty until the
        // bootstrap tells it, and the brief is their first refusal
        val m = meet(role = Meet.Role.RESPONDER)
        assertEquals(Meet.Adopt.NONE, m.adopt)
        m.crossBootstrap()
        assertEquals(Meet.Step.BRIEF, m.step())
        m.refuse()
        assertEquals(Meet.Step.STOPPED, m.step())
        assertEquals("you refused the meeting", m.stopReason())
    }

    // ---- D1.5: UX-001, UX-002, PRD-02 ---------------------------------

    @Test
    fun the_brief_tells_both_people_what_becomes_durable_and_who_reads_it() {
        val items = meet().brief()
        assertTrue("UX-001 is answered", rows(items).contains("UX-001"))
        assertTrue("UX-002 is answered", rows(items).contains("UX-002"))
        val text = items.joinToString(" ") { it.text }
        assertTrue("what becomes durable", text.contains("durable"))
        assertTrue("who may read it", text.contains("readable by anyone"))
        assertTrue("no image goes in", text.contains("No image"))
        assertTrue("the retention is stated", text.contains("retention"))
        assertTrue("withheld is not absent", text.contains("withheld, never as absent"))
        // and that the device stops taking input, which is the thing the
        // front-loading rule exists to make true
        assertTrue(text.contains("takes\nnothing from you") || text.contains("takes nothing from you"))
    }

    @Test
    fun a_meeting_that_asks_for_no_adoption_carries_no_authority_warning() {
        // PRD-02 is about an authority change, and a regular meeting is not
        // one: the screen must not warn about a thing that is not happening
        assertFalse(rows(meet().brief()).contains("PRD-02"))
        assertFalse(meet().brief().joinToString(" ") { it.text }.contains("patron"))
    }

    @Test
    fun an_adoption_names_the_authority_it_moves_and_which_way() {
        val under = meet(Meet.Adopt.ME_UNDER_THEM).brief()
        assertTrue(rows(under).contains("PRD-02"))
        val u = under.joinToString(" ") { it.text }
        assertTrue("names them as patron", u.contains("carol becomes your patron"))
        assertTrue("names the upward-reaching resources", u.contains("reaches upward"))

        val over = meet(Meet.Adopt.THEM_UNDER_ME).brief()
        assertTrue(rows(over).contains("PRD-02"))
        val o = over.joinToString(" ") { it.text }
        assertTrue("names the cascade downward", o.contains("outage cascades"))
        assertTrue("whose risk it is", o.contains("their risk"))
    }

    @Test
    fun a_backup_request_says_what_leaves_and_what_does_not() {
        val text = meet(backup = true).brief().joinToString(" ") { it.text }
        assertTrue(text.contains("encrypted backup"))
        assertTrue("biometrics and secrets stay", text.contains("never leave this device"))
        assertFalse(meet().brief().joinToString(" ") { it.text }.contains("encrypted backup"))
    }

    // ---- D6: UX-003 ---------------------------------------------------

    private fun atReview(m: Meet): Meet {
        m.crossBootstrap(); m.accept(); m.intentSent(); m.intentReceived(); m.proximityDone(); m.captureDone()
        m.converse(FakeCourier(Meet.Progress(proposer = true, queriesOutstanding = 0)))
        check(m.step() == Meet.Step.REVIEW)
        return m
    }

    @Test
    fun the_pre_sign_warnings_name_each_weakness_and_only_the_real_ones() {
        val m = atReview(meet())
        m.selected(
            listOf(
                Meet.Chosen("c1", Meet.Basis.DISCRETIONARY, Meet.Verdict.MATCH),
                Meet.Chosen("c2", Meet.Basis.DISCRETIONARY, Meet.Verdict.UNAVAILABLE),
            ),
        )
        m.learned(mine = 2, theirs = 0, channel = "NFC", familiar = false)
        val w = m.presign()
        assertEquals(listOf("UX-003"), rows(w))
        val text = w.joinToString(" ") { it.text }
        assertTrue("missing familiar verifiers", text.contains("someone you have met"))
        assertTrue("unavailable evidence", text.contains("answered nothing"))
        assertTrue("witness imbalance", text.contains("one-sided"))
        assertTrue("weak proximity", text.contains("not UWB"))
        // and NEVER as malformed: that is UX-003's whole point
        assertFalse(text.contains("malformed"))
        assertFalse(text.contains("invalid"))
    }

    @Test
    fun a_sound_meeting_is_not_warned_about() {
        val m = atReview(meet())
        m.selected(listOf(Meet.Chosen("c1", Meet.Basis.MET, Meet.Verdict.MATCH)))
        m.learned(mine = 2, theirs = 2, channel = "UWB", familiar = true)
        assertTrue("nothing to flag", m.presign().isEmpty())
    }

    @Test
    fun no_witnesses_is_the_formation_case_and_said_so() {
        val m = atReview(meet())
        m.selected(listOf())
        m.learned(mine = 0, theirs = 0, channel = "UWB", familiar = true)
        val text = m.presign().joinToString(" ") { it.text }
        assertTrue(text.contains("formation record"))
        assertTrue("real, and visibly uncorroborated", text.contains("uncorroborated"))
        // an empty selection is a real answer, not a failure
        assertTrue(text.contains("no prior\nrecords") || text.contains("no prior records"))
    }

    @Test
    fun no_channel_passing_is_named_without_calling_the_meeting_broken() {
        val m = atReview(meet())
        m.selected(listOf())
        m.learned(mine = 1, theirs = 1, channel = null, familiar = true)
        val text = m.presign().joinToString(" ") { it.text }
        assertTrue(text.contains("No proximity channel passed"))
        assertFalse(text.contains("malformed"))
    }

    // ---- D7: PRD-05 ---------------------------------------------------

    @Test
    fun two_people_both_offering_to_be_patron_is_not_a_protocol_failure() {
        val m = meet(Meet.Adopt.THEM_UNDER_ME)
        assertFalse("nothing opposed until the other's is known", m.opposedAdoptions())
        m.theirAdopt(Meet.Adopt.THEM_UNDER_ME)
        assertTrue(m.opposedAdoptions())
        // the flow is not stopped and nothing is reported broken
        assertEquals(Meet.Step.INTENT, m.step())
        assertNull(m.stopReason())
        assertNull(m.direction())
        m.chooseDirection(Meet.Adopt.ME_UNDER_THEM)
        assertEquals(Meet.Adopt.ME_UNDER_THEM, m.direction())
    }

    @Test
    fun agreeing_directions_are_not_an_opposition_at_all() {
        val m = meet(Meet.Adopt.THEM_UNDER_ME)
        // they chose to be the client, which agrees with this side
        m.theirAdopt(Meet.Adopt.ME_UNDER_THEM)
        assertFalse(m.opposedAdoptions())
        assertThrows(IllegalStateException::class.java) {
            m.chooseDirection(Meet.Adopt.NONE)
        }
    }

    @Test
    fun leaving_the_authority_question_alone_is_one_of_the_answers() {
        val m = meet(Meet.Adopt.ME_UNDER_THEM)
        m.theirAdopt(Meet.Adopt.ME_UNDER_THEM)
        assertTrue(m.opposedAdoptions())
        m.chooseDirection(Meet.Adopt.NONE)
        assertEquals(Meet.Adopt.NONE, m.direction())
    }
}
