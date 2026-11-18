package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TypeTest {

    private val brand = File("../../../brand")

    /** **The two faces are the brand's files**, read from where the desktop
     *  reads them, and the theme wears the sans [author, 2026-11-18]. */
    @Test
    fun the_shell_draws_in_the_brands_two_faces() {
        for (face in listOf("geist", "geist_italic", "geist_mono", "geist_mono_italic")) {
            assertTrue("$face is in the brand", File(brand, "res/font/$face.ttf").isFile)
        }
        assertTrue("the licence travels with them", File(brand, "OFL.txt").isFile)
        val gradle = File("build.gradle.kts").readText()
        assertTrue(gradle.contains("res.srcDir(\"../../../brand/res\")"))
        val theme = File("src/main/res/values/themes.xml").readText()
        assertTrue(theme.contains("<item name=\"android:fontFamily\">@font/geist</item>"))
        assertEquals("Geist", Tokens.SANS)
        assertEquals("Geist Mono", Tokens.MONO)
    }

    /** **No screen reaches past them** for the platform's own bold or mono. */
    @Test
    fun no_screen_draws_in_the_platforms_own_faces() {
        val screens = File("src/main/kotlin/com/comptus/fueros").listFiles()!!
            .filter { it.name.endsWith(".kt") && it.name != "Type.kt" }
        for (f in screens) {
            val text = f.readText()
            for (own in listOf("Typeface.MONOSPACE", "Typeface.DEFAULT_BOLD", "Typeface.SANS_SERIF")) {
                assertFalse("${f.name} draws in $own", text.contains(own))
            }
        }
        val app = File("src/main/kotlin/com/comptus/fueros/FuerosApp.kt").readText()
        assertTrue("loaded before anything draws", app.contains("Type.load(this)"))
    }
}
