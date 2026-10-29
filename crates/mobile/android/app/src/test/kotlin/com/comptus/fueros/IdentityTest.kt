package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test

/** The phrase round-trips, refuses with a reason, and derives the same seeds. */
class IdentityTest {
    /** A list of the right size that is nobody's standard, for the arithmetic. */
    private val synthetic: List<String> = (0 until Identity.LIST).map { "w$it" }

    /** The standard's list where the asset is present; the vector test needs it. */
    private fun english(): List<String>? {
        var at: File? = File(".").canonicalFile
        while (at != null) {
            val f = File(at, "app/src/main/assets/bip39/english.txt")
            if (f.isFile) return Identity.wordlist(f.readLines().asSequence())
            at = at.parentFile
        }
        return null
    }

    @Test
    fun the_words_read_back_to_the_bytes_they_came_from() {
        val entropy = ByteArray(32) { (it * 37 + 11).toByte() }
        val words = Identity.phrase(entropy, synthetic)
        assertEquals(24, words.size)
        val back = Identity.entropy(words, synthetic)
        assertTrue(back is Identity.Read.Entropy)
        assertArrayEquals(entropy, (back as Identity.Read.Entropy).bytes)
    }

    @Test
    fun a_fault_is_named_by_where_it_is() {
        val words = Identity.phrase(ByteArray(32), synthetic)
        val short = Identity.entropy(words.dropLast(1), synthetic) as Identity.Read.Refused
        assertTrue(short.why.contains("23"))
        val unknown = Identity.entropy(words.toMutableList().also { it[4] = "nope" }, synthetic) as Identity.Read.Refused
        assertTrue(unknown.why, unknown.why.contains("5: nope"))
        // two words swapped: the bits change and the checksum almost
        // always fails; where they are the same word nothing is swapped
        val swapped = words.toMutableList().also { val t = it[0]; it[0] = it[1]; it[1] = t }
        if (swapped[0] != swapped[1]) {
            assertTrue(Identity.entropy(swapped, synthetic) is Identity.Read.Refused)
        }
    }

    @Test
    fun the_standards_own_vector_for_all_zero_entropy() {
        val list = english()
        assumeTrue("the wordlist asset is not present", list != null)
        val words = Identity.phrase(ByteArray(32), list!!)
        assertEquals(List(23) { "abandon" } + "art", words)
        assertEquals(2048, list.size)
    }

    @Test
    fun the_seeds_are_sixty_four_bytes_and_deterministic() {
        val master = ByteArray(32) { it.toByte() }
        val a = Identity.seeds(master)
        assertEquals(64, a.size)
        assertArrayEquals(a, Identity.seeds(master.copyOf()))
        // the two halves are not the same bytes
        assertTrue(!a.copyOfRange(0, 32).contentEquals(a.copyOfRange(32, 64)))
        // a different master is a different identity
        assertTrue(!a.contentEquals(Identity.seeds(ByteArray(32))))
    }

    @Test
    fun an_alias_is_required_and_bounded() {
        assertTrue(Identity.aliasRefusal("  ") != null)
        assertNull(Identity.aliasRefusal("Dana"))
        assertTrue(Identity.aliasRefusal("é".repeat(33))!!.contains("66"))
    }

    @Test
    fun the_layout_is_numbered_three_to_a_line() {
        val laid = Identity.laidOut((1..24).map { "w$it" })
        assertEquals(8, laid.lines().size)
        assertTrue(laid.startsWith(" 1 w1"))
    }
}
