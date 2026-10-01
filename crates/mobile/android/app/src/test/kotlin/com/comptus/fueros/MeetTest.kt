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
        assertTrue(meet(backup = true).kind.askBackup)
    }

    @Test
    fun the_hands_off_phase_takes_no_input_and_is_reached_only_through_the_brief() {
        val m = meet()
        m.crossBootstrap()
        assertEquals(Meet.Step.BRIEF, m.step())
        assertTrue("the brief still takes input", m.acceptsInput())
        m.accept()
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
        m.crossBootstrap() // at BRIEF
        // there is no way to reach the hands-off phase but through the brief
        assertThrows(IllegalStateException::class.java) { m.opticalDone() }
        m.accept()
        assertEquals(Meet.Step.OPTICAL, m.step())
    }

    @Test
    fun a_record_is_signed_only_from_review() {
        val m = meet()
        assertThrows(IllegalStateException::class.java) { m.signed("tx") }
        m.crossBootstrap(); m.accept()
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
        crossBootstrap(); accept(); opticalDone(); proximityDone(); captureDone()
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
        m.crossBootstrap(); m.accept(); m.opticalDone(); m.proximityDone(); m.captureDone()
        m.verifiersDone()
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
