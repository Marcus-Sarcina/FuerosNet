package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.rhtn_model.Relocation
import uniffi.rhtn_model.Standing

/**
 * **A patron's server said to have moved, as the person is asked**
 * ([Relocate]; `light-client-requirements.md` §4) [author, 2026-11-16].
 *
 * The words and the blocking rule are [Relocate]'s and asserted directly;
 * that the subnet's page and orb consult them is read out of the source,
 * as [ResourcePageTest] reads its frame's.
 */
class RelocateTest {

    private fun moved(standing: Standing) = Relocation(
        node = ByteArray(32),
        from = listOf("64.227.31.70:7431"),
        to = "167.172.146.119:7431",
        standing = standing,
        why = null,
        label = null,
    )

    // functional: UX-033
    @Test
    fun the_question_is_the_authors_words_with_the_addresses_as_ips() {
        assertEquals(
            "Mirror has relocated their server for Mirror's Manse from 64.227.31.70 to 167.172.146.119.\n" +
                "Do you accept the server at 167.172.146.119 as your new home for Mirror's Manse?",
            Relocate.question("Mirror", "Manse", "64.227.31.70:7431", "167.172.146.119:7431"),
        )
        assertEquals("Accept" to "Do not connect", Relocate.ANSWERS)
        assertEquals("10.0.0.1", Relocate.host("10.0.0.1"))
    }

    // functional: UX-033
    @Test
    fun only_a_move_the_person_must_answer_marks_the_orb_and_blocks_the_page() {
        assertFalse("no move", Relocate.blocks(null))
        assertFalse("nothing has answered there yet", Relocate.blocks(moved(Standing.UNPROVED)))
        assertFalse("a failed check is the operator's, not the person's", Relocate.blocks(moved(Standing.FAILED)))
        // declined still asks: the notice does not clear
        for (s in listOf(Standing.PROVED, Standing.DECLINED)) {
            assertTrue("$s blocks", Relocate.blocks(moved(s)))
        }
    }

    // functional: UX-033
    @Test
    fun the_question_names_the_owner_and_never_a_role() {
        assertEquals("Mirror", Relocate.owner("Mirror", "GXV", "703a82c2ffff"))
        assertEquals("GXV", Relocate.owner(null, "GXV", "703a82c2ffff"))
        assertEquals("GXV", Relocate.owner(" ", "GXV", "703a82c2ffff"))
        assertEquals("703a82c2…", Relocate.owner(null, null, "703a82c2ffff"))
        val shell = File("src/main/kotlin/com/comptus/fueros/ShellActivity.kt").readText()
        assertFalse("a role stands in for a name", shell.contains("\"Your patron\""))
    }

    // functional: UX-033
    @Test
    fun the_orb_and_the_subnet_page_consult_the_move() {
        val shell = File("src/main/kotlin/com/comptus/fueros/ShellActivity.kt").readText()
        assertTrue(
            "the subnet's orb is not marked by its move",
            shell.contains("unseen = Chrome.unseen(Relocate.blocks(o.row.relocation), launchersOf(o), unreadOn(o))"),
        )
        assertTrue(
            "the subnet's page is not put behind the question",
            shell.contains("scope?.relocation?.takeIf { Relocate.blocks(it) }"),
        )
        assertTrue(
            "the question can be dismissed",
            shell.contains("AlertDialog.Builder(this).setCancelable(false)"),
        )
        // the page is drawn more than once; the question is put once, and
        // put again only from a model that already holds the answer
        assertTrue("a second drawing stacks a second question", shell.contains("if (asking) return"))
        assertFalse(
            "cleared as the dialog closes, the page is drawn from the copy before the answer",
            shell.contains(".setOnDismissListener { asking = false }"),
        )
        assertEquals(
            "both answers are drawn from a model read after them",
            2,
            Regex("""Model\.refreshHere\([^)]*\)\)\s*runOnUiThread \{ asking = false;""").findAll(shell).count(),
        )
        assertTrue(
            "a question posted after the person left its page is put anyway",
            shell.contains("if (here !is Chrome.Place.Subnet || here.id != subnet.id.hex()) return"),
        )
    }

    // functional: UX-033
    @Test
    fun a_first_invitation_is_asked_as_a_move_from_nowhere() {
        assertEquals(
            "Mirror has invited you to their server for Mirror's Manse at 167.172.146.119.\n" +
                "Do you accept the server at 167.172.146.119 as your home for Mirror's Manse?",
            Relocate.question("Mirror", "Manse", "", "167.172.146.119:7431"),
        )
    }

    // functional: UX-033
    @Test
    fun an_accepted_server_that_does_not_answer_is_tried_again() {
        val attach = File("src/main/kotlin/com/comptus/fueros/Attachments.kt").readText()
            .substringAfter("fun attachTo(").substringBefore("\n    }\n")
        assertTrue(attach.contains("reattachLater(by, p, scope, 1)"))
        val shell = File("src/main/kotlin/com/comptus/fueros/ShellActivity.kt").readText()
        assertTrue("accepting attaches through it", shell.contains("Attachments.attachTo(by, nodeHex, to)"))
    }

    // functional: UX-033
    @Test
    fun an_invitation_shows_nothing() {
        val invite = File("src/main/kotlin/com/comptus/fueros/InviteActivity.kt").readText()
        assertFalse("an invitation draws a page", invite.contains("setContentView"))
        assertTrue("an invitation opens something", !invite.contains("startActivity"))
        assertTrue("the window goes before anything is read", invite.indexOf("finish()") < invite.indexOf("Thread("))
        val manifest = File("src/main/AndroidManifest.xml").readText()
        assertTrue(manifest.contains("android:name=\".InviteActivity\" android:exported=\"true\"\n            android:theme=\"@style/Fueros.Quiet\""))
    }
}
