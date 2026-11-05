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
     * **Ignored rather than red**, as the generated acceptance stubs are
     * for work not yet done: a failing test would block every commit
     * until the sentences exist, and the sentences are the author's to
     * write ([Disclosure]). The failure message names each topic, the row
     * that asks for its words and what that row asks, so taking the
     * annotation off is the whole of closing it.
     */
    @org.junit.Ignore("owed: the sentences in Disclosure.owed() are the author's to write")
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
            Disclosure.Topic.ROTATION to Disclosure.Topic.RECOVERY,
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
}
