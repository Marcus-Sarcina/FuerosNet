package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
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

    // functional: UX-012
    // functional: UX-015
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

    // functional: UX-015
    @Test
    fun the_foot_carries_a_name_where_one_was_published_and_a_keyhash_otherwise() {
        assertEquals("Marcus" to "home", Chrome.foot("Marcus", sub))
        assertEquals("a3f21c081234…" to "home", Chrome.foot(null, sub))
        assertEquals("a3f21c081234…" to "home", Chrome.foot(" ", sub))
    }
    /**
     * **PRD-04: a second patron is suggested while only one is held.**
     *
     * **Nothing here waits for an eclipse** [author, 2026-11-05]: *"It is
     * a theoretical attack, not an intended feature of the network."* So
     * what is asked is how many patrons there are, and nothing else.
     */
    // functional: UX-005
    // acceptance: PRD-04
    @Test
    fun a_second_patron_is_suggested_while_one_is_held_and_not_otherwise() {
        assertTrue("one patron is the case it is for", Chrome.suggestingSecondPatron(1))
        // nobody with no patron has a second to add
        assertFalse("no patron, nothing to add to", Chrome.suggestingSecondPatron(0))
        // and once a second is held it is done, however many follow
        assertFalse("two is already plurality", Chrome.suggestingSecondPatron(2))
        assertFalse(Chrome.suggestingSecondPatron(7))
    }

    // functional: UX-005
    // acceptance: PRD-04
    @Test
    fun the_suggestion_says_what_a_single_patron_costs() {
        val said = Disclosure.words(Disclosure.Topic.SECOND_PATRON).joinToString(" ")
        assertTrue("the words come from the table", said.isNotEmpty())
        assertTrue("names the single patron: $said", said.contains("single patron"))
        assertTrue("says what flows through them: $said", said.contains("reachability"))
        assertTrue("and suggests the remedy: $said", said.contains("second adoption"))
    }

    private fun offered(node: String, resource: String, name: String, waiting: Int) =
        uniffi.rhtn_model.Resource(
            node = node.unhex(),
            resource = resource.unhex(),
            owner = "99".repeat(32).unhex(),
            serviceType = "",
            instance = name,
            endpoint = ByteArray(0),
            dataPractice = null,
            truncated = false,
            stale = false,
            waiting = waiting.toUInt(),
            notice = null,
            iconFormat = null,
            icon = null,
        )

    // functional: UX-018
    @Test
    fun a_subnet_launches_what_its_node_offers_and_its_orb_wears_their_count() {
        val ours = "aa".repeat(32)
        val theirs = "bb".repeat(32)
        val held = listOf(
            offered(ours, "11".repeat(32), "testapp.internal", 0),
            offered(theirs, "22".repeat(32), "elsewhere", 4),
            offered(ours, "33".repeat(32), "Board", 2),
        )
        val launched = Chrome.launchers(held, ours)
        assertEquals(
            "one card each for what the subnet's node offered, and nothing another node did",
            listOf(
                Chrome.Launcher(ours, "33".repeat(32), "Board", 2),
                Chrome.Launcher(ours, "11".repeat(32), "testapp.internal", 0),
            ),
            launched,
        )
        assertEquals("the resources' asks roll up", 2, Chrome.unseen(false, launched))
        assertEquals("and a move of the node besides", 3, Chrome.unseen(true, launched))
        assertEquals(0, Chrome.unseen(false, emptyList()))
    }

    // functional: CAT-029
    @Test
    fun a_card_wears_the_icon_its_resource_served() {
        val ours = "aa".repeat(32)
        val png = byteArrayOf(0x89.toByte(), 0x50, 0x4e, 0x47)
        val held = listOf(
            offered(ours, "11".repeat(32), "testapp.internal", 0).copy(iconFormat = 1uL, icon = png),
            offered(ours, "22".repeat(32), "Board", 0),
        )
        val launched = Chrome.launchers(held, ours)
        assertSame(png, launched.first { it.name == "testapp.internal" }.icon)
        assertNull("its mark stands in", launched.first { it.name == "Board" }.icon)
    }

    // functional: CAT-030
    @Test
    fun the_shade_is_told_only_a_rise_that_carries_a_line() {
        val ours = "aa".repeat(32)
        val said = { r: String, n: Int, line: String? -> offered(ours, r, "app", n).copy(notice = line) }
        val held = listOf(
            said("11".repeat(32), 3, "Three replies"),
            said("22".repeat(32), 2, "Two orders"),
            said("33".repeat(32), 4, null),
            said("44".repeat(32), 1, "One step"),
        )
        val seen = mapOf("11".repeat(32) to 1, "22".repeat(32) to 2, "44".repeat(32) to 5)
        assertEquals(
            "a rise with a line; not a count unchanged, a count fallen, or one with nothing to say",
            listOf("11".repeat(32)),
            Notify.rising(seen, held).map { it.resource.hex() },
        )
        assertEquals("nothing seen is nothing waiting before", 3, Notify.rising(emptyMap(), held).size)
    }

    @Test
    fun a_person_keeps_the_colour_their_key_gives_them_until_one_is_chosen() {
        val key = "a3" + "00".repeat(31)
        val given = Chrome.personColour(null, key)
        assertEquals(given, Chrome.personColour(null, key))
        assertEquals(Palette.IDENTITY[0xa3 % Palette.IDENTITY.size], given)
        assertEquals("Teal", Chrome.personColour("Teal", key).first)
        assertEquals("a name the palette lacks is not chosen", given, Chrome.personColour("Mauve", key))
    }

    @Test
    fun a_card_says_above_with_a_plus_below_with_a_minus_and_across_as_nought() {
        assertEquals("+2", Chrome.standing(2))
        assertEquals("-1", Chrome.standing(-1))
        assertEquals("0", Chrome.standing(0))
        assertEquals(null, Chrome.standing(null))
    }

    @Test
    fun ink_reads_on_every_person_colour_bubble_in_both_modes() {
        for (set in listOf(Palette.DARK, Palette.LIGHT)) {
            assertTrue("ink on the grey of one's own bubble", Palette.contrast(set.ink, set.control) >= 4.5)
            for ((name, c) in Palette.IDENTITY) {
                val bubble = Palette.mix(c, set.ground, Chrome.BUBBLE_SHARE)
                assertTrue("ink on $name's bubble", Palette.contrast(set.ink, bubble) >= 4.5)
            }
        }
    }

    private fun thread(title: String?, posts: Int, unread: Int, latest: Long) =
        uniffi.rhtn_model.BoardThread(
            id = ByteArray(32) { 0x44 },
            title = title,
            beganBy = null,
            posts = posts.toUInt(),
            unread = unread.toUInt(),
            latest = latest.toULong(),
        )

    @Test
    fun a_subnets_count_rolls_in_what_is_unread_on_its_board() {
        val threads = listOf(thread("One", 3, 2, 100), thread("Two", 1, 0, 50), thread(null, 4, 4, 10))
        assertEquals(6, Chrome.boardUnread(threads))
        assertEquals(0, Chrome.boardUnread(emptyList()))
        assertEquals("beside a move of its node", 7, Chrome.unseen(true, emptyList(), 6))
        assertEquals("a board is one identity's in one subnet", "ab:22", Chrome.boardKey("ab", "22"))
    }

    @Test
    fun a_reach_is_said_as_written_and_moves_a_level_at_a_time_within_its_bound() {
        assertEquals("+2/-2", Chrome.reach(Chrome.DEFAULT_REACH, Chrome.DEFAULT_REACH))
        assertEquals("+0/-24", Chrome.reach(0, 24))
        assertEquals(3, Chrome.step(2, 1))
        assertEquals(0, Chrome.step(0, -1))
        assertEquals(Chrome.MAX_REACH, Chrome.step(Chrome.MAX_REACH, 1))
    }

    @Test
    fun a_threads_row_says_its_title_what_is_unread_and_how_it_stands() {
        val now = 10_000L
        assertEquals(
            "Meeting notes (2 unread)" to "began by Cy, 3 posts, 2 h ago",
            Chrome.threadRow(thread("Meeting notes", 3, 2, now - 7_200), "Cy", now),
        )
        assertEquals(
            "A thread begun out of your reach" to "1 post, just now",
            Chrome.threadRow(thread(null, 1, 0, now), null, now),
        )
        assertEquals("10 min ago", Chrome.ago(700, 100))
        assertEquals("3 d ago", Chrome.ago(100 + 3 * 86_400, 100))
        assertEquals("a clock behind the writer's", "just now", Chrome.ago(100, 900))
    }

    @Test
    fun a_post_is_by_the_name_held_else_the_one_it_carried_else_its_key() {
        val key = "ab".repeat(32)
        assertEquals("Held", Chrome.writer("Held", "Carried", key))
        assertEquals("Carried", Chrome.writer(null, "Carried", key))
        assertEquals("abababab…", Chrome.writer(null, null, key))
    }
}
