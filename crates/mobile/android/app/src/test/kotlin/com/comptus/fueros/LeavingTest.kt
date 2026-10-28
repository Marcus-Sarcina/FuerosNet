package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **What the authority screen says before a departure** (the screens
 * sheet's E3; `light-client-requirements.md` §6, PRD-02, UX-006).
 *
 * The case worth the test is the empty one. A cost that came to nothing
 * and a cost that could not be worked out both arrive as an empty list of
 * losses, and they mean opposite things — so what the screen leads with
 * has to differ, and the limits have to print whether or not anything is
 * lost. §6's *stale permission list identified as stale, never silently
 * used* is that requirement from the other side.
 */
class LeavingTest {

    private val patronKey = "a3f21c0812345678abcdef0011223344556677889900aabbccddeeff00112233"
    private val node = "5046da3700000000000000000000000000000000000000000000000000000000"
    private val owner = "9b2c7e4100000000000000000000000000000000000000000000000000000000"

    private val patron = Kernel.Named(patronKey, "Rosa")
    private val nameless = Kernel.Named(patronKey, null)

    private fun cost(
        losing: List<Kernel.Lost> = emptyList(),
        keeping: Int = 0,
        leaving: List<Kernel.Named> = emptyList(),
        limits: List<Kernel.Limit> = listOf(Kernel.Limit.PredicateNotHeld),
    ) = Kernel.Cost(losing, keeping, leaving, limits)

    private fun lost(
        hostLeaves: Boolean = true,
        instance: String = "Standpipe rota",
        nodeAlias: String? = "Hallkeeper",
        ownerAlias: String? = "Rosa",
    ) = Kernel.Lost(
        resource = "0101010101010101010101010101010101010101010101010101010101010101",
        node = Kernel.Named(node, nodeAlias),
        owner = Kernel.Named(owner, ownerAlias),
        serviceType = "rhtn-forum",
        instance = instance,
        hostLeaves = hostLeaves,
    )

    // ---- the empty list, which is two different answers ----------------

    @Test
    fun a_cost_of_nothing_and_a_cost_that_could_not_be_worked_out_read_differently() {
        val free = cost(leaving = emptyList())
        val unknown = cost(
            limits = listOf(Kernel.Limit.PredicateNotHeld, Kernel.Limit.NotInTable),
        )
        assertTrue(Leaving.computed(free))
        assertFalse(Leaving.computed(unknown))
        assertEquals("Nothing this device can see goes with it.", Leaving.headline(free))
        assertEquals(
            "What leaving costs cannot be worked out on this device.",
            Leaving.headline(unknown),
        )
        // and the words, not just the headline, must not read as a figure
        val said = Leaving.words(unknown)
        assertTrue(said, said.contains("no figure to give"))
        assertFalse(
            "an uncomputable cost must not claim nothing goes: $said",
            said.contains("Nothing this device can see goes"),
        )
    }

    @Test
    fun the_standing_condition_is_said_every_time_including_when_nothing_is_lost() {
        for (c in listOf(cost(), cost(losing = listOf(lost())))) {
            assertTrue(
                "the floor is not stated: ${Leaving.words(c)}",
                Leaving.limits(c).contains(Leaving.LIMIT_PREDICATE),
            )
        }
        // and it is last, where it reads as a condition rather than news
        val c = cost(
            limits = listOf(Kernel.Limit.PredicateNotHeld, Kernel.Limit.Stale(node)),
        )
        assertEquals(Leaving.LIMIT_PREDICATE, Leaving.limits(c).last())
    }

    @Test
    fun what_cannot_be_computed_is_said_before_anything_else() {
        val c = cost(
            limits = listOf(
                Kernel.Limit.PredicateNotHeld,
                Kernel.Limit.Stale(node),
                Kernel.Limit.NotInTable,
            ),
        )
        assertEquals(Leaving.LIMIT_UNCOMPUTED, Leaving.limits(c).first())
    }

    // ---- the gaps in the list ------------------------------------------

    @Test
    fun a_stale_a_truncated_and_an_unswept_node_are_each_named_as_a_gap() {
        val c = cost(
            limits = listOf(
                Kernel.Limit.PredicateNotHeld,
                Kernel.Limit.Stale(node),
                Kernel.Limit.Truncated(node),
                Kernel.Limit.Unswept(owner),
            ),
        )
        val said = Leaving.limits(c)
        assertTrue(said.toString(), said.any { it.contains("could not be refreshed") })
        assertTrue(said.toString(), said.any { it.contains("will list at once") })
        assertTrue(said.toString(), said.any { it.contains("never been asked what") })
    }

    @Test
    fun several_nodes_of_one_kind_are_counted_rather_than_listed_one_each() {
        val c = cost(
            limits = listOf(
                Kernel.Limit.Unswept(node),
                Kernel.Limit.Unswept(owner),
                Kernel.Limit.Unswept(patronKey),
                Kernel.Limit.PredicateNotHeld,
            ),
        )
        val said = Leaving.limits(c)
        assertEquals("one sentence for the three, plus the floor: $said", 2, said.size)
        assertTrue(said.toString(), said[0].startsWith("3 nodes within reach have"))
    }

    @Test
    fun more_than_one_line_to_one_patron_says_the_cost_is_of_ending_them_all() {
        val c = cost(
            limits = listOf(Kernel.Limit.SeveralLines(2), Kernel.Limit.PredicateNotHeld),
        )
        assertTrue(
            Leaving.limits(c).toString(),
            Leaving.limits(c).any { it.contains("runs on 2 lines") && it.contains("ending them all") },
        )
    }

    // ---- the figure, and the two reasons -------------------------------

    /** The verb agreed with the count in one clause and not the other
     *  until this caught it: *1 person leave your horizon*. */
    @Test
    fun the_headline_counts_what_goes_and_who_goes_with_it() {
        assertEquals(
            "1 resource goes, and 1 person leaves your horizon.",
            Leaving.headline(cost(losing = listOf(lost()), leaving = listOf(patron))),
        )
        assertEquals(
            "2 resources go, and 3 people leave your horizon.",
            Leaving.headline(
                cost(
                    losing = listOf(lost(), lost(instance = "Minutes")),
                    leaving = listOf(patron, Kernel.Named(node, "Hallkeeper"), nameless),
                ),
            ),
        )
        assertEquals(
            "No resource this device knows of goes, but 2 people leave your horizon.",
            Leaving.headline(cost(leaving = listOf(patron, nameless))),
        )
    }

    @Test
    fun a_host_that_goes_and_an_owner_that_goes_are_told_apart() {
        val host = Leaving.lost(lost(hostLeaves = true))
        val own = Leaving.lost(lost(hostLeaves = false))
        assertTrue(host, host.contains("the node serving it leaves your horizon: Hallkeeper, 5046da370000…"))
        assertTrue(own, own.contains("its owner leaves your horizon: Rosa, 9b2c7e410000…"))
        assertTrue(
            "an owner gone with the host still there is a door that will not open: $own",
            own.contains("the node stays"),
        )
        assertTrue(host, host.startsWith("Standpipe rota   rhtn-forum"))
    }

    // ---- the name, and the keyhash under it ----------------------------

    /** The author's ruling of 2026-10-28: show the alias here. Both,
     *  though — an alias is what somebody signed about themselves, and
     *  the act is not undoable. */
    @Test
    fun a_party_is_named_by_the_alias_and_the_keyhash_together() {
        assertEquals("Rosa, a3f21c081234…", Leaving.party(patron))
        assertEquals(
            "a published alias that arrived blank is no name at all",
            "a3f21c081234…",
            Leaving.party(Kernel.Named(patronKey, "  ")),
        )
    }

    @Test
    fun a_party_who_published_no_name_is_shown_by_the_keyhash_alone() {
        assertEquals("a3f21c081234…", Leaving.party(nameless))
        assertEquals("a3f21c081234…   series 3", Leaving.line(Kernel.Line(nameless, 3U)))
        assertEquals("Leave a3f21c081234…", Leaving.button(nameless, armed = false))
    }

    @Test
    fun a_resource_whose_host_published_no_name_still_says_which_node() {
        val said = Leaving.lost(lost(nodeAlias = null))
        assertTrue(said, said.contains("leaves your horizon: 5046da370000…"))
    }

    @Test
    fun a_resource_with_no_published_name_is_shown_by_its_keyhash() {
        val said = Leaving.lost(lost(instance = ""))
        assertTrue(said, said.startsWith("010101010101…"))
    }

    @Test
    fun what_stays_is_counted_only_where_the_answer_is_one() {
        assertEquals("4 resources stay within reach.", Leaving.kept(cost(keeping = 4)))
        assertEquals("1 resource stays within reach.", Leaving.kept(cost(keeping = 1)))
        assertEquals(null, Leaving.kept(cost(keeping = 0)))
        assertEquals(
            "a count of what stays means nothing where nothing was computed",
            null,
            Leaving.kept(cost(keeping = 4, limits = listOf(Kernel.Limit.NotInTable))),
        )
    }

    // ---- the lines, and the act ----------------------------------------

    @Test
    fun no_patron_says_there_is_nothing_to_leave_and_where_one_comes_from() {
        assertTrue(Leaving.NONE, Leaving.NONE.contains("under no patron"))
        assertTrue(Leaving.NONE, Leaving.NONE.contains("Meet screen"))
        assertEquals("Rosa, a3f21c081234…   series 3", Leaving.line(Kernel.Line(patron, 3U)))
    }

    /** The same slip as *1 person leave your horizon*, twice more: a
     *  singular count with a plural pronoun after it. */
    @Test
    fun one_node_and_several_nodes_each_take_the_pronoun_that_agrees() {
        fun said(n: Int, of: (String) -> Kernel.Limit) =
            Leaving.limits(cost(limits = List(n) { of(node) })).first()
        assertEquals(
            "1 node holds more than it will list at once, so it serves things this " +
                "device has never seen.",
            said(1) { Kernel.Limit.Truncated(it) },
        )
        assertTrue(said(2) { Kernel.Limit.Truncated(it) }.startsWith("2 nodes hold more than they will"))
        assertEquals(
            "1 node within reach has never been asked what it serves, so whatever " +
                "that is, it is not counted above.",
            said(1) { Kernel.Limit.Unswept(it) },
        )
        assertTrue(said(2) { Kernel.Limit.Unswept(it) }.contains("have never been asked what they serve"))
    }

    @Test
    fun the_button_asks_twice_and_says_so_the_second_time() {
        assertEquals("Leave Rosa", Leaving.button(patron, armed = false))
        assertEquals("Tap again to leave Rosa", Leaving.button(patron, armed = true))
    }
}
