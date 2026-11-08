package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **Several identities on one device** [author, 2026-11-07], and the
 * division of what each may do.
 */
class IdentitiesTest {

    private fun id(n: Int) = "%064x".format(n)

    private fun held(n: Int, scope: String? = null, alias: String? = null) =
        Identities.Held(id(n), scope, alias)

    @Test
    fun a_meeting_and_an_operator_action_take_the_active_one_and_nothing_else_does() {
        // **the whole of the rule** [author, 2026-11-07]: "Concurrent for
        // viewing or acting within subnets … but only one ID active for
        // meetings or performing operator actions"
        assertTrue(Identities.needsTheActive(Identities.Act.MEETING))
        assertTrue(Identities.needsTheActive(Identities.Act.OPERATOR))
        assertFalse(Identities.needsTheActive(Identities.Act.VIEWING))
        assertFalse(Identities.needsTheActive(Identities.Act.WITHIN_SUBNET))
        // **a conversation is a third answer**: it belongs to the identity
        // it started in, so switching which one acts must not move it
        assertFalse(
            "a conversation follows neither the active identity nor the caller",
            Identities.needsTheActive(Identities.Act.CONVERSATION),
        )
    }

    @Test
    fun the_index_round_trips_with_its_scopes_and_aliases() {
        val all = listOf(
            held(1, Identities.LEGACY_SCOPE, "Marcus"),
            held(2, Identities.scopeFor(id(2)), "a second one"),
            held(3, Identities.scopeFor(id(3)), null),
        )
        val back = Identities.decode(Identities.encode(all))
        assertEquals(all, back)
        // the one that held the top-level directory keeps it, which is
        // what makes the migration move no files
        assertNull("the legacy scope stays null", back[0].scope)
        assertEquals(id(2), back[1].scope)
    }

    @Test
    fun an_alias_with_a_space_survives_and_one_with_a_newline_is_not_written() {
        val spaced = listOf(held(1, null, "two words"))
        assertEquals("two words", Identities.decode(Identities.encode(spaced))[0].alias)
        // a newline would write a second record, so the line is dropped
        // rather than written and read back as something else
        val broken = listOf(held(1, null, "one\ntwo"))
        assertEquals(listOf<Identities.Held>(), Identities.decode(Identities.encode(broken)))
    }

    @Test
    fun a_line_this_model_would_not_have_written_is_dropped_on_reading() {
        val text = (
            "notanid - nope\n" +
                "${id(1)} - Marcus\n" +
                "\n" +
                "${id(1)} - a duplicate\n" +
                "${"g".repeat(64)} - not hex\n"
            ).toByteArray()
        val back = Identities.decode(text)
        assertEquals("one identity survives", 1, back.size)
        assertEquals(id(1), back[0].id)
        assertEquals("the first record of an id wins", "Marcus", back[0].alias)
        assertEquals("nothing at all from nothing", listOf<Identities.Held>(), Identities.decode(null))
    }

    @Test
    fun an_id_is_only_a_name_this_model_accepts_if_it_could_be_a_directory() {
        // the scope **is** the id, so anything that could name a path must
        // not be accepted
        assertTrue(Identities.names(id(7)))
        assertFalse("too short", Identities.names("abcd"))
        assertFalse("upper case", Identities.names("A".repeat(64)))
        assertFalse("a traversal", Identities.names("../" + "a".repeat(61)))
        assertFalse("a separator", Identities.names("a".repeat(63) + "/"))
        assertEquals("and the scope is the id itself", id(7), Identities.scopeFor(id(7)))
    }

    @Test
    fun the_identity_held_before_this_model_is_adopted_once_and_idempotently() {
        val none = listOf<Identities.Held>()
        assertEquals(
            Identities.Migration.Adopt(id(1)),
            Identities.migrated(hasLegacySeeds = true, legacyId = id(1), index = none),
        )
        // **idempotent**: a start that died after sealing the index and
        // before forgetting the legacy name must change nothing
        assertEquals(
            Identities.Migration.None,
            Identities.migrated(true, id(1), listOf(held(1, null, "Marcus"))),
        )
        // and nothing to do where there was never a single-identity device
        assertEquals(Identities.Migration.None, Identities.migrated(false, id(1), none))
        assertEquals(Identities.Migration.None, Identities.migrated(true, null, none))
        assertEquals(Identities.Migration.None, Identities.migrated(true, "nope", none))
    }

    @Test
    fun a_device_holding_any_identity_always_has_one_that_acts() {
        val two = listOf(held(1), held(2))
        assertEquals("the saved choice stands", id(2), Identities.active(two, id(2)))
        // **the chosen one taken off the device falls back** rather than
        // leaving a device that holds identities and can meet as none
        assertEquals(id(1), Identities.active(two, id(9)))
        assertEquals(id(1), Identities.active(two, null))
        assertNull("and nothing where nothing is held", Identities.active(listOf(), id(1)))
    }

    @Test
    fun the_list_names_each_one_and_marks_the_one_that_acts() {
        val two = listOf(held(1, null, "Marcus"), held(2))
        val rows = Identities.listed(two, id(2))
        assertEquals(2, rows.size)
        assertTrue("the alias is used where there is one: ${rows[0]}", rows[0].contains("Marcus"))
        // **never a name this device was not given** (PRD-14): the first
        // eight of the id where there is no alias
        assertTrue("and the id where there is not: ${rows[1]}", rows[1].contains(id(2).take(8)))
        assertFalse("the inactive one is unmarked: ${rows[0]}", rows[0].contains("●"))
        assertTrue("the active one is marked: ${rows[1]}", rows[1].contains("●"))
    }

    @Test
    fun the_selector_says_what_is_concurrent_and_what_is_not() {
        val said = Identities.words()
        assertTrue("says the subnets run at once: $said", said.contains("at the same time"))
        assertTrue("says a conversation stays put: $said", said.contains("stays with the identity"))
        assertTrue("and what the mark means: $said", said.contains("meeting"))
    }

    @Test
    fun only_the_two_acts_that_need_one_can_be_refused_for_want_of_one() {
        assertTrue(Identities.refusal(Identities.Act.MEETING)!!.contains("meet as"))
        assertTrue(Identities.refusal(Identities.Act.OPERATOR)!!.contains("act as"))
        for (act in listOf(
            Identities.Act.VIEWING,
            Identities.Act.WITHIN_SUBNET,
            Identities.Act.CONVERSATION,
        )) {
            assertNull("$act cannot be refused for want of an identity", Identities.refusal(act))
        }
    }
}
