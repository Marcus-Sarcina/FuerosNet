package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **PRD-17's retry and its words** (`light-client-requirements.md` §1.1,
 * design §7.1), now that [Engagement] holds the decision and [Kernel] only
 * asks how many were reached.
 *
 * The clause about the trace each nominee receives is held by
 * `OpticalExchangeTest`'s
 * `a_whole_exchange_traces_every_part_it_moved_and_no_content`, and the
 * clause about engaging *before the code is shown* is the order [Kernel]
 * calls things in rather than a decision made here.
 */
class EngagementTest {

    // acceptance: PRD-17
    @Test
    fun a_nominee_not_yet_reached_is_waited_for_and_not_written_off() {
        // **the retry is the point**: a nominee whose session material is
        // still being fetched is absent for a moment, not unavailable
        for (attempt in 0 until Engagement.TRIES - 1) {
            assertEquals(
                "attempt $attempt gave up on a nominee still being fetched",
                Engagement.Next.Again,
                Engagement.next(engaged = 1u, nominated = 2u, attempt = attempt),
            )
        }
    }

    // acceptance: PRD-17
    @Test
    fun the_person_is_told_how_many_of_the_nominated_are_watching() {
        val said = Engagement.next(engaged = 2u, nominated = 2u, attempt = 0)
        assertTrue("every nominee reached ends it with a sentence", said is Engagement.Next.Say)
        val note = (said as Engagement.Next.Say).note
        assertTrue("it says how many: $note", note.contains("2 of 2"))
        assertTrue("and that they are watching: $note", note.contains("watching"))
    }

    // acceptance: PRD-17
    @Test
    fun the_last_attempt_says_how_many_were_reached_and_that_the_rest_will_not_appear() {
        val said = Engagement.next(
            engaged = 1u,
            nominated = 3u,
            attempt = Engagement.TRIES - 1,
        )
        assertTrue("the last attempt ends it", said is Engagement.Next.Say)
        val note = (said as Engagement.Next.Say).note
        assertTrue("it says how many were reached: $note", note.contains("1 of 3"))
        assertTrue("and that the rest will not appear: $note", note.contains("not appear"))
        // **as a fact about reachability and not a fault of theirs**: the
        // words say they were not reachable, and nothing says they refused
        assertTrue("it reads as a refusal by them: $note", !note.contains("declined"))
        assertTrue("it reads as a refusal by them: $note", !note.contains("refused"))
    }

    // acceptance: PRD-17
    @Test
    fun a_meeting_with_nobody_nominated_says_nothing_about_witnesses_here() {
        // nothing was nominated, so there is nothing to report engaging:
        // the witnessless record is said at the end and not twice
        assertEquals(
            Engagement.Next.Stop,
            Engagement.next(engaged = 0u, nominated = 0u, attempt = 0),
        )
    }

    // acceptance: PRD-17
    @Test
    fun more_reached_than_nominated_ends_it_rather_than_waiting_forever() {
        // a count this shell did not expect must not become a loop that
        // never ends: the set closes when the ceremony-id fixes, and this
        // decision is reached once per attempt
        val said = Engagement.next(engaged = 3u, nominated = 2u, attempt = 0)
        assertTrue(said is Engagement.Next.Say)
    }
}
