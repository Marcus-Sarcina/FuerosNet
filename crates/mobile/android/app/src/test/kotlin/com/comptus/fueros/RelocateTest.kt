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
        assertEquals("Nothing changes.", Relocate.found(true, Standing.FAILED))
        assertEquals("Nothing changes.", Relocate.found(true, null))
    }

    // functional: UX-033
    @Test
    fun the_orb_and_the_subnet_page_consult_the_move() {
        val shell = File("src/main/kotlin/com/comptus/fueros/ShellActivity.kt").readText()
        assertTrue(
            "the subnet's orb is not marked by its move",
            shell.contains("unseen = if (Relocate.blocks(o.row.relocation)) 1 else 0"),
        )
        assertTrue(
            "the subnet's page is not put behind the question",
            shell.contains("scope?.relocation?.takeIf { Relocate.blocks(it) }"),
        )
        assertTrue(
            "the question can be dismissed",
            shell.contains("AlertDialog.Builder(this).setCancelable(false)"),
        )
    }
}
