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
}
