package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **D7, the adoption the meeting carried**, held on the JVM: the five
 * frames, who sends which, and what each side shows when one of them is
 * refused.
 *
 * The kernel is not here. What is here is the shell's own part: the
 * direction the responder settles and the initiator is told, the order of
 * the legs, and the rule that a refusal is carried with its reason and
 * leaves the meeting alone (PRD-05, `light-client-requirements.md` §7).
 */
class AdoptionTest {

    /** A kernel that answers, recording what it was asked. */
    private class Kern(
        val series: Int? = 7,
        val refuseBody: String? = null,
        val refuseProposal: String? = null,
        val refuseTake: String? = null,
    ) : Adoption.Kern {
        val asked = mutableListOf<String>()
        override fun freshSeries(): Int? {
            asked.add("freshSeries")
            return series
        }

        override fun backPointers(): List<ByteArray> {
            asked.add("backPointers")
            return listOf(ByteArray(32) { 9 })
        }

        override fun position(): String = "a".repeat(64)

        override fun propose(
            node: String,
            presence: String,
            series: Int,
            nodeBack: List<ByteArray>,
        ): Adoption.Result<ByteArray> {
            asked.add("propose:$series:${nodeBack.size}")
            refuseProposal?.let { return Adoption.Result.Refused(it) }
            return Adoption.Result.Ok("body-$series".toByteArray())
        }

        override fun reviewAndSign(body: ByteArray): Adoption.Result<ByteArray> {
            asked.add("reviewAndSign:${String(body)}")
            refuseBody?.let { return Adoption.Result.Refused(it) }
            return Adoption.Result.Ok("entries".toByteArray())
        }

        override fun envelope(
            body: ByteArray,
            entries: List<Pair<String, ByteArray>>,
        ): Adoption.Result<ByteArray> {
            asked.add("envelope:${entries.size}")
            return Adoption.Result.Ok("envelope".toByteArray())
        }

        override fun take(envelope: ByteArray): Adoption.Result<String> {
            asked.add("take:${String(envelope)}")
            refuseTake?.let { return Adoption.Result.Refused(it) }
            return Adoption.Result.Ok("txid-${String(envelope)}")
        }

        override fun positionIn(anchor: String): String? = "0/$anchor"
    }

    /** Two devices and a wire between them, each frame delivered at once. */
    private class Pair2(
        initiatorKern: Adoption.Kern = Kern(),
        responderKern: Adoption.Kern = Kern(),
        val drop: Boolean = false,
    ) {
        lateinit var initiator: Adoption
        lateinit var responder: Adoption
        val toInitiator = mutableListOf<ByteArray>()
        val toResponder = mutableListOf<ByteArray>()

        init {
            initiator = Adoption(
                role = Meet.Role.INITIATOR,
                counterparty = "bb",
                counterpartyName = "carol",
                presence = "cafe",
                kern = initiatorKern,
                post = { b ->
                    toResponder.add(b)
                    if (!drop) responder.frame(b)
                    !drop
                },
            )
            responder = Adoption(
                role = Meet.Role.RESPONDER,
                counterparty = "aa",
                counterpartyName = "alice",
                presence = "cafe",
                kern = responderKern,
                post = { b ->
                    toInitiator.add(b)
                    if (!drop) initiator.frame(b)
                    !drop
                },
            )
        }
    }

    @Test
    fun the_responder_states_the_direction_and_the_initiator_is_told_it() {
        val p = Pair2()
        p.initiator.begin()
        assertNull("nothing is settled until it is told", p.initiator.direction())
        p.responder.settle(Meet.Adopt.THEM_UNDER_ME)
        // the responder is the patron, so at the initiator it is the other way
        assertEquals(Meet.Adopt.THEM_UNDER_ME, p.responder.direction())
        assertEquals(Meet.Adopt.ME_UNDER_THEM, p.initiator.direction())
        assertTrue(p.responder.patron)
        assertTrue(!p.initiator.patron)
    }

    @Test
    fun the_five_legs_run_in_order_and_both_devices_hold_the_record() {
        val patron = Kern()
        val node = Kern(series = 3)
        val p = Pair2(initiatorKern = node, responderKern = patron)
        p.initiator.begin()
        p.responder.settle(Meet.Adopt.THEM_UNDER_ME)
        // the party being adopted offered what only it knows; the patron
        // proposed on it; the record is held at both
        assertEquals(listOf("freshSeries", "backPointers", "reviewAndSign:body-3", "take:envelope"), node.asked)
        assertEquals(listOf("propose:3:1", "envelope:1", "take:envelope"), patron.asked)
        assertEquals(Adoption.Step.HELD, p.initiator.step())
        assertEquals(Adoption.Step.HELD, p.responder.step())
        assertEquals("txid-envelope", p.initiator.txid())
        assertEquals("txid-envelope", p.responder.txid())
        assertNotNull(p.initiator.position())
    }

    @Test
    fun the_initiator_is_the_patron_where_the_responder_says_so() {
        val patron = Kern()
        val node = Kern(series = 4)
        val p = Pair2(initiatorKern = patron, responderKern = node)
        p.initiator.begin()
        p.responder.settle(Meet.Adopt.ME_UNDER_THEM)
        assertTrue(p.initiator.patron)
        assertEquals(Adoption.Step.HELD, p.initiator.step())
        assertEquals("propose:4:1", patron.asked.first { it.startsWith("propose") })
    }

    @Test
    fun a_refused_body_carries_its_reason_to_the_patron_and_stops_there() {
        val patron = Kern()
        val node = Kern(series = 3, refuseBody = "series 3 is one this key has occupied")
        val p = Pair2(initiatorKern = node, responderKern = patron)
        p.initiator.begin()
        p.responder.settle(Meet.Adopt.THEM_UNDER_ME)
        assertEquals(Adoption.Step.REFUSED, p.initiator.step())
        assertEquals(Adoption.Step.REFUSED, p.responder.step())
        assertTrue(
            "the patron is told what the other device refused: ${p.responder.reason()}",
            p.responder.reason()!!.contains("series 3 is one this key has occupied"),
        )
        // and nothing was recorded on either side
        assertNull(p.initiator.txid())
        assertNull(p.responder.txid())
        assertTrue(patron.asked.none { it.startsWith("take") })
    }

    @Test
    fun neither_is_a_real_answer_and_the_other_device_is_told() {
        val p = Pair2()
        p.initiator.begin()
        p.responder.settle(Meet.Adopt.NONE)
        assertEquals(Meet.Adopt.NONE, p.initiator.direction())
        assertEquals(Adoption.Step.HELD, p.initiator.step())
        assertEquals(Adoption.Step.HELD, p.responder.step())
        assertNull("nothing was recorded", p.initiator.txid())
        assertTrue(p.initiator.said().any { it.contains("left the authority question alone") })
    }

    @Test
    fun a_frame_that_does_not_read_is_refused_and_nothing_is_signed() {
        val node = Kern()
        val p = Pair2(initiatorKern = node)
        p.initiator.frame(byteArrayOf(9, 9, 9))
        assertEquals(Adoption.Step.REFUSED, p.initiator.step())
        assertTrue(node.asked.isEmpty())
    }

    @Test
    fun a_leg_at_the_wrong_side_is_refused() {
        // an offer reaching the party being adopted, or a body reaching
        // the patron, is a device with the direction backwards. Alone on
        // its own wire, so nothing of the other side answers
        val sent = mutableListOf<ByteArray>()
        val node = Adoption(
            role = Meet.Role.RESPONDER,
            counterparty = "aa",
            counterpartyName = "alice",
            presence = "cafe",
            kern = Kern(),
            post = { b -> sent.add(b); true },
        )
        node.settle(Meet.Adopt.ME_UNDER_THEM)
        node.frame(Adoption.Frame.offer(2, listOf(ByteArray(32))))
        assertEquals(Adoption.Step.REFUSED, node.step())
        assertTrue(node.reason()!!.contains("offer arrived at the party being adopted"))

        val patron = Adoption(
            role = Meet.Role.RESPONDER,
            counterparty = "aa",
            counterpartyName = "alice",
            presence = "cafe",
            kern = Kern(),
            post = { b -> sent.add(b); true },
        )
        patron.settle(Meet.Adopt.THEM_UNDER_ME)
        patron.frame(Adoption.Frame.body("not mine to review".toByteArray()))
        assertEquals(Adoption.Step.REFUSED, patron.step())
        assertTrue(patron.reason()!!.contains("arrived at the patron"))
    }

    @Test
    fun a_frame_the_radio_would_not_take_is_said_and_not_silently_lost() {
        val p = Pair2(drop = true)
        p.responder.settle(Meet.Adopt.THEM_UNDER_ME)
        assertEquals(Adoption.Step.REFUSED, p.responder.step())
        assertTrue(p.responder.reason()!!.contains("could not be sent"))
    }

    @Test
    fun the_frames_read_back_as_what_was_written() {
        val back = listOf(ByteArray(32) { 1 }, ByteArray(32) { 2 })
        val offer = Adoption.Frame.read(Adoption.Frame.offer(11, back))
        assertTrue(offer is Adoption.Frame.Offer)
        assertEquals(11, (offer as Adoption.Frame.Offer).series)
        assertEquals(2, offer.back.size)
        assertTrue(offer.back[1].all { it == 2.toByte() })
        assertEquals(
            "why not",
            (Adoption.Frame.read(Adoption.Frame.refused("why not")) as Adoption.Frame.Refused).why,
        )
        // a truncated offer, and a version this build does not know
        assertNull(Adoption.Frame.read(byteArrayOf(1, 1, 0, 0, 0, 5)))
        assertNull(Adoption.Frame.read(byteArrayOf(2, 1, 0, 0, 0, 5, 0)))
        assertNull(Adoption.Frame.read(ByteArray(0)))
    }

    /** The direction frame carries the subnet's label, in words, so the
     *  initiator's acceptance can state it (`light-client-requirements.md` §7). */
    @Test
    fun the_direction_frame_carries_the_subnet_label_and_reads_back() {
        val with = Adoption.Frame.read(Adoption.Frame.direction(1, "Dana's Bowling League"))
        assertEquals(Adoption.Frame.Direction(1, "Dana's Bowling League"), with)
        val without = Adoption.Frame.read(Adoption.Frame.direction(2))
        assertEquals(Adoption.Frame.Direction(2, null), without)
    }
}
