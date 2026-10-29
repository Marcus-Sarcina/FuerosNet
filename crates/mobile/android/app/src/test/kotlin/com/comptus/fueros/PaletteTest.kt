package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **Every colour that carries text clears the floor** (the screens
 * sheet's G; WCAG 2.2 AA at 4.5:1 for body text).
 *
 * This is the test the app most needed and did not have. Drawing in the
 * platform's defaults put body text at **4.0:1** and everything set to
 * `Color.GRAY` at **3.1:1**, both under the floor, and nothing said so —
 * the contrast was measured once by sampling a photograph of a running
 * phone, which is not a thing that happens twice.
 *
 * It also found the one token in the drawing that cannot carry text, and
 * that there is no replacement for it: see [a_third_grey_does_not_exist].
 */
class PaletteTest {

    @Test
    fun every_pair_the_shell_draws_clears_the_floor() {
        for ((mode, set) in listOf("dark" to Palette.DARK, "light" to Palette.LIGHT)) {
            for ((inkName, surfaceName) in Palette.DRAWN) {
                val r = Palette.contrast(Palette.of(set, inkName), Palette.of(set, surfaceName))
                assertTrue(
                    "$mode: $inkName on $surfaceName is ${"%.2f".format(r)}:1, " +
                        "under ${Palette.FLOOR}",
                    r >= Palette.FLOOR,
                )
            }
        }
    }

    /**
     * **And the pair that cannot be drawn is not declared.** `sub` is the
     * dimmest text the floor allows on the ground, so a surface raised
     * above the ground is already too close to it: `sub` on `control` is
     * 3.44:1, and the value that would fix it is darker than the ground.
     * The finding is that the palette needs declared pairs rather than a
     * free cross-product.
     */
    @Test
    fun sub_is_a_ground_ink_and_the_declaration_does_not_pretend_otherwise() {
        val r = Palette.contrast(Palette.DARK.sub, Palette.DARK.control)
        assertTrue(
            "if this now passes the finding is stale: ${"%.2f".format(r)}:1",
            r < Palette.FLOOR,
        )
        assertFalse(
            "a pair that cannot read must not be declared drawable",
            Palette.DRAWN.contains("sub" to "control"),
        )
        // the amber is the same shape of finding, less absolutely: 4.35:1
        // on a control, and the drawing never puts it there
        assertTrue(Palette.contrast(Palette.DARK.warn, Palette.DARK.control) < Palette.FLOOR)
        assertFalse(Palette.DRAWN.contains("warn" to "control"))
        // every declared name resolves, so a typo is a failure and not a
        // silently skipped check
        for ((a, b) in Palette.DRAWN) {
            Palette.of(Palette.DARK, a)
            Palette.of(Palette.DARK, b)
        }
    }

    @Test
    fun the_accent_can_be_read_where_it_is_a_filled_button() {
        for ((mode, set) in listOf("dark" to Palette.DARK, "light" to Palette.LIGHT)) {
            val r = Palette.contrast(set.onAccent, set.accent)
            assertTrue("$mode: ${"%.2f".format(r)}:1 on the accent", r >= Palette.FLOOR)
        }
    }

    /**
     * **The rail is the mark, and its own ink reads on it.** The russet
     * field and the crowns are not themed, so they are checked once
     * rather than per mode.
     */
    @Test
    fun the_rails_ink_and_its_crowns_read_on_the_russet() {
        assertTrue(Palette.contrast(Palette.RAIL_INK, Palette.RAIL) >= Palette.FLOOR)
        assertTrue(Palette.contrast(Palette.RAIL_INK, Palette.RAIL_ORB) >= Palette.FLOOR)
        assertTrue(
            "a count must be readable on its own ground",
            Palette.contrast(Palette.RAIL_INK, Palette.COUNT) >= Palette.FLOOR,
        )
        // a crown is a graphic rather than text, so 3:1 is its floor
        for (c in listOf(Palette.GOLD, Palette.SILVER)) {
            assertTrue(
                "a crown must be distinguishable from the field it sits on",
                Palette.contrast(c, Palette.RAIL) >= 3.0,
            )
        }
    }

    /**
     * **There is no third grey**, which is a finding rather than a
     * choice. The artboards use `#6E757E` for timestamps and small notes
     * at 11px; it is 3.55:1 on the dark ground, and every value between
     * it and `sub` is also under the floor. So the drawing's three greys
     * are two for text, and `#6E757E` is kept only for what says
     * nothing.
     */
    @Test
    fun a_third_grey_does_not_exist() {
        val onGround = Palette.contrast(Palette.RULE_ONLY, Palette.DARK.ground)
        assertTrue(
            "if this now passes, the finding is stale and the token can carry text: " +
                "${"%.2f".format(onGround)}:1",
            onGround < Palette.FLOOR,
        )
        // and `sub` is the floor of that family rather than a free choice
        assertTrue(Palette.contrast(Palette.DARK.sub, Palette.DARK.ground) >= Palette.FLOOR)
        assertTrue(
            "sub is close to the floor, so it is the dimmest text there is",
            Palette.contrast(Palette.DARK.sub, Palette.DARK.ground) < 5.0,
        )
    }

    /** The arithmetic itself, against values anyone can check. */
    @Test
    fun the_contrast_arithmetic_is_wcags() {
        val white = 0xFFFFFFFF.toInt()
        val black = 0xFF000000.toInt()
        assertEquals(21.0, Palette.contrast(white, black), 0.01)
        assertEquals(1.0, Palette.contrast(white, white), 0.001)
        assertEquals(
            "and it does not care which way round it is asked",
            Palette.contrast(white, black),
            Palette.contrast(black, white),
            0.0001,
        )
        assertEquals(0.0, Palette.luminance(black), 0.0001)
        assertEquals(1.0, Palette.luminance(white), 0.0001)
    }

    @Test
    fun dark_is_what_a_person_meets_first() {
        assertEquals(Palette.Mode.DARK, Palette.mode)
        assertEquals(Palette.DARK, Palette.now)
    }

    // ---- the theme, which is the same colours in the other language ----

    private fun module(): java.io.File {
        var at: java.io.File? = java.io.File(".").canonicalFile
        while (at != null) {
            if (java.io.File(at, "app/src/main/AndroidManifest.xml").isFile) return at
            at = at.parentFile
        }
        throw AssertionError("no app/src/main/AndroidManifest.xml at or above here")
    }

    private fun res(name: String): String {
        val f = java.io.File(module(), "app/src/main/res/values/$name")
        assertTrue("no $name", f.isFile)
        return f.readText()
    }

    /**
     * **Two copies of the dark set, tied here.** A theme is what restyles
     * thirteen Activities without editing thirteen Activities, and a
     * theme's values must be resources; [Palette] is what a screen reads
     * a colour from. So they are both real, and this is what stops them
     * drifting apart silently.
     */
    @Test
    fun the_theme_names_the_same_colours_the_palette_holds() {
        val xml = res("colors.xml")
        val named = Regex("""<color name="(\w+)">#([0-9A-Fa-f]{8})</color>""")
            .findAll(xml)
            .associate { it.groupValues[1] to it.groupValues[2].uppercase() }
        assertTrue("no colours parsed out of colors.xml", named.size >= 10)
        for ((name, hex) in named) {
            val mine = Palette.of(Palette.DARK, name)
            assertEquals(
                "colors.xml and Palette.DARK disagree about $name",
                "%08X".format(mine),
                hex,
            )
        }
        // and every ink and surface the shell draws with is in the theme
        for (needed in listOf("ground", "chrome", "sunken", "control", "ink", "muted", "sub")) {
            assertTrue("the theme does not name $needed", named.containsKey(needed))
        }
    }

    /** And the theme is what every screen wears, rather than each one
     *  asking for it. */
    @Test
    fun the_application_wears_it_so_no_screen_has_to_ask() {
        val manifest = java.io.File(module(), "app/src/main/AndroidManifest.xml").readText()
        assertTrue(
            "the application does not carry the theme, so a screen gets the platform's",
            manifest.contains("""android:theme="@style/Fueros""""),
        )
        val theme = res("themes.xml")
        for (item in listOf(
            "android:windowNoTitle",
            "android:windowBackground",
            "android:textColor",
            "android:statusBarColor",
            "android:navigationBarColor",
        )) {
            assertTrue("the theme does not set $item", theme.contains(item))
        }
    }

    /**
     * **No screen reaches for a platform grey.** `Color.GRAY` at 3.1:1
     * and a `TextView`'s own ink at 4.0:1 were what the app drew in, and
     * nothing said so. The QR codes keep black and white: a symbol is
     * read by a camera and has no palette.
     */
    @Test
    fun no_screen_draws_in_the_platforms_own_colours() {
        val sources = java.io.File(module(), "app/src/main/kotlin").walkTopDown()
            .filter { it.extension == "kt" }
            .toList()
        assertTrue("no sources found", sources.size > 10)
        val greyed = sources
            .filter { it.name != "Palette.kt" }
            .filter { Regex("""Color\.(GRAY|DKGRAY|LTGRAY)""").containsMatchIn(it.readText()) }
            .map { it.name }
        assertTrue("these draw in a platform grey: $greyed", greyed.isEmpty())

        // black and white survive only where a camera reads the result
        val monochrome = sources
            .filter { Regex("""Color\.(BLACK|WHITE)""").containsMatchIn(it.readText()) }
            .map { it.name }
            .sorted()
        assertEquals(
            "black and white belong to the symbologies and nowhere else",
            listOf("MeetActivity.kt", "Tracking.kt"),
            monochrome,
        )
    }
}
