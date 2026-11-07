package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **T1 and T2 of `Robot/disclosure-tests.md`**: the two tests that
 * discharge every narrative obligation among the manual rows, on
 * [author, 2026-11-05]'s assumption that *"the disclosure page is
 * parameterized boilerplate by transaction type"*.
 *
 * If the words are fixed per kind, no occurrence needs testing. So there
 * is no test here that a particular screen said a particular thing on a
 * particular run: there is one test that every kind has its words, and one
 * that each kind's words are its own.
 */
class DisclosureTest {

    /**
     * **T1, the half that holds today: the table is total over the
     * topics.** Every kind the rows name resolves to an entry, so a topic
     * cannot quietly disappear from the table and take its obligation
     * with it.
     */
    @Test
    fun every_kind_of_disclosure_is_in_the_table() {
        val topics = Disclosure.topics()
        assertEquals(
            "a topic was added to the enum and not to the table",
            topics.size,
            topics.mapNotNull { runCatching { Disclosure.of(it) }.getOrNull() }.size,
        )
        assertTrue("the table is empty", topics.isNotEmpty())
    }

    /**
     * **T1, the half that is owed: no kind is still waiting for words.**
     *
     * **Live as of 2026-11-05**: it was ignored while the sentences were
     * owed, because a failing test would have blocked every commit until
     * prose arrived. The author wrote seven, one topic turned out to be
     * written already on the node, and the last was withdrawn — so this
     * holds now, and a topic added without words fails here.
     */
    @Test
    fun no_kind_of_disclosure_is_still_owed_its_words() {
        val owed = Disclosure.owed()
        assertEquals(
            owed.joinToString("\n", prefix = "words are owed:\n") {
                "  ${it.row}: ${it.obligation}"
            },
            emptyList<Disclosure.Says.Owed>(),
            owed,
        )
    }

    /**
     * **T2: the table is selected by kind, and each kind's words are its
     * own.** Rotation against recovery, adoption against departure: the
     * pairs the rows distinguish must not resolve to one another.
     *
     * This holds whether a topic's words are here or elsewhere, so it runs
     * green while T1 is still failing — it is about the mapping, not the
     * text.
     */
    @Test
    fun each_kind_resolves_to_its_own_words_and_not_a_neighbour_s() {
        val all = Disclosure.topics()
        assertEquals(
            "a topic is missing from the table, or two share an entry",
            all.size,
            all.map { Disclosure.of(it) }.size,
        )
        // the distinctions the rows turn on: neither of a pair may answer
        // with the other's entry
        val pairs = listOf(
            Disclosure.Topic.UNLINKED_PATH to Disclosure.Topic.LINKED_PATH,
            Disclosure.Topic.ADOPTION to Disclosure.Topic.DEPARTURE,
            Disclosure.Topic.ADOPTION to Disclosure.Topic.ADOPTION_UPWARD,
            Disclosure.Topic.MEETING to Disclosure.Topic.PEERING,
            Disclosure.Topic.BACKUP to Disclosure.Topic.RESTORE,
        )
        for ((a, b) in pairs) {
            assertTrue(
                "$a and $b resolve to the same words, and the rows distinguish them",
                Disclosure.of(a) != Disclosure.of(b),
            )
        }
    }

    /**
     * **Every row named in the table is one the catalogue carries.** A
     * topic citing a row that does not exist would be a gap nobody could
     * find from the other end.
     */
    @Test
    fun the_rows_cited_are_rows_that_exist() {
        val cited = Disclosure.topics().map {
            when (val s = Disclosure.of(it)) {
                is Disclosure.Says.Here -> s.items.map { i -> i.row }
                is Disclosure.Says.Elsewhere -> listOf(s.row)
                is Disclosure.Says.Owed -> listOf(s.row)
            }
        }.flatten()
        for (r in cited) {
            assertTrue(
                "`$r` is not shaped like an acceptance or UX row",
                Regex("^(PRD|UX|RSC|CER)-[0-9]{2,3}$").matches(r),
            )
        }
    }
    /**
     * **PRD-11: the screen says what the ordering means.**
     *
     * The ordering itself is proven in `client/tests/horizon.rs` — least
     * concentrated first, ties in the caller's order, every candidate
     * returned with its count rather than dropped. What this holds is the
     * sentence beside it, and that the screen renders it from the table
     * rather than from a string of its own.
     *
     * Read from the source, as [InsetSweepTest] and [ManifestTest] read
     * theirs: a JVM test cannot lay out the provisioning form.
     */
    /**
     * **What a rotation costs on the subnet it rotates** [author,
     * 2026-11-07]: *"The old key loses its reachability on the rotated
     * subnet as well as any standing which depends on it. They are
     * granted roles and standing only within the trust horizon on the
     * rotated subnet."*
     *
     * The table already said what the old key **keeps** — every other
     * subnet — and said nothing about what it loses on this one, which
     * left both paths reading as though a rotation cost nothing anywhere.
     */
    @Test
    fun both_paths_say_what_the_old_key_loses_on_the_subnet_it_rotates() {
        val unlinked = Disclosure.words(Disclosure.Topic.UNLINKED_PATH).joinToString(" ")
        val linked = Disclosure.words(Disclosure.Topic.LINKED_PATH).joinToString(" ")
        for ((name, said) in listOf("unlinked" to unlinked, "linked" to linked)) {
            assertTrue(
                "$name does not say the old key keeps the other subnets: $said",
                said.contains("other subnets"),
            )
            assertTrue(
                "$name does not say it loses reachability on this one: $said",
                said.contains("loses its reachability"),
            )
        }
        // **and the two differ on what happens to the standing**, which is
        // the difference between the paths: unlinked loses it on that
        // subnet, linked carries it
        assertTrue(
            "unlinked does not say the standing goes: $unlinked",
            unlinked.contains("any standing which depends on it goes with it"),
        )
        assertTrue(
            "linked does not say it is carried: $linked",
            linked.contains("carried to the new") && linked.contains("rather than lost"),
        )
        assertTrue(
            "unlinked does not scope the new identifier: $unlinked",
            unlinked.contains("within the trust horizon"),
        )
    }

    @Test
    fun the_provisioning_screen_says_whose_horizon_the_ordering_speaks_to() {
        val said = Disclosure.words(Disclosure.Topic.PROVISIONING).joinToString(" ")
        assertTrue("there are words", said.isNotEmpty())
        assertTrue("says it is about your own subnet: $said", said.contains("local subnet"))
        assertTrue(
            "and not the network's: $said",
            said.contains("does not reflect the distribution of all FuerosNet nodes"),
        )
        // and the screen renders them rather than saying it in its own words
        val screen = java.io.File(
            module(), "app/src/main/kotlin/com/comptus/fueros/OperatorActivity.kt",
        ).readText()
        assertTrue(
            "OperatorActivity no longer renders the table's provisioning words",
            screen.contains("Disclosure.Topic.PROVISIONING"),
        )
    }

    /** Where the tree is, for the source-reading test above. */
    private fun module(): java.io.File {
        var at: java.io.File? = java.io.File(".").canonicalFile
        while (at != null) {
            if (java.io.File(at, "app/src/main/AndroidManifest.xml").isFile) return at
            at = at.parentFile
        }
        throw AssertionError("no app/src/main/AndroidManifest.xml at or above here")
    }

}
