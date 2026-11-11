package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **Taking an identity off the device** [author, 2026-11-07], and the one
 * thing a JVM test can hold about it that matters most: that the list of
 * what goes is complete.
 */
class WipeTest {

    @Test
    fun every_store_the_shell_writes_is_on_the_list() {
        // **a half-wiped device reads as clean**, so this is checked
        // against the sources rather than remembered: a store added
        // anywhere in the shell and not added to [Wipe.STORES] fails here
        val named = mutableSetOf<String>()
        val literal = Regex("""getSharedPreferences\(\s*"([A-Za-z._-]+)"""")
        val viaConst = Regex("""getSharedPreferences\(\s*([A-Za-z]+\.[A-Za-z_]+)""")
        val consts = mutableSetOf<String>()
        for (f in sources()) {
            val text = stripped(f.readText())
            literal.findAll(text).forEach { named += it.groupValues[1] }
            viaConst.findAll(text).forEach { consts += it.groupValues[1] }
        }
        assertTrue("no preference store was found at all — the regex has rotted", named.isNotEmpty())
        val missing = named - Wipe.STORES.toSet()
        assertTrue("stores the shell writes and a wipe would leave: $missing", missing.isEmpty())
        // the ones named through a constant are resolved by hand, and the
        // test says which so a new one cannot hide behind the indirection
        assertEquals(
            "a store is named through a constant this test does not know: $consts",
            setOf("Giving.NOTIFY_STORE", "Context.MODE_PRIVATE"),
            consts.filter { !it.startsWith("MODE_") }.toSet() + "Context.MODE_PRIVATE",
        )
        assertTrue(
            "the constant-named store is on the list",
            Wipe.STORES.contains(Giving.NOTIFY_STORE),
        )
    }

    @Test
    fun the_at_rest_alias_matches_the_shell_that_wraps_with_it() {
        // **two copies, tied by a test**, as `PaletteTest` does for the
        // dark set: a wipe deleting the wrong alias would leave the seeds
        // readable and report success
        val shell = sources().first { it.name == "AndroidShell.kt" }.readText()
        assertTrue(
            "AndroidShell's alias is not ${Wipe.AT_REST_ALIAS}",
            shell.contains("\"${Wipe.AT_REST_ALIAS}\""),
        )
    }

    @Test
    fun the_kernel_directory_is_the_one_the_shell_seals_into() {
        // **the tie is structural now** [2026-11-07]: the shell reads
        // `Wipe.KERNEL_DIR` rather than a literal of its own, which is
        // stronger than asserting the two strings match — they cannot
        // differ. What this holds is that the shell still takes it from
        // here, so a shell given a directory of its own would fail.
        val shell = sources().first { it.name == "AndroidShell.kt" }.readText()
        assertTrue(
            "the shell no longer takes its directory from Wipe.KERNEL_DIR",
            stripped(shell).contains("File(context.filesDir, Wipe.KERNEL_DIR)"),
        )
        assertEquals("and that directory is still the one a wipe clears", "kernel", Wipe.KERNEL_DIR)
    }

    @Test
    fun the_warning_says_what_goes_what_cannot_come_back_and_what_a_new_one_is() {
        val said = Wipe.words()
        assertEquals("three things, in that order", 3, said.size)
        assertTrue("names the keys: ${said[0]}", said[0].contains("their keys"))
        assertTrue("and the sealed captures: ${said[0]}", said[0].contains("capture sealed to them"))
        // **nothing is sent and nobody is told**: a wipe is local, and a
        // person must not read it as a departure
        assertTrue("says it is local: ${said[0]}", said[0].contains("Nothing is sent"))
        assertTrue("the phrase is the way back: ${said[1]}", said[1].contains("seed phrase"))
        assertTrue("and a backup for the history: ${said[1]}", said[1].contains("backup file"))
    }

    @Test
    fun the_new_identity_is_described_as_new_and_not_as_a_loss_of_status() {
        // **PRD-03's unlinked half, in the shape the entry asks for**: the
        // new key arrives as a genuinely-new user, vouched for afresh —
        // and NOT that the person loses the standing they hold, which is
        // a different claim and a false one
        val third = Wipe.words()[2]
        assertTrue("says it arrives new: $third", third.contains("genuinely-new user"))
        assertTrue("and is vouched for afresh: $third", third.contains("vouched for afresh"))
        assertTrue("and inherits nothing: $third", third.contains("does not inherit"))
        for (wrong in listOf("lose your", "lose the", "you will lose", "demoted", "downgrade")) {
            assertFalse("reads as a loss of status — `$wrong` in: $third", third.contains(wrong))
        }
    }

    @Test
    fun the_confirmation_names_the_act_rather_than_asking_again() {
        val c = Wipe.confirm()
        // **the door takes the device, not one identity** [author,
        // 2026-11-11]: a device can hold several, and taking one off is
        // its own control on the Options screen ([Identities.removable])
        assertTrue("names the act: $c", c.contains("Wipe this device"))
        assertFalse("still reads as one identity: $c", c.contains("this identity"))
        assertTrue("and the way back: $c", c.contains("phrase"))
        assertFalse("does not ask `are you sure`: $c", c.lowercase().contains("are you sure"))
    }

    @Test
    fun a_wipe_that_could_not_finish_says_so_and_a_clean_one_says_nothing() {
        assertNull("nothing to say when it all went", Wipe.left(listOf()))
        val said = Wipe.left(listOf("kernel", "backup"))!!
        assertTrue("names what stayed: $said", said.contains("kernel") && said.contains("backup"))
        assertTrue("does not call it clean: $said", said.contains("not\nclean") || said.contains("not clean"))
        assertTrue("and says what to do: $said", said.contains("uninstall"))
    }

    /** Every Kotlin source of the shell, for the reading tests above. */
    private fun sources(): List<java.io.File> =
        java.io.File(module(), "app/src/main/kotlin/com/comptus/fueros")
            .listFiles { f -> f.name.endsWith(".kt") }!!
            .sortedBy { it.name }

    /**
     * The source with its comments removed. **A comment naming a store is
     * not a store**, and the sweep above must not be satisfied — or
     * tripped — by prose, which is the mistake an earlier reading test
     * made [2026-11-05].
     */
    private fun stripped(text: String): String = text
        .replace(Regex("""/\*[\s\S]*?\*/"""), " ")
        .lines()
        .joinToString("\n") { it.substringBefore("//") }

    /** Where the tree is, for the source-reading tests above. */
    private fun module(): java.io.File {
        var at: java.io.File? = java.io.File(".").canonicalFile
        while (at != null) {
            if (java.io.File(at, "app/src/main/AndroidManifest.xml").isFile) return at
            at = at.parentFile
        }
        throw AssertionError("no app/src/main/AndroidManifest.xml at or above here")
    }
}
