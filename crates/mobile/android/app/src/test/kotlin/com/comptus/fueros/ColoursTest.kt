package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **Each identity has a colour** [author, 2026-11-12]: chosen on Options,
 * a new identity taking the first unused; what the focus changes wears
 * the focused identity's, each subnet orb its holder's, each thread the
 * identity it is held as.
 */
class ColoursTest {
    private fun id(n: Int) = "%064x".format(n)
    private fun held(vararg n: Int) = n.map { Identities.Held(id(it), "s$it", null) }

    @Test
    fun a_new_identity_takes_the_first_colour_no_identity_uses() {
        val one = Identities.assigned(emptyMap(), held(1))
        assertEquals(Palette.IDENTITY[0].second, one[id(1)])
        val two = Identities.assigned(one, held(1, 2))
        assertEquals(Palette.IDENTITY[0].second, two[id(1)])
        assertEquals(Palette.IDENTITY[1].second, two[id(2)])
        // a chosen colour is kept, and the next new identity skips it
        val chosen = mapOf(id(1) to Palette.IDENTITY[2].second)
        val three = Identities.assigned(chosen, held(1, 2, 3))
        assertEquals(Palette.IDENTITY[2].second, three[id(1)])
        assertEquals(Palette.IDENTITY[0].second, three[id(2)])
        assertEquals(Palette.IDENTITY[1].second, three[id(3)])
        // more identities than colours: nothing is left uncoloured
        val many = Identities.assigned(emptyMap(), held(*(1..10).toList().toIntArray()))
        assertEquals(10, many.size)
    }

    @Test
    fun the_colours_file_round_trips_and_skips_what_is_not_a_line_of_it() {
        val map = mapOf(id(1) to Palette.IDENTITY[0].second, id(2) to Palette.IDENTITY[3].second)
        assertEquals(map, Identities.colours(Identities.encodeColours(map)))
        assertEquals(emptyMap<String, Int>(), Identities.colours(null))
        assertEquals(emptyMap<String, Int>(), Identities.colours("not a line\n${id(1)} zz\n".toByteArray()))
        assertEquals(Palette.RAIL_ORB, Identities.colourOf(map, id(9)))
        assertEquals("Blue", Identities.colourName(Palette.IDENTITY[3].second))
    }

    @Test
    fun every_colour_carries_the_rail_ink() {
        for ((name, c) in Palette.IDENTITY) {
            assertTrue("$name is under the floor for the ink", Palette.contrast(c, Palette.RAIL_INK) >= Palette.FLOOR)
        }
        assertEquals(Palette.IDENTITY.size, Palette.IDENTITY.map { it.second }.toSet().size)
    }

    @Test
    fun what_the_focus_changes_wears_its_colour() {
        val shell = source("ShellActivity.kt")
        assertTrue("the meeting orb is not the focused identity's colour", shell.contains("is Chrome.Rung.Meet -> focusColour()"))
        assertTrue("a subnet orb is not its holder's colour", shell.contains("is Chrome.Rung.Subnet -> Identities.colourOf(colours, rung.by)"))
        assertEquals("the nameplate and the options orb do not wear the focused colour", 2, Regex("""Crowns\.rounded\(focusColour\(\)""").findAll(shell).count())
        assertTrue("Options offers no colour", shell.contains("row(\"Colour\", Identities.colourName(focusColour())) { chooseColour() }"))
        val list = source("ConversationsActivity.kt")
        assertTrue("a thread is not coloured by the identity it is held as", list.contains("Identities.colourOf(colours, t.by)"))
    }

    private fun source(name: String): String {
        val f = java.io.File(module(), "app/src/main/kotlin/com/comptus/fueros/$name")
        return f.readText()
            .replace(Regex("""/\*[\s\S]*?\*/"""), " ")
            .lines()
            .joinToString("\n") { it.substringBefore("//") }
    }

    private fun module(): java.io.File {
        var at: java.io.File? = java.io.File(".").canonicalFile
        while (at != null) {
            if (java.io.File(at, "app/src/main/kotlin/com/comptus/fueros/Kernel.kt").exists()) return at
            at = at.parentFile
        }
        error("the android module was not found above ${java.io.File(".").canonicalFile}")
    }
}
