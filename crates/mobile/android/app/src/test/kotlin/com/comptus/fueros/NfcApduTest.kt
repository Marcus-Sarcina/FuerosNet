package com.comptus.fueros

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Both sides of the tap, held on the JVM: the radio is the only part
 *  missing, and the radio adds distance, not different bytes. */
class NfcApduTest {

    private val cid = ByteArray(32) { (it * 7).toByte() }
    private val other = ByteArray(32) { (it * 11 + 1).toByte() }

    @Test
    fun a_tap_in_one_ceremony_passes_both_sides() {
        val (selected, pass0) = NfcApdu.respond(NfcApdu.select(), cid)
        assertArrayEquals(byteArrayOf(0x90.toByte(), 0x00), selected)
        assertFalse("selecting is not passing", pass0)
        val (resp, cardPassed) = NfcApdu.respond(NfcApdu.exchange(cid), cid)
        assertTrue("the card passed", cardPassed)
        assertTrue("and the reader did", NfcApdu.passed(resp, cid))
    }

    @Test
    fun a_tap_across_two_ceremonies_passes_neither_side() {
        val (resp, cardPassed) = NfcApdu.respond(NfcApdu.exchange(other), cid)
        assertFalse(cardPassed)
        assertFalse(NfcApdu.passed(resp, cid))
        // and the refusal carries no ceremony-id for the stranger to keep
        assertArrayEquals(byteArrayOf(0x69, 0x85.toByte()), resp)
    }

    @Test
    fun a_phone_with_no_ceremony_answers_unknown_and_leaks_nothing() {
        val (resp, passed) = NfcApdu.respond(NfcApdu.exchange(cid), null)
        assertFalse(passed)
        assertArrayEquals(byteArrayOf(0x6D, 0x00), resp)
    }

    @Test
    fun junk_is_answered_unknown_rather_than_thrown_at() {
        val (resp, passed) = NfcApdu.respond(byteArrayOf(0x00), cid)
        assertFalse(passed)
        assertArrayEquals(byteArrayOf(0x6D, 0x00), resp)
        // a response of the wrong shape is not a pass, whatever it says
        assertFalse(NfcApdu.passed(ByteArray(0), cid))
        assertFalse(NfcApdu.passed(cid, cid))
    }
}
