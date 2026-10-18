package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **A3, the peering the meeting carried**, held on the JVM: the four
 * frames, which side builds the body, and what each side shows when one
 * of them refuses.
 *
 * The kernel is not here. What is here is the shell's own part: that the
 * lower keyhash builds, that the other side sends the one thing the
 * builder cannot work out, that the legs run in order, and that a
 * refusal is carried with its reason and leaves the meeting alone.
 */
class PeeringTest {

    private class Kern(
        val refuseProposal: String? = null,
        val refuseBody: String? = null,
        val refuseTake: String? = null,
        val back: List<ByteArray> = listOf(ByteArray(32) { 9 }),
    ) : Peering.Kern {
        val asked = mutableListOf<String>()

        override fun backPointers(): List<ByteArray> {
            asked.add("backPointers")
            return back
        }

        override fun propose(
            other: String,
            presence: String,
            otherBack: List<ByteArray>,
        ): Peering.Result<ByteArray> {
            asked.add("propose:${otherBack.size}")
            return refuseProposal?.let { Peering.Result.Refused(it) }
                ?: Peering.Result.Ok(byteArrayOf(0x50, 0x45))
        }

        override fun reviewAndSign(body: ByteArray): Peering.Result<ByteArray> {
            asked.add("reviewAndSign")
            return refuseBody?.let { Peering.Result.Refused(it) }
                ?: Peering.Result.Ok(byteArrayOf(0x51))
        }

        override fun envelope(
            body: ByteArray,
            entries: List<Pair<String, ByteArray>>,
        ): Peering.Result<ByteArray> {
            asked.add("envelope:${entries.size}")
            return Peering.Result.Ok(byteArrayOf(0x52))
        }

        override fun take(envelope: ByteArray): Peering.Result<String> {
            asked.add("take")
            return refuseTake?.let { Peering.Result.Refused(it) }
                ?: Peering.Result.Ok("ab".repeat(32))
        }
    }

    private val low = "11".repeat(32)
    private val high = "ee".repeat(32)

    private fun peering(me: String, them: String, kern: Kern, sent: MutableList<ByteArray>) =
        Peering(
            me = me,
            counterparty = them,
            counterpartyName = "Bob",
            presence = "cd".repeat(32),
            kern = kern,
            post = { sent.add(it); true },
        )

    // acceptance: PRD-16
    @Test
    fun the_lower_keyhash_builds_the_body_and_the_other_sends_its_back_pointers() {
        assertTrue(peering(low, high, Kern(), mutableListOf()).builder)
        assertFalse(peering(high, low, Kern(), mutableListOf()).builder)
    }

    @Test
    fun the_builder_waits_and_the_other_offers() {
        val sent = mutableListOf<ByteArray>()
        val k = Kern()
        peering(low, high, k, sent).begin()
        assertTrue("the builder sends nothing", sent.isEmpty())
        assertEquals(emptyList<String>(), k.asked)

        val theirs = mutableListOf<ByteArray>()
        val k2 = Kern()
        peering(high, low, k2, theirs).begin()
        assertEquals(1, theirs.size)
        val read = Peering.Frame.read(theirs[0])
        assertTrue(read is Peering.Frame.Offer)
        assertEquals(1, (read as Peering.Frame.Offer).back.size)
        assertEquals(listOf("backPointers"), k2.asked)
    }

    @Test
    fun the_four_legs_run_in_order_and_both_sides_end_holding_the_record() {
        // the builder's side
        val sent = mutableListOf<ByteArray>()
        val k = Kern()
        val b = peering(low, high, k, sent)
        b.begin()
        b.frame(Peering.Frame.offer(listOf(ByteArray(32) { 3 })))
        assertEquals(Peering.Step.PROPOSED, b.step())
        assertTrue(Peering.Frame.read(sent.last()) is Peering.Frame.Body)
        b.frame(Peering.Frame.signed(byteArrayOf(0x51)))
        assertEquals(Peering.Step.HELD, b.step())
        assertEquals("ab".repeat(32), b.txid())
        // both entries went into the envelope: theirs, and the kernel adds
        // this device's own
        assertTrue(k.asked.contains("envelope:1"))
        assertTrue(Peering.Frame.read(sent.last()) is Peering.Frame.Record)

        // the other side's
        val theirs = mutableListOf<ByteArray>()
        val k2 = Kern()
        val o = peering(high, low, k2, theirs)
        o.begin()
        o.frame(Peering.Frame.body(byteArrayOf(0x50, 0x45)))
        assertEquals(Peering.Step.SIGNED, o.step())
        assertTrue(Peering.Frame.read(theirs.last()) is Peering.Frame.Signed)
        o.frame(Peering.Frame.record(byteArrayOf(0x52)))
        assertEquals(Peering.Step.HELD, o.step())
        assertNotNull(o.txid())
    }

    @Test
    fun a_refused_body_is_carried_with_its_reason_both_ways() {
        val theirs = mutableListOf<ByteArray>()
        val o = peering(high, low, Kern(refuseBody = "the meeting is not one of mine"), theirs)
        o.frame(Peering.Frame.body(byteArrayOf(0x50)))
        assertEquals(Peering.Step.REFUSED, o.step())
        assertTrue(o.reason()!!.contains("the meeting is not one of mine"))
        val back = Peering.Frame.read(theirs.last())
        assertTrue(back is Peering.Frame.Refused)

        val sent = mutableListOf<ByteArray>()
        val b = peering(low, high, Kern(), sent)
        b.frame(Peering.Frame.refused("the meeting is not one of mine"))
        assertEquals(Peering.Step.REFUSED, b.step())
        assertTrue(b.reason()!!.contains("Bob"))
    }

    @Test
    fun a_leg_arriving_at_the_wrong_side_is_refused_rather_than_acted_on() {
        // a body at the builder
        val b = peering(low, high, Kern(), mutableListOf())
        b.frame(Peering.Frame.body(byteArrayOf(1)))
        assertEquals(Peering.Step.REFUSED, b.step())
        // back-pointers at the side that does not build
        val o = peering(high, low, Kern(), mutableListOf())
        o.frame(Peering.Frame.offer(listOf(ByteArray(32))))
        assertEquals(Peering.Step.REFUSED, o.step())
        // a signature with no body held
        val n = peering(low, high, Kern(), mutableListOf())
        n.frame(Peering.Frame.signed(byteArrayOf(1)))
        assertEquals(Peering.Step.REFUSED, n.step())
    }

    @Test
    fun the_kernels_refusals_are_the_peerings_to_carry() {
        val b = peering(low, high, Kern(refuseProposal = "no position for that peer"), mutableListOf())
        b.frame(Peering.Frame.offer(listOf(ByteArray(32))))
        assertEquals(Peering.Step.REFUSED, b.step())
        assertTrue(b.reason()!!.contains("no position for that peer"))

        val t = peering(low, high, Kern(refuseTake = "the record did not verify"), mutableListOf())
        t.frame(Peering.Frame.offer(listOf(ByteArray(32))))
        t.frame(Peering.Frame.signed(byteArrayOf(1)))
        assertEquals(Peering.Step.REFUSED, t.step())
        assertNull(t.txid())
    }

    @Test
    fun the_frames_read_back_and_nothing_else_reads_as_one() {
        val back = listOf(ByteArray(32) { 1 }, ByteArray(32) { 2 })
        val offer = Peering.Frame.read(Peering.Frame.offer(back)) as Peering.Frame.Offer
        assertEquals(2, offer.back.size)
        assertEquals(1, offer.back[0][0].toInt())
        assertEquals(
            byteArrayOf(7, 8).toList(),
            (Peering.Frame.read(Peering.Frame.body(byteArrayOf(7, 8))) as Peering.Frame.Body).body.toList(),
        )
        assertEquals(
            "why",
            (Peering.Frame.read(Peering.Frame.refused("why")) as Peering.Frame.Refused).why,
        )
        // an empty offer, a short one and a long one are not frames
        assertNull(Peering.Frame.read(byteArrayOf(1, 0, 0)))
        assertNull(Peering.Frame.read(byteArrayOf(1, 0, 1) + ByteArray(31)))
        assertNull(Peering.Frame.read(byteArrayOf(1, 0, 1) + ByteArray(33)))
        // another version, an unknown type, and an adoption's frame
        assertNull(Peering.Frame.read(byteArrayOf(2, 0, 1) + ByteArray(32)))
        assertNull(Peering.Frame.read(byteArrayOf(1, 9)))
        assertFalse(Peering.Frame.mine(byteArrayOf(1)))
        // the two conversations ride different kinds, so neither device
        // ever hands one to the other's reader
        assertTrue(Peering.KIND != Adoption.KIND)
    }
}
