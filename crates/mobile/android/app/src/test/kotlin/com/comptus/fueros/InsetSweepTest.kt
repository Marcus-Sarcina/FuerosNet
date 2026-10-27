package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **Every screen is on the one mechanism, and none is left on the old
 * one** [2026-10-27].
 *
 * `fitsSystemWindows` on the same view as a screen's padding is what
 * erased that padding and dropped the bottom inset inside the scrolling
 * content, so the foot of every scrolling screen sat under the navigation
 * bar. The repair is uniform, which means the way it regresses is uniform
 * too: a new screen written from an old one as a template, carrying the
 * attribute back in, or forgetting to inset its frame at all.
 *
 * A JVM test cannot lay out a view, so this reads the sources, as
 * [ManifestTest] does. It says nothing about whether the insets are
 * correct on a device. It says every screen asks for them the one way,
 * which is the thing that was uneven.
 */
class InsetSweepTest {

    private fun module(): File {
        var at: File? = File(".").canonicalFile
        while (at != null) {
            if (File(at, "app/src/main/AndroidManifest.xml").isFile) return at
            at = at.parentFile
        }
        throw AssertionError("no app/src/main/AndroidManifest.xml at or above ${File(".").canonicalPath}")
    }

    private fun activities(root: File): List<File> {
        val found = File(root, "app/src/main/kotlin").walkTopDown()
            .filter { it.name.endsWith("Activity.kt") }
            .toList()
        assertTrue("no Activity sources under $root", found.size >= 8)
        return found
    }

    @Test
    fun no_screen_sets_fits_system_windows() {
        val wrong = activities(module())
            .filter { Regex("""fitsSystemWindows\s*=\s*true""").containsMatchIn(it.readText()) }
            .map { it.name }
        assertTrue(
            "these set fitsSystemWindows, which REPLACES a view's padding with the insets " +
                "rather than adding to it: $wrong",
            wrong.isEmpty(),
        )
    }

    @Test
    fun every_screen_insets_its_own_frame() {
        val without = activities(module())
            .filter { !it.readText().contains("Screen.insetting(") }
            .map { it.name }
        assertTrue(
            "these draw a screen and never hold it clear of the system bars: $without",
            without.isEmpty(),
        )
    }

    /** And the mechanism itself lives in one place: a second hand-rolled
     *  listener is how the seven came to differ from the one. */
    @Test
    fun the_listener_is_written_once() {
        val root = module()
        val copies = File(root, "app/src/main/kotlin").walkTopDown()
            .filter { it.extension == "kt" }
            .filter { it.readText().contains("setOnApplyWindowInsetsListener") }
            .map { it.name }
            .toList()
        assertTrue("the insets are handled in more than one place: $copies", copies == listOf("Screen.kt"))
    }
}
