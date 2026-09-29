package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The Meet flow's gating, held on the JVM — the rules the screens sheet
 * ruled, as tests. The kernel calls and the hardware are not here; the
 * sequence and its gates are, and they are what the rulings are about.
 */
class MeetTest {

    private fun meet(adopt: Meet.Adopt = Meet.Adopt.NONE) =
        Meet("aa", "carol", adopt)

    @Test
    fun the_flow_begins_at_intent_and_takes_input_there() {
        val m = meet()
        assertEquals(Meet.Step.INTENT, m.step())
        assertTrue(m.acceptsInput())
        assertFalse(m.handsOff())
    }

    @Test
    fun adoption_is_fixed_with_the_intent_not_chosen_later() {
        // the choice is a constructor argument: there is no method to set it
        // after the flow has begun
        assertEquals(Meet.Adopt.THEM_UNDER_ME, meet(Meet.Adopt.THEM_UNDER_ME).adopt)
        assertEquals(Meet.Adopt.NONE, meet().adopt)
    }

    @Test
    fun the_hands_off_phase_takes_no_input_and_is_reached_only_through_the_brief() {
        val m = meet()
        m.intentExchanged()
        assertEquals(Meet.Step.BRIEF, m.step())
        assertTrue("the brief still takes input", m.acceptsInput())
        m.acknowledgeBrief()
        // optical, proximity, capture: the device faces away
        for (s in listOf(Meet.Step.OPTICAL, Meet.Step.PROXIMITY, Meet.Step.CAPTURE)) {
            assertEquals(s, m.step())
            assertTrue(m.handsOff())
            assertFalse("no input while the device faces the counterparty", m.acceptsInput())
            when (s) {
                Meet.Step.OPTICAL -> m.opticalDone()
                Meet.Step.PROXIMITY -> m.proximityDone()
                Meet.Step.CAPTURE -> m.captureDone()
                else -> {}
            }
        }
        // capture hands back to the user for verifier selection
        assertEquals(Meet.Step.VERIFIERS, m.step())
        assertTrue(m.acceptsInput())
    }

    @Test
    fun the_brief_cannot_be_skipped() {
        val m = meet()
        m.intentExchanged() // at BRIEF
        // there is no way to reach the hands-off phase but through the brief
        assertThrows(IllegalStateException::class.java) { m.opticalDone() }
        m.acknowledgeBrief()
        assertEquals(Meet.Step.OPTICAL, m.step())
    }

    @Test
    fun a_record_is_signed_only_from_review() {
        val m = meet()
        assertThrows(IllegalStateException::class.java) { m.signed("tx") }
        m.intentExchanged(); m.acknowledgeBrief()
        m.opticalDone(); m.proximityDone(); m.captureDone()
        m.verifiersDone()
        assertEquals(Meet.Step.REVIEW, m.step())
        m.signed("tx01")
        assertEquals(Meet.Step.DONE, m.step())
        assertEquals("tx01", m.recordTxid())
    }

    @Test
    fun a_stop_is_terminal_and_carries_its_reason() {
        val m = meet()
        m.intentExchanged(); m.acknowledgeBrief()
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
        m.intentExchanged()
        m.acknowledgeBrief()
        assertEquals(atBind + 2, renders)
    }

    // ---- the verifier step -------------------------------------------

    /** The flow, up to the point the verifier step is live. */
    private fun atVerifiers(): Meet = meet().apply {
        intentExchanged(); acknowledgeBrief(); opticalDone(); proximityDone(); captureDone()
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
    fun the_verifier_step_still_takes_input_and_hands_on_to_review() {
        val m = atVerifiers()
        assertTrue(m.acceptsInput())
        assertFalse(m.handsOff())
        m.selected(listOf())
        m.verifiersDone()
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
