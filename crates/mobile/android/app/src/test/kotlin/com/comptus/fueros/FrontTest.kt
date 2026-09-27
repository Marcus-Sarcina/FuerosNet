package com.comptus.fueros

import kotlin.concurrent.thread
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The screen contract, held on the JVM: what S05 was about, as tests
 * rather than an emulator observation.  A screen binds and is given the
 * state so far; a screen that has gone is a null sink; a replacement is
 * never silenced by the corpse it replaced; and the replay is exact even
 * against a kernel speaking at that moment.
 */
class FrontTest {

    private class Screen : Front.Ui {
        val heard = mutableListOf<String>()
        var status = ""

        // the kernel calls under its own lock; this side keeps its own,
        // as a real screen posts to its own thread
        override fun status(line: String) {
            synchronized(heard) { status = line }
        }

        override fun say(line: String) {
            synchronized(heard) { heard.add(line) }
        }

        fun lines(): List<String> = synchronized(heard) { heard.toList() }
    }

    @Test
    fun a_bound_screen_gets_the_state_so_far_then_everything_new() {
        val front = Front()
        front.say("one")
        front.say("two")
        val screen = Screen()
        front.bind(screen)
        assertEquals(listOf("one", "two"), screen.lines())
        front.say("three")
        assertEquals(listOf("one", "two", "three"), screen.lines())
    }

    @Test
    fun what_a_gone_screen_missed_is_replayed_to_its_replacement() {
        val front = Front()
        val first = Screen()
        front.bind(first)
        front.say("seen by the first")
        front.unbind(first)
        front.say("said to nobody")
        val second = Screen()
        front.bind(second)
        assertEquals(
            listOf("seen by the first", "said to nobody"),
            second.lines(),
        )
        assertEquals(listOf("seen by the first"), first.lines())
    }

    @Test
    fun a_stale_unbind_does_not_silence_the_replacement() {
        val front = Front()
        val old = Screen()
        val replacement = Screen()
        front.bind(old)
        front.bind(replacement)
        // the recreated Activity's predecessor stops late, as it does on a
        // configuration change; its unbind names a screen no longer bound
        front.unbind(old)
        front.say("for the replacement")
        assertTrue("for the replacement" in replacement.lines())
    }

    @Test
    fun the_transcript_is_bounded_and_the_oldest_lines_go() {
        val front = Front()
        repeat(520) { front.say("line $it") }
        val screen = Screen()
        front.bind(screen)
        val lines = screen.lines()
        assertEquals(500, lines.size)
        assertEquals("line 20", lines.first())
        assertEquals("line 519", lines.last())
    }

    @Test
    fun the_latest_status_is_replayed_on_bind() {
        val front = Front()
        front.status("detached")
        front.status("attached, primary")
        val screen = Screen()
        front.bind(screen)
        assertEquals("attached, primary", synchronized(screen.heard) { screen.status })
    }

    @Test
    fun a_line_said_during_bind_arrives_exactly_once_and_in_order() {
        // the documented guarantee: a line the kernel is saying at the
        // moment of a bind arrives in the replay or after it, never in
        // both places or out of order
        repeat(20) {
            val front = Front()
            val said = (0 until 400).map { "line $it" }
            val screen = Screen()
            val speaker = thread {
                said.forEach { front.say(it) }
            }
            front.bind(screen)
            speaker.join()
            // whatever the interleaving, the screen ends with every line,
            // each exactly once, in the order they were said
            assertEquals(said, screen.lines())
        }
    }
}
