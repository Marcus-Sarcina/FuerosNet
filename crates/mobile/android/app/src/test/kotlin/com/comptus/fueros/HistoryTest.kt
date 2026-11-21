package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **Where back goes in the shell** [author, 2026-11-20]: the previous
 * place, else home, and only from home with nothing behind it the
 * platform's, which leaves the app.
 */
class HistoryTest {

    private val home = Chrome.Place.Home
    private val a = Chrome.Place.Subnet("a3f2")
    private val b = Chrome.Place.Subnet("9b2c")
    private val aOptions = Chrome.Place.SubnetOptions("a3f2")

    private fun fresh() = History<Chrome.Place>(home)

    @Test
    fun from_home_with_nothing_behind_it_back_is_the_platforms() {
        val h = fresh()
        assertEquals(home, h.here)
        assertFalse("nothing for the shell to take", h.holding)
        assertNull(h.back())
        assertEquals(home, h.here)
    }

    @Test
    fun back_retraces_the_places_visited_then_stops_at_home() {
        val h = fresh()
        h.go(a)
        h.go(b)
        h.go(Chrome.Place.Options)
        assertTrue(h.holding)
        assertEquals(listOf(home, a, b), h.trail)
        assertEquals(b, h.back())
        assertEquals(a, h.back())
        assertEquals(home, h.back())
        assertFalse("home with nothing behind it", h.holding)
        assertNull(h.back())
    }

    @Test
    fun a_redraw_of_the_place_shown_is_not_a_visit() {
        val h = fresh()
        h.go(home)
        assertFalse(h.holding)
        h.go(a)
        h.go(a)
        h.go(a)
        assertEquals(listOf(home), h.trail)
        assertEquals(home, h.back())
        assertNull(h.back())
    }

    @Test
    fun going_home_empties_the_history() {
        val h = fresh()
        h.go(a)
        h.go(aOptions)
        h.go(b)
        h.go(home)
        assertEquals(emptyList<Chrome.Place>(), h.trail)
        assertFalse(h.holding)
        assertNull(h.back())
    }

    @Test
    fun a_place_already_behind_cuts_the_history_back_to_it() {
        val h = fresh()
        h.go(a)
        h.go(aOptions)
        // the title bar's `‹` names the subnet: the same as back would do
        h.go(a)
        assertEquals(listOf(home), h.trail)
        assertEquals(a, h.here)

        // a loop between two places does not grow it
        repeat(10) {
            h.go(b)
            h.go(a)
        }
        assertEquals(listOf(home), h.trail)
        h.go(b)
        assertEquals(listOf(home, a), h.trail)
        assertEquals(a, h.back())
        assertEquals(home, h.back())
    }

    @Test
    fun the_oldest_is_dropped_beyond_the_depth_and_back_past_it_is_home() {
        val h = History<Chrome.Place>(home, depth = 3)
        val places = (1..6).map { Chrome.Place.Subnet("s$it") }
        for (p in places) h.go(p)
        assertEquals(places.subList(2, 5), h.trail)
        assertEquals(places[4], h.back())
        assertEquals(places[3], h.back())
        assertEquals(places[2], h.back())
        // nothing kept behind it, and not home: back is home, then the platform's
        assertTrue(h.holding)
        assertEquals(home, h.back())
        assertNull(h.back())
    }

    @Test
    fun a_place_away_from_home_with_nothing_behind_it_still_backs_home() {
        val h = History<Chrome.Place>(home, depth = 1)
        h.go(a)
        h.go(b)
        assertEquals(listOf(a), h.trail)
        assertEquals(a, h.back())
        assertTrue("away from home is the shell's", h.holding)
        assertEquals(home, h.back())
        assertFalse(h.holding)
    }
}
