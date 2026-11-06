package com.comptus.fueros

import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **PRD-19's three cases, told apart** — and the clause that none of them
 * is presented as a fault, which is the one a count alone would break.
 */
class WitnessingTest {

    // acceptance: PRD-19
    @Test
    fun both_sides_survivors_gives_the_count_and_the_split() {
        val said = Witnessing.split(mine = 2, others = 3, theirs = "Pat")!!
        assertTrue("says how many attested: $said", said.contains("5 witnesses attest"))
        assertTrue("says how many were yours: $said", said.contains("2 nominees of yours"))
        assertTrue("and how many theirs: $said", said.contains("3 nominees of Pat's"))
    }

    // acceptance: PRD-19
    @Test
    fun the_counterpartys_alone_says_so_and_that_yours_are_absent() {
        val said = Witnessing.split(mine = 0, others = 2, theirs = "Pat")!!
        assertTrue("says they were all theirs: $said", said.contains("all nominated by Pat"))
        assertTrue("and that none of yours is there: $said", said.contains("None of yours"))
        // **not a fault of this side's** (PRD-19): the words say what
        // happened — they were not reachable — and nothing about failing
        assertTrue("says why, as reachability: $said", said.contains("not reachable"))
        assertTrue("and that it is held against nobody: $said", said.contains("does not hold"))
    }

    // acceptance: PRD-19
    @Test
    fun this_sides_alone_reads_the_same_way_round() {
        val said = Witnessing.split(mine = 2, others = 0, theirs = "Pat")!!
        assertTrue("$said", said.contains("all nominated by you"))
        assertTrue("$said", said.contains("None of Pat's"))
        assertTrue("$said", said.contains("not reachable"))
    }

    // acceptance: PRD-19
    @Test
    fun nobody_attesting_is_not_reported_as_a_count_of_zero() {
        // the witnessless case is said in its own words on the screen —
        // that the record finalises and says so permanently — rather than
        // as "0 witnesses attest", which reads as a tally gone wrong
        assertNull(Witnessing.split(mine = 0, others = 0, theirs = "Pat"))
    }

    // acceptance: PRD-19
    @Test
    fun none_of_the_cases_blames_anybody() {
        val cases = listOf(
            Witnessing.split(1, 1, "Pat"),
            Witnessing.split(0, 1, "Pat"),
            Witnessing.split(1, 0, "Pat"),
        ).filterNotNull()
        assertTrue("every case says something", cases.size == 3)
        for (said in cases) {
            for (blame in listOf("failed", "fault", "should have", "unfortunately", "only")) {
                assertTrue("a case reads as a fault — `$blame` in: $said", !said.contains(blame))
            }
        }
    }

    @Test
    fun one_witness_reads_as_english() {
        val said = Witnessing.split(mine = 1, others = 0, theirs = "Pat")!!
        assertTrue("$said", said.contains("1 witness attests"))
        assertTrue("$said", !said.contains("1 witnesses"))
    }
}
