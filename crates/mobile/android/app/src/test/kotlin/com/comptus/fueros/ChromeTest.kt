package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **What the shell shows around you, and where** (`Robot/light-client-
 * screens.md`, *the shell*).
 *
 * The rulings this holds are the author's of 2026-10-27: a thread hides
 * the side and bottom navigation; a subnet's icon opens that subnet
 * alone; only the highest of three standings is drawn on an orb; and a
 * name that has not been received is never shown.
 */
class ChromeTest {

    private val sub = "a3f21c0812345678abcdef0011223344556677889900aabbccddeeff00112233"

    @Test
    fun the_rail_and_the_foot_persist_except_where_the_author_ruled_they_do_not() {
        for (kept in listOf(
            Chrome.Place.Home,
            Chrome.Place.Subnet(sub),
            Chrome.Place.Direct,
        )) {
            assertTrue("$kept keeps the chrome", Chrome.chrome(kept))
            assertFalse("so it needs no back control", Chrome.back(kept))
        }
        for (gone in listOf(
            Chrome.Place.Thread("t", sub),
            Chrome.Place.Thread("t", null),
            Chrome.Place.Resource("n", "r"),
            Chrome.Place.Options,
            Chrome.Place.SubnetOptions(sub),
            Chrome.Place.Meet,
        )) {
            assertFalse("$gone takes the screen", Chrome.chrome(gone))
            assertTrue("so it carries the way back itself", Chrome.back(gone))
        }
    }

    @Test
    fun the_rail_pins_two_things_above_the_subnets_and_divides_them() {
        val subnets = listOf(
            Chrome.Rung.Subnet(sub, "Allotments", 12, Chrome.Crown.GOLD),
            Chrome.Rung.Subnet("9b2c", null, 0, Chrome.Crown.NONE),
        )
        val rail = Chrome.rail(unseenDirect = 3, subnets = subnets)
        assertEquals(Chrome.Rung.Direct(3), rail[0])
        assertEquals(Chrome.Rung.Meet, rail[1])
        assertEquals(Chrome.Rung.Divider, rail[2])
        assertEquals(subnets, rail.drop(3))

        // **no divider with nothing under it**: a line between the pinned
        // and an empty scroll is a line about nothing
        val bare = Chrome.rail(unseenDirect = 0, subnets = emptyList())
        assertEquals(listOf(Chrome.Rung.Direct(0), Chrome.Rung.Meet), bare)
    }

    /** The author's ruling: the three overlap, so the highest wins —
     *  root before node before subordinates. */
    @Test
    fun an_orb_wears_only_the_highest_standing_that_applies() {
        assertEquals(
            Chrome.Crown.FULL,
            Chrome.crown(root = true, operator = true, hasSubordinate = true),
        )
        assertEquals(
            Chrome.Crown.GOLD,
            Chrome.crown(root = false, operator = true, hasSubordinate = true),
        )
        assertEquals(
            Chrome.Crown.SILVER,
            Chrome.crown(root = false, operator = false, hasSubordinate = true),
        )
        assertEquals(
            Chrome.Crown.NONE,
            Chrome.crown(root = false, operator = false, hasSubordinate = false),
        )
        // a root with nothing else is still a root
        assertEquals(
            Chrome.Crown.FULL,
            Chrome.crown(root = true, operator = false, hasSubordinate = false),
        )
    }

    @Test
    fun an_orbs_mark_is_the_published_name_or_the_keyhash_and_never_a_guess() {
        assertEquals("Al", Chrome.mark("Allotments", sub))
        assertEquals("NW", Chrome.mark("North Ward", sub))
        assertEquals("NW", Chrome.mark("north ward", sub), )
        assertEquals("Jo", Chrome.mark("joiners", sub))
        // **never a name that has not been received**
        assertEquals("a3f2", Chrome.mark(null, sub))
        assertEquals("a3f2", Chrome.mark("   ", sub))
    }

    @Test
    fun a_count_is_shown_until_it_stops_being_a_number_anybody_reads() {
        assertNull("nothing unseen is no badge at all", Chrome.badge(0))
        assertNull(Chrome.badge(-1))
        assertEquals("1", Chrome.badge(1))
        assertEquals("99", Chrome.badge(99))
        assertEquals("99+", Chrome.badge(100))
        assertEquals("99+", Chrome.badge(4000))
    }

    @Test
    fun what_the_rail_says_aloud_names_the_standing_and_the_count() {
        assertEquals(
            "Allotments, you run a node here, 12 unseen",
            Chrome.said(Chrome.Rung.Subnet(sub, "Allotments", 12, Chrome.Crown.GOLD)),
        )
        assertEquals(
            "Joiners, you are its root",
            Chrome.said(Chrome.Rung.Subnet(sub, "Joiners", 0, Chrome.Crown.FULL)),
        )
        assertEquals(
            "subnet a3f21c08, 1 unseen",
            Chrome.said(Chrome.Rung.Subnet(sub, null, 1, Chrome.Crown.NONE)),
        )
        assertEquals("Direct messages, 3 unseen", Chrome.said(Chrome.Rung.Direct(3)))
        assertEquals("Direct messages", Chrome.said(Chrome.Rung.Direct(0)))
        assertEquals("Meet somebody", Chrome.said(Chrome.Rung.Meet))
    }

    @Test
    fun the_foot_carries_a_name_where_one_was_published_and_a_keyhash_otherwise() {
        assertEquals("Marcus" to "home", Chrome.foot("Marcus", sub))
        assertEquals("a3f21c081234…" to "home", Chrome.foot(null, sub))
        assertEquals("a3f21c081234…" to "home", Chrome.foot(" ", sub))
    }
}
