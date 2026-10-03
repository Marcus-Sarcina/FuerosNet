package com.comptus.fueros

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * The bearer's counters (`bearer.packets`): what one carriage counts on
 * the way out and on the way in, and the events those counts become.
 */
class CarriageCountersTest {

    private val lines = mutableListOf<String>()

    @Before
    fun collect() = Diag.install({ lines.add(it) })

    @After
    fun release() = Diag.uninstall()

    private class Wire(val mtu: Int, val failFrom: Int = Int.MAX_VALUE) : Bearer.Link {
        val sent = ArrayList<ByteArray>()
        override fun mtu() = mtu
        override fun send(packet: ByteArray): Boolean {
            if (sent.size >= failFrom) return false
            return sent.add(packet)
        }
    }

    private fun events(name: String) = lines.filter { it.contains("\"event\":\"$name\"") }

    @Test
    fun the_sender_counts_packets_and_slice_bytes_per_phase() {
        val wire = Wire(24)
        val out = Carriage(wire)
        val messages = listOf(ByteArray(50), ByteArray(21))
        assertTrue(out.send(messages, Carriage.Phase.PROXIMITY))
        val c = out.counters(Carriage.Phase.PROXIMITY)
        // 20 bytes of slice per packet: 3 + 2 packets
        assertEquals(5, c.sent)
        assertEquals(71, c.sentBytes)
        assertEquals(0, c.sendFailed)
        assertEquals(0, out.counters(Carriage.Phase.INTENT).sent)
        val e = events("bearer.packets").single()
        assertTrue(
            e,
            e.contains("\"dir\":\"sent\",\"phase\":1,\"messages\":2,\"packets\":5,\"bytes\":71,\"failed\":0,\"ok\":true"),
        )
    }

    @Test
    fun a_packet_the_link_would_not_take_is_counted_as_failed() {
        val wire = Wire(24, failFrom = 2)
        val out = Carriage(wire)
        assertFalse(out.send(listOf(ByteArray(50)), Carriage.Phase.INTENT))
        val c = out.counters(Carriage.Phase.INTENT)
        assertEquals(2, c.sent)
        assertEquals(1, c.sendFailed)
    }

    @Test
    fun the_receiver_counts_taken_duplicates_and_refusals_and_times_the_assembly() {
        val wire = Wire(24)
        Carriage(wire).send(listOf(ByteArray(50) { it.toByte() }, ByteArray(5)), Carriage.Phase.INTENT)
        lines.clear()
        val inward = Carriage(null)
        for (p in wire.sent) assertNull(inward.packet(p))
        // a repeat, which the assembly drops
        assertNull(inward.packet(wire.sent[0]))
        // an empty slice that ends nothing, which the assembly refuses
        val bad = byteArrayOf(0, (Carriage.Phase.INTENT shl 4).toByte(), 0, 5)
        assertEquals("an empty slice that ends nothing", inward.packet(bad))
        val set = inward.received(Carriage.Phase.INTENT)
        assertNotNull(set)
        assertEquals(2, set!!.size)
        val c = inward.counters(Carriage.Phase.INTENT)
        assertEquals(4, c.received)
        assertEquals(55, c.receivedBytes)
        assertEquals(1, c.duplicates)
        assertEquals(mapOf("an empty slice that ends nothing" to 1), c.refused)
        assertNotNull(c.wholeMs)
        val refused = events("bearer.refused").single()
        assertTrue(refused, refused.contains("\"level\":\"warn\""))
        assertTrue(refused, refused.contains("\"phase\":0,\"reason\":\"an empty slice that ends nothing\""))
        val report = events("bearer.packets").single()
        assertTrue(
            report,
            report.contains(
                "\"dir\":\"received\",\"phase\":0,\"sent\":0,\"sent_bytes\":0,\"send_failed\":0," +
                    "\"received\":4,\"received_bytes\":55,\"duplicates\":1,\"refused\":1," +
                    "\"refused_why\":{\"an empty slice that ends nothing\":1},\"assembly_ms\":",
            ),
        )
        // taken again, the set is reported once
        assertNotNull(inward.received(Carriage.Phase.INTENT))
        inward.discard(Carriage.Phase.INTENT)
        assertEquals(1, events("bearer.packets").size)
    }

    @Test
    fun a_phase_discarded_before_it_was_whole_is_still_reported() {
        val wire = Wire(24)
        Carriage(wire).send(listOf(ByteArray(50)), Carriage.Phase.CAPTURE_KEY)
        lines.clear()
        val inward = Carriage(null)
        inward.packet(wire.sent[0])
        assertNull(inward.received(Carriage.Phase.CAPTURE_KEY))
        inward.discard(Carriage.Phase.CAPTURE_KEY)
        val report = events("bearer.packets").single()
        assertTrue(report, report.contains("\"dir\":\"received\",\"phase\":2"))
        assertTrue(report, report.contains("\"received\":1,"))
        assertFalse(report, report.contains("assembly_ms"))
    }

    @Test
    fun a_short_packet_is_refused_against_no_phase() {
        val inward = Carriage(null)
        assertEquals("a packet shorter than its header", inward.packet(byteArrayOf(1, 2)))
        assertEquals(1, inward.counters(-1).refusals())
    }
}
