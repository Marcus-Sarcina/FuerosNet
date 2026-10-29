package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Test

/** The wording of the shell's content where it says a number. */
class ShellTest {
    private fun standing(n: Int) = uniffi.rhtn_model.Rank(root = false, operator = false, subordinates = n.toUInt())

    @Test
    fun the_downline_is_the_direct_count_and_agrees_in_number() {
        assertEquals("Your standing here has not been read yet.", Shell.downline(null))
        assertEquals("No line of yours is issued here.", Shell.downline(standing(0)))
        assertEquals("Your downline here: 1 direct line.", Shell.downline(standing(1)))
        assertEquals("Your downline here: 4 direct lines.", Shell.downline(standing(4)))
    }
}
