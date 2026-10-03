package com.comptus.fueros

import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The one piece of the Bluetooth bearer a JVM can hold: the subscription
 * state both sides' `send` is gated on. The radio itself is not run here
 * and the class under test touches no Android type, so what this covers is
 * the reading of a Client Characteristic Configuration write on the server
 * and of the write's confirmation on the client, and that nothing is ready
 * before either.
 */
class BleBearerTest {

    private val other: UUID = UUID.fromString("00002901-0000-1000-8000-00805f9b34fb")

    @Test
    fun nothing_is_ready_before_a_subscription() {
        assertFalse(BleBearer.Subscription().active)
    }

    @Test
    fun the_server_is_ready_when_the_client_asks_for_notifications() {
        val s = BleBearer.Subscription()
        assertEquals(true, s.requested(BleBearer.CCCD, byteArrayOf(0x01, 0x00)))
        assertTrue(s.active)
        // notify and indicate both set still asks for notifications
        assertEquals(true, s.requested(BleBearer.CCCD, byteArrayOf(0x03, 0x00)))
        assertTrue(s.active)
    }

    @Test
    fun a_client_that_withdraws_or_asks_only_to_be_indicated_is_not_subscribed() {
        val s = BleBearer.Subscription()
        s.requested(BleBearer.CCCD, byteArrayOf(0x01, 0x00))
        assertEquals(false, s.requested(BleBearer.CCCD, byteArrayOf(0x00, 0x00)))
        assertFalse(s.active)
        assertEquals(false, s.requested(BleBearer.CCCD, byteArrayOf(0x02, 0x00)))
        assertFalse(s.active)
    }

    @Test
    fun a_write_that_is_not_a_cccd_value_is_not_a_subscription_and_changes_nothing() {
        val s = BleBearer.Subscription()
        assertNull(s.requested(other, byteArrayOf(0x01, 0x00)))
        assertFalse(s.active)
        assertNull(s.requested(BleBearer.CCCD, null))
        assertNull(s.requested(BleBearer.CCCD, byteArrayOf(0x01)))
        assertNull(s.requested(BleBearer.CCCD, byteArrayOf(0x01, 0x00, 0x00)))
        assertFalse(s.active)
        // and once subscribed, a stray write elsewhere does not unsubscribe
        s.requested(BleBearer.CCCD, byteArrayOf(0x01, 0x00))
        assertNull(s.requested(other, byteArrayOf(0x00, 0x00)))
        assertTrue(s.active)
    }

    @Test
    fun the_client_is_ready_only_on_a_confirmed_descriptor_write() {
        val s = BleBearer.Subscription()
        assertFalse(s.confirmed(133))
        assertFalse(s.active)
        assertTrue(s.confirmed(0))
        assertTrue(s.active)
        // a later failure, as on a reconnect, takes it back
        assertFalse(s.confirmed(3))
        assertFalse(s.active)
    }

    @Test
    fun a_disconnect_clears_either_side() {
        val s = BleBearer.Subscription()
        s.confirmed(0)
        s.reset()
        assertFalse(s.active)
        s.requested(BleBearer.CCCD, byteArrayOf(0x01, 0x00))
        s.reset()
        assertFalse(s.active)
    }
}
