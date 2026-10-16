package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * What the People screen shows for a contact
 * (`light-client-requirements.md` §4.3): the published alias, or the
 * keyhash where none has reached this device.
 */
class PeopleTest {
    private fun person(alias: String?, located: Boolean = true, how: String = "met") =
        Kernel.Person(
            who = "ab12cd34",
            alias = alias,
            how = how,
            located = located,
            addressed = false,
            count = 3UL,
            lastAt = 0UL,
        )

    @Test
    fun a_published_alias_is_shown_and_a_keyhash_stands_in_where_none_came() {
        assertTrue(People.line(person("Dana")).startsWith("Dana  met  ×3"))
        assertTrue(People.line(person(null)).startsWith("ab12cd34  met  ×3"))
    }

    @Test
    fun the_marks_say_what_is_held_and_nothing_more() {
        assertEquals("Dana  met  ×3 — placed", People.line(person("Dana")))
        assertEquals("Dana  met  ×3", People.line(person("Dana", located = false)))
        assertEquals(
            "Dana  a node  ×3 — placed, answers somewhere",
            People.line(person("Dana", how = "a node")),
        )
    }

    @Test
    fun an_empty_roll_says_so() {
        assertEquals("nobody yet", People.roll(emptyList()))
        assertEquals(
            "Dana  met  ×3 — placed\nab12cd34  met  ×3 — placed",
            People.roll(listOf(person("Dana"), person(null))),
        )
    }
}
