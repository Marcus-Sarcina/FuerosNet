package com.comptus.fueros

import kotlin.concurrent.thread
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The front's state model and its S05 lifecycle, held on the JVM. A screen
 * binds and re-reads on every change; a gone screen is a null sink; a stale
 * unbind does not silence a replacement; and the conversation model keeps
 * attribution and an honest delivery state — never delivered or read.
 */
class FrontTest {
    /** The identity every thread here belongs to; a second one is
     *  [two_identities_with_one_peer_are_two_threads]'s. */
    private val ME = "11".repeat(32)


    /** A screen that counts renders and reads the state each time, as a
     *  real one does on its own thread. */
    private class Screen(val front: Front) : Front.Ui {
        var renders = 0
        var lastStatus = ""
        var lastThreads: List<Front.Thread> = emptyList()

        override fun render() {
            synchronized(this) {
                renders++
                lastStatus = front.status()
                lastThreads = front.threads()
            }
        }

        fun renders() = synchronized(this) { renders }
    }

    // ---- lifecycle (S05 as tests) --------------------------------------

    @Test
    fun a_bound_screen_renders_once_against_the_state_so_far() {
        val front = Front()
        front.setStatus("attached, primary")
        front.peerKnown(ME, "aa", "carol")
        val screen = Screen(front)
        front.bind(screen)
        assertEquals(1, screen.renders())
        assertEquals("attached, primary", screen.lastStatus)
        assertEquals(listOf("carol"), screen.lastThreads.map { it.peerName })
    }

    @Test
    fun a_change_renders_the_bound_screen() {
        val front = Front()
        val screen = Screen(front)
        front.bind(screen)
        val before = screen.renders()
        front.incoming(ME, "aa", "carol", "hello")
        assertEquals(before + 1, screen.renders())
        assertEquals("hello", screen.lastThreads.single().messages.single().text)
    }

    @Test
    fun a_gone_screen_is_a_null_sink_and_the_next_one_sees_the_state() {
        val front = Front()
        val first = Screen(front)
        front.bind(first)
        front.incoming(ME, "aa", "carol", "while first was bound")
        front.unbind(first)
        val quiet = first.renders()
        front.incoming(ME, "aa", "carol", "said to nobody")
        assertEquals("the gone screen renders no more", quiet, first.renders())
        val second = Screen(front)
        front.bind(second)
        assertEquals(
            listOf("while first was bound", "said to nobody"),
            second.lastThreads.single().messages.map { it.text },
        )
    }

    @Test
    fun a_stale_unbind_does_not_silence_the_replacement() {
        val front = Front()
        val old = Screen(front)
        val replacement = Screen(front)
        front.bind(old)
        front.bind(replacement)
        // the recreated Activity's predecessor stops late, its unbind naming
        // a screen no longer bound
        front.unbind(old)
        val before = replacement.renders()
        front.setStatus("still live")
        assertEquals(before + 1, replacement.renders())
    }

    @Test
    fun notices_are_bounded_and_the_oldest_go() {
        val front = Front()
        repeat(220) { front.note("line $it") }
        val n = front.notices()
        assertEquals(200, n.size)
        assertEquals("line 20", n.first())
        assertEquals("line 219", n.last())
    }

    // ---- the conversation model ----------------------------------------

    @Test
    fun messages_carry_their_sender_and_incoming_is_not_mine() {
        val front = Front()
        val id = front.outgoing(ME, "aa", "from me")
        front.settle(ME, "aa", id, Front.Delivery.SENT, null)
        front.incoming(ME, "aa", "carol", "from carol")
        val msgs = front.thread(ME, "aa")!!.messages
        assertEquals(2, msgs.size)
        assertTrue(msgs[0].mine)
        assertEquals("me", msgs[0].who)
        assertFalse(msgs[1].mine)
        assertEquals("carol", msgs[1].who)
    }

    @Test
    fun an_outgoing_message_is_sending_then_sent_never_delivered() {
        val front = Front()
        val id = front.outgoing(ME, "aa", "hi")
        assertEquals(Front.Delivery.SENDING, front.thread(ME, "aa")!!.messages.single().delivery)
        front.settle(ME, "aa", id, Front.Delivery.SENT, null)
        val m = front.thread(ME, "aa")!!.messages.single()
        assertEquals(Front.Delivery.SENT, m.delivery)
        assertNull(m.reason)
        // there is no DELIVERED or READ to reach: the enum has three states
        assertEquals(3, Front.Delivery.entries.size)
    }

    @Test
    fun a_refused_send_is_unsent_and_carries_its_reason() {
        val front = Front()
        val id = front.outgoing(ME, "aa", "hi")
        front.settle(ME, "aa", id, Front.Delivery.UNSENT, "no session")
        val m = front.thread(ME, "aa")!!.messages.single()
        assertEquals(Front.Delivery.UNSENT, m.delivery)
        assertEquals("no session", m.reason)
    }

    @Test
    fun the_direct_path_chip_is_status_only_and_defaults_off() {
        val front = Front()
        front.peerKnown(ME, "aa", "carol")
        assertFalse("no path is claimed before one is held", front.thread(ME, "aa")!!.direct)
        front.directPath(ME, "aa", true)
        assertTrue(front.thread(ME, "aa")!!.direct)
        front.directPath(ME, "aa", false)
        assertFalse(front.thread(ME, "aa")!!.direct)
    }

    /**
     * **A message belongs to the thread it was sent on**, which is what a
     * screen showing one party and a kernel sending to another broke: the
     * recipient was the kernel's one current peer, so a bubble typed in an
     * older thread appeared in the newest one and went to that party. The
     * front was always able to say this; nothing had asked it to.
     */
    @Test
    fun a_message_sits_in_the_thread_it_was_sent_on_and_in_no_other() {
        val front = Front()
        front.peerKnown(ME, "aa", "alice")
        front.peerKnown(ME, "bb", "bob")
        val toAlice = front.outgoing(ME, "aa", "for alice")
        val toBob = front.outgoing(ME, "bb", "for bob")
        assertEquals(listOf("for alice"), front.thread(ME, "aa")!!.messages.map { it.text })
        assertEquals(listOf("for bob"), front.thread(ME, "bb")!!.messages.map { it.text })
        // and each settles in its own thread, by its own id
        front.settle(ME, "aa", toAlice, Front.Delivery.SENT, null)
        front.settle(ME, "bb", toBob, Front.Delivery.UNSENT, "nobody carried it")
        assertEquals(Front.Delivery.SENT, front.thread(ME, "aa")!!.messages[0].delivery)
        assertEquals(Front.Delivery.UNSENT, front.thread(ME, "bb")!!.messages[0].delivery)
        assertEquals("nobody carried it", front.thread(ME, "bb")!!.messages[0].reason)
        // a settle naming the wrong thread moves nothing
        front.settle(ME, "aa", toBob, Front.Delivery.UNSENT, "wrong thread")
        assertEquals(Front.Delivery.SENT, front.thread(ME, "aa")!!.messages[0].delivery)
    }

    /** Every thread's chip is its own: read off one peer, the rest showed
     *  whatever they showed when that party was the current one. */
    @Test
    fun each_threads_direct_chip_is_its_own() {
        val front = Front()
        front.peerKnown(ME, "aa", "alice")
        front.peerKnown(ME, "bb", "bob")
        front.directPath(ME, "aa", true)
        assertTrue(front.thread(ME, "aa")!!.direct)
        assertFalse("bob's thread says nothing about alice's path", front.thread(ME, "bb")!!.direct)
        front.directPath(ME, "bb", true)
        front.directPath(ME, "aa", false)
        assertFalse(front.thread(ME, "aa")!!.direct)
        assertTrue(front.thread(ME, "bb")!!.direct)
    }

    @Test
    fun threads_keep_the_order_their_peers_were_first_known() {
        val front = Front()
        front.peerKnown(ME, "bb", "bob")
        front.peerKnown(ME, "aa", "alice")
        assertEquals(listOf("bob", "alice"), front.threads().map { it.peerName })
    }

    @Test
    fun concurrent_arrivals_are_all_kept_each_once_and_in_order() {
        // the lock's guarantee: a screen binding while the kernel speaks
        // sees every line, each once, in the order it was said
        repeat(20) {
            val front = Front()
            val said = (0 until 400).map { "line $it" }
            val screen = Screen(front)
            val speaker = thread { said.forEach { front.incoming(ME, "aa", "carol", it) } }
            front.bind(screen)
            speaker.join()
            front.thread(ME, "aa")!! // ensure the thread exists
            assertEquals(said, front.thread(ME, "aa")!!.messages.map { it.text })
        }
    }

    /**
     * **Each {identity, peer} is its own thread** — step 2b of the browser
     * model [author, 2026-11-11]: two identities on one device talking to
     * one party are two conversations, and neither sees the other's.
     */
    @Test
    fun two_identities_with_one_peer_are_two_threads() {
        val front = Front()
        val other = "22".repeat(32)
        front.incoming(ME, "aa", "carol", "to the first")
        front.incoming(other, "aa", "carol", "to the second")
        assertEquals(2, front.threads().size)
        assertEquals("to the first", front.thread(ME, "aa")!!.messages.single().text)
        assertEquals("to the second", front.thread(other, "aa")!!.messages.single().text)
        assertEquals(ME, front.thread(ME, "aa")!!.by)
        // and settling one identity's message never touches the other's
        val id = front.outgoing(ME, "aa", "mine")
        front.settle(other, "aa", id, Front.Delivery.SENT, null)
        assertEquals(Front.Delivery.SENDING, front.thread(ME, "aa")!!.messages.last().delivery)
    }
}
