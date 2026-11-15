package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **The shell's side of bringing another device to the dataset**
 * ([Pairing]; APP-017): the roles decide the radio's sides, the steps run
 * in one order, and the outcome is said in counts.
 */
class PairingTest {

    // functional: APP-017
    @Test
    fun the_handing_device_offers_the_link_and_the_taking_device_seeks_it() {
        assertTrue("the handing device advertises", Pairing.offers(Pairing.Role.HAND))
        assertFalse("the taking device scans", Pairing.offers(Pairing.Role.TAKE))
    }

    // functional: APP-017
    @Test
    fun the_steps_run_role_codes_link_carriage_done_and_the_ends_stay() {
        var s = Pairing.Step.ROLE
        val seen = mutableListOf(s)
        repeat(4) { s = Pairing.next(s); seen.add(s) }
        assertEquals(
            listOf(Pairing.Step.ROLE, Pairing.Step.CODES, Pairing.Step.LINK, Pairing.Step.CARRIAGE, Pairing.Step.DONE),
            seen,
        )
        assertEquals(Pairing.Step.DONE, Pairing.next(Pairing.Step.DONE))
        assertEquals(Pairing.Step.STOPPED, Pairing.next(Pairing.Step.STOPPED))
        for (step in Pairing.Step.values()) {
            for (role in listOf(null, Pairing.Role.HAND, Pairing.Role.TAKE)) {
                assertTrue("every step says something", Pairing.narration(step, role).isNotBlank())
            }
        }
    }

    // functional: APP-017
    @Test
    fun the_outcome_is_said_in_counts_and_names_what_was_left_out() {
        assertEquals("Taken: 3 records, 1 contact, 0 other items.", Pairing.taken(3uL, 1uL, 0uL, 0uL))
        assertEquals(
            "Taken: 1 record, 2 contacts, 4 other items. 1 record did not verify or did not chain and was left out.",
            Pairing.taken(1uL, 2uL, 4uL, 1uL),
        )
    }
}
