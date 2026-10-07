package com.comptus.fueros

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Three QRs in one image, round-tripped on the JVM: composed into ARGB
 * pixels, separated back into three channels, each decoded by ZXing.
 *
 * **What a JVM cannot test is the camera**, and for this format the camera
 * is the whole question: a `YUV_420_888` frame carries chroma at half the
 * linear resolution of luminance, so whether three channels separate at
 * module scale is a hardware measurement and the probe exchange is what
 * takes it. What runs here is that the composition and the separation are
 * inverses, and that losing a channel loses exactly one part.
 */
class PolychromeTest {

    private fun part(n: Int, bytes: Int = 11) = ByteArray(bytes) { (n * 37 + it).toByte() }

    private fun frame(n: Int, got: Int, bytes: Int = 11) =
        (0 until 3).map { ch -> Polychrome.header(n, got) + part(n * 3 + ch, bytes) }

    @Test
    fun a_frame_composes_and_separates_into_the_three_parts_it_carried() {
        val parts = frame(0, 0)
        val (px, w) = Polychrome.compose(parts, scale = 8)
        val out = Polychrome.separate(px, w, w)
        assertEquals(3, out.size)
        for (ch in 0 until 3) {
            assertNotNull("channel $ch decoded", out[ch])
            assertArrayEquals("channel $ch is its own part", parts[ch], out[ch])
        }
    }

    @Test
    fun a_channel_lost_costs_one_part_and_not_the_frame() {
        val parts = frame(4, 7)
        val (px, w) = Polychrome.compose(parts, scale = 8)
        // green gone: the camera saw no green, so that plane is flat
        val damaged = IntArray(px.size) { px[it] and 0xffff00ff.toInt() }
        val out = Polychrome.separate(damaged, w, w)
        assertArrayEquals("red survives", parts[0], out[0])
        assertNull("green is the part that was lost", out[1])
        assertArrayEquals("blue survives", parts[2], out[2])
    }

    /** **A composed frame decodes to nothing in monochrome**, which is
     *  why a reader that cannot separate the channels reports holding none
     *  of the sender's parts rather than some: luminance is a blend of the
     *  three symbols and not any one of them. */
    @Test
    fun a_colour_frame_is_not_a_monochrome_code() {
        val parts = frame(0, 0)
        val (px, w) = Polychrome.compose(parts, scale = 8)
        val luma = IntArray(px.size) { i ->
            val p = px[i]
            val y = (0.299f * ((p ushr 16) and 0xff) + 0.587f * ((p ushr 8) and 0xff) +
                0.114f * (p and 0xff)).toInt().coerceIn(0, 255)
            (0xff shl 24) or (y shl 16) or (y shl 8) or y
        }
        assertNull("a monochrome reader sees a blend, not a symbol", Polychrome.decode(luma, w, w))
    }

    @Test
    fun a_compressed_header_is_never_mistaken_for_a_monochrome_one() {
        val c = Polychrome.header(42, 9)
        assertTrue(Polychrome.compressed(c))
        assertEquals(42, Polychrome.frameOf(c))
        assertEquals(9, Polychrome.gotOf(c))
        // a monochrome frame opens with its version, which is 1
        val mono = OpticalExchange(OpticalExchange.CONTRIBUTION, ByteArray(40), 20).frame()
        assertEquals(OpticalExchange.VERSION, mono[0].toInt())
        assertFalse("a monochrome header is not read as compressed", Polychrome.compressed(mono))
        assertEquals(-1, Polychrome.frameOf(mono))
    }

    @Test
    fun the_frame_index_is_seven_bits_and_says_so() {
        Polychrome.header(Polychrome.MAX_FRAME, 0)
        try {
            Polychrome.header(Polychrome.MAX_FRAME + 1, 0)
            throw AssertionError("a frame index past seven bits is refused")
        } catch (e: IllegalArgumentException) {
            assertTrue(e.message!!.contains("seven bits"))
        }
    }

    /**
     * **The symbol the format exists for.** Eleven payload bytes under a
     * two-byte header is thirteen, which is a version-1 symbol — the
     * smallest QR there is, 25 modules with its quiet zone, reading at
     * about 18 × its own width. Three of them a frame carries the
     * 2,022-byte contribution in 62 frames.
     */
    @Test
    fun eleven_byte_parts_are_the_smallest_symbol_and_sixty_two_frames() {
        val one = Polychrome.header(0, 0) + part(0, 11)
        assertEquals("a part plus its compressed header", 13, one.size)
        assertEquals("the smallest symbol, with its quiet zone", 25, Optical.matrix(one).width)
        val frames = (2022 + 11 * 3 - 1) / (11 * 3)
        assertEquals("frames for the contribution", 62, frames)
    }
}

/**
 * The exchange driven in colour: three parts a frame, the frame that
 * decides it carrying full headers, and the fall back to monochrome when
 * fewer than three of its channels arrive.
 */
class PolychromeExchangeTest {

    private fun obj(n: Int, seed: Int) = ByteArray(n) { (it * 7 + seed).toByte() }

    /** Move one colour frame from `from` to `to`, dropping the channels
     *  named, and answer what each channel came to. */
    private fun cross(
        from: OpticalExchange,
        to: OpticalExchange,
        compressed: Boolean,
        drop: Set<Int> = emptySet(),
    ): List<OpticalExchange.Took> {
        val frames = from.colourFrames(compressed)
        val (px, w) = Polychrome.compose(
            // a short frame at the object's end is padded with a repeat of
            // its last part, as a sender must put something in the channel
            (0 until 3).map { frames.getOrElse(it) { frames.last() } },
            scale = 8,
        )
        val seen = Polychrome.separate(px, w, w)
        return seen.mapIndexed { ch, bytes ->
            if (ch in drop || bytes == null) OpticalExchange.Took.NOT_OURS else to.take(bytes, ch)
        }
    }

    @Test
    fun a_colour_exchange_carries_three_parts_a_frame_and_completes() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 1), OpticalExchange.CHUNK_POLY)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 2), OpticalExchange.CHUNK_POLY)
        assertEquals("parts of eleven bytes", 9, a.count)

        // the frame that decides it: full headers, so the count crosses
        val first = cross(a, b, compressed = false)
        assertEquals(
            "three parts and the count",
            listOf(OpticalExchange.Took.ACCEPTED, OpticalExchange.Took.ACCEPTED, OpticalExchange.Took.ACCEPTED),
            first,
        )
        assertEquals(9, b.theirCount())
        assertEquals(3, b.received())

        // and the rest compressed, three a frame
        var guard = 0
        while (!b.complete() && guard++ < 10) {
            // b's progress has to reach a for a to advance its frame
            a.take(b.frame())
            cross(a, b, compressed = true)
        }
        assertTrue("b holds all of a's object", b.complete())
        assertArrayEquals(obj(99, 1), b.theirs())
        assertTrue("four frames carried nine parts", guard <= 4)
    }

    @Test
    fun a_frame_with_two_channels_read_is_what_the_fall_back_is_decided_on() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 1), OpticalExchange.CHUNK_POLY)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 2), OpticalExchange.CHUNK_POLY)
        cross(a, b, compressed = false, drop = setOf(2))
        assertEquals("two of three channels arrived", 2, b.received())
        // which is what the sender reads off b's header and acts on
        a.take(b.frame())
        assertEquals(2, a.theirReceived())
    }

    @Test
    fun a_sender_that_falls_back_re_partitions_and_the_receiver_follows() {
        val whole = obj(300, 1)
        val colour = OpticalExchange(OpticalExchange.CONTRIBUTION, whole, OpticalExchange.CHUNK_POLY)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 2), OpticalExchange.CHUNK_POLY)
        cross(colour, b, compressed = false)
        assertEquals("three colour parts held", 3, b.received())
        assertEquals(28, b.theirCount())

        // colour did not work for the other side, so this one starts again
        // in monochrome: a different partitioning of the same object
        val mono = OpticalExchange(OpticalExchange.CONTRIBUTION, whole, OpticalExchange.CHUNK)
        assertEquals((whole.size + OpticalExchange.CHUNK - 1) / OpticalExchange.CHUNK, mono.count)
        assertEquals(OpticalExchange.Took.ACCEPTED, b.take(mono.frame()))
        assertEquals("the old partitioning's parts went with it", 1, b.received())
        assertEquals(mono.count, b.theirCount())
        var guard = 0
        while (!b.complete() && guard++ < 4 * mono.count) {
            mono.take(b.frame())
            b.take(mono.frame())
        }
        assertTrue("and the object assembles under the new one", b.complete())
        assertArrayEquals(whole, b.theirs())
    }

    @Test
    fun a_compressed_part_before_the_count_has_arrived_is_not_placed() {
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 2), OpticalExchange.CHUNK_POLY)
        val early = Polychrome.header(0, 0) + ByteArray(11)
        assertEquals(OpticalExchange.Took.NOT_OURS, b.take(early, 0))
    }

    /**
     * **The fall back has to be symmetric** [author, 2026-10-06]: the two
     * presentations are lock-step, so a side left in colour while the
     * other has gone monochrome reads nothing of theirs at all and its own
     * allowance never expires. The length of a part is what says which
     * format the other side is on, and nothing else has to be sent.
     */
    @Test
    fun a_monochrome_part_of_theirs_is_how_a_colour_side_learns_to_follow() {
        val colour = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 1), OpticalExchange.CHUNK_POLY)
        val stillColour = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 2), OpticalExchange.CHUNK_POLY)
        assertFalse("nothing of theirs read yet", colour.theirsAreCoarse())
        cross(stillColour, colour, compressed = false)
        assertFalse("their parts are colour-sized", colour.theirsAreCoarse())

        // the other side has given up and re-partitioned at the monochrome
        // chunk; its next code is one 76-byte part under a full header
        val mono = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(2022, 2), OpticalExchange.CHUNK)
        assertEquals(OpticalExchange.Took.ACCEPTED, colour.take(mono.frame(), 0))
        assertTrue("a part longer than a colour frame carries", colour.theirsAreCoarse())
    }

    /** A short last part cannot make a monochrome partitioning look like a
     *  colour one: it is short, and the test is for a part that is long. */
    @Test
    fun a_short_last_part_does_not_read_as_colour() {
        val colour = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 1), OpticalExchange.CHUNK_POLY)
        // a monochrome object whose last part is short: the long parts are
        // what settle it, and the first one it shows is long
        val mono = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(OpticalExchange.CHUNK + 1, 2), OpticalExchange.CHUNK)
        assertEquals("a long part and a one-byte one", 2, mono.count)
        assertEquals(OpticalExchange.Took.ACCEPTED, colour.take(mono.frame(), 0))
        assertTrue("the first part is a whole monochrome chunk", colour.theirsAreCoarse())
    }

    /**
     * **The other side's progress arrives on a part already held**, and
     * the screen depends on it: `take` reads a header's `got` before it
     * judges the part a duplicate, and that figure is what advances this
     * side's own presentation. A redraw gated on the verdict rather than
     * on either count moving would stall the lockstep.
     */
    /**
     * **Both cameras, or neither.** The conjunction is what keeps the two
     * lock-step presentations on one format: each side holds one half of
     * the evidence directly and reads the other half off the
     * counterparty's `got`, so the two verdicts cannot diverge.
     */
    @Test
    fun colour_is_compressed_only_once_both_cameras_have_read_three() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 1), OpticalExchange.CHUNK_POLY)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 2), OpticalExchange.CHUNK_POLY)
        val m = Meet("aa", "carol", Meet.Kind())
        m.colourSince = 0L

        // nothing read: probing, and no clock has run out
        assertEquals(Meet.Colour.PROBE, m.colourVerdict(a, 1_000L))

        // this camera separated all three of theirs, but their header has
        // not yet said the same of mine: still probing
        cross(b, a, compressed = false)
        m.colourChannels = 3
        assertEquals("my camera alone is half the evidence", Meet.Colour.PROBE, m.colourVerdict(a, 1_000L))

        // their camera reads three of mine and says so in its next header
        cross(a, b, compressed = false)
        cross(b, a, compressed = false)
        assertEquals("their header says three", 3, a.theirReceived())
        assertEquals(Meet.Colour.COMPRESS, m.colourVerdict(a, 1_000L))

        // and the other way round: their header says three, this camera
        // never managed more than two
        val m2 = Meet("aa", "carol", Meet.Kind())
        m2.colourSince = 0L
        m2.colourChannels = 2
        assertEquals("their camera alone is the other half", Meet.Colour.PROBE, m2.colourVerdict(a, 1_000L))
    }

    /** The allowance is on a probe that has gone quiet, and a capture that
     *  separated all three is what restarts it — so a clock that has run
     *  out is a camera that has not managed a full capture in that time. */
    @Test
    fun a_quiet_probe_falls_back_and_a_confirmed_one_does_not() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 1), OpticalExchange.CHUNK_POLY)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 2), OpticalExchange.CHUNK_POLY)
        val m = Meet("aa", "carol", Meet.Kind())
        m.colourSince = 0L
        assertEquals(Meet.Colour.PROBE, m.colourVerdict(a, Meet.COLOUR_PROBE_MS))
        assertEquals(Meet.Colour.FALL_BACK, m.colourVerdict(a, Meet.COLOUR_PROBE_MS + 1))

        // the same clock, but both cameras have shown three: the allowance
        // has nothing left to decide
        cross(b, a, compressed = false)
        cross(a, b, compressed = false)
        cross(b, a, compressed = false)
        m.colourChannels = 3
        assertEquals(Meet.Colour.COMPRESS, m.colourVerdict(a, Meet.COLOUR_PROBE_MS * 10))
    }

    /** Following the other side out of colour beats every other rule: it
     *  is the case where this side would otherwise read nothing at all. */
    @Test
    fun following_them_out_of_colour_beats_a_confirmation() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 1), OpticalExchange.CHUNK_POLY)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 2), OpticalExchange.CHUNK_POLY)
        val m = Meet("aa", "carol", Meet.Kind())
        m.colourSince = 0L
        m.colourChannels = 3
        cross(b, a, compressed = false)
        cross(a, b, compressed = false)
        cross(b, a, compressed = false)
        assertEquals(Meet.Colour.COMPRESS, m.colourVerdict(a, 1_000L))

        // and then a monochrome part of theirs arrives
        val mono = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(2022, 2), OpticalExchange.CHUNK)
        a.take(mono.frame(), 0)
        assertEquals(Meet.Colour.FALL_BACK, m.colourVerdict(a, 1_000L))
    }

    /**
     * **The object's last frame killed run 2 of 2026-10-07**, one part
     * short of 184, with `"a code of the exchange did not read as one"`.
     *
     * A compressed header names only the frame; the reader takes the index
     * from the channel a part arrived in. The presentation must put
     * something in all three channels, so it pads a short frame by
     * repeating the last part — and the repeat, read in the next channel
     * along, decodes as the part *after* the last one, which does not
     * exist. `184 = 61 × 3 + 1`, so the final frame carried one real part
     * and two poisoned copies, and it could not have ended any other way.
     *
     * A short frame therefore goes out under full headers even mid-compression.
     */
    @Test
    fun the_last_frame_of_an_object_that_is_not_a_multiple_of_three() {
        // 34 bytes at an 11-byte chunk is 4 parts: frame 1 holds part 3
        // alone, which is the shape that killed the run
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(34, 1), OpticalExchange.CHUNK_POLY)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(34, 2), OpticalExchange.CHUNK_POLY)
        assertEquals(4, a.count)

        // b reads a's first frame — parts 0, 1 and 2 — and says so in its
        // own next frame, which is what moves a on to its last part
        cross(a, b, compressed = false)
        assertEquals("b holds three of a's", 3, b.received())
        cross(b, a, compressed = false)
        assertEquals("so a shows its fourth part", 3, a.showing())

        val frames = a.colourFrames(compressed = true)
        assertEquals("one real part in the last frame", 1, frames.size)
        assertFalse(
            "and it is self-describing, not compressed, or its copies poison the frame",
            Polychrome.compressed(frames[0]),
        )

        // the presentation pads the two spare channels with that part; each
        // copy carries its own true index, so it is a duplicate and not a
        // part past the end
        assertEquals(OpticalExchange.Took.COMPLETE, b.take(frames[0], 0))
        assertEquals(OpticalExchange.Took.DUPLICATE, b.take(frames[0], 1))
        assertEquals(OpticalExchange.Took.DUPLICATE, b.take(frames[0], 2))
        assertArrayEquals("a's object, whole", obj(34, 1), b.theirs())
    }

    /**
     * **The fall back restarts both sequences from the first part**
     * [author, 2026-10-07]: the side that falls back starts over and waits
     * for the other side to read its first monochrome part and re-present
     * its own, or parts are dropped at one end of the sequence or the
     * other.
     *
     * This drives a real mid-exchange fall back and then runs the
     * monochrome exchange to completion, asserting both objects assemble
     * byte for byte — which is the only check that catches a drop
     * anywhere in the sequence, including at its ends.
     */
    @Test
    fun a_fall_back_restarts_both_sequences_from_the_first_part() {
        val aObj = obj(600, 1)
        val bObj = obj(600, 2)
        var a = OpticalExchange(OpticalExchange.CONTRIBUTION, aObj, OpticalExchange.CHUNK_POLY)
        var b = OpticalExchange(OpticalExchange.CONTRIBUTION, bObj, OpticalExchange.CHUNK_POLY)
        val m = Meet("aa", "carol", Meet.Kind())
        m.colourSince = 0L

        // a colour probe that got somewhere: both sides hold parts and
        // both have reported progress, so there is state to be dropped
        cross(a, b, compressed = false)
        cross(b, a, compressed = false)
        cross(a, b, compressed = false)
        assertTrue("a holds parts of b", a.received() > 0)
        assertTrue("b has reported progress to a", a.theirReceived() > 0)

        // **a falls back**, which is `MeetActivity` dropping the exchange
        // and the next redraw building one at the monochrome chunk
        a = OpticalExchange(OpticalExchange.CONTRIBUTION, aObj, OpticalExchange.CHUNK)
        assertEquals("a starts over from its first part", 0, a.showing())
        assertEquals("and holds nothing of b's", 0, a.received())

        // b is still in colour and cannot read a monochrome presentation
        // as colour, so what reaches it is the luminance read: one part,
        // channel 0
        assertEquals(OpticalExchange.Took.ACCEPTED, b.take(a.frame(), 0))
        assertEquals("b follows a out of colour", Meet.Colour.FALL_BACK, m.colourVerdict(b, 1_000L))
        b = OpticalExchange(OpticalExchange.CONTRIBUTION, bObj, OpticalExchange.CHUNK)
        assertEquals("b starts over from its first part too", 0, b.showing())
        assertEquals("and its count of a's parts is gone with it", 0, b.received())

        // and now the monochrome lockstep, to completion
        var rounds = 0
        while (!(a.done() && b.done()) && rounds < 200) {
            val fromA = a.frame()
            val fromB = b.frame()
            a.take(fromB)
            b.take(fromA)
            rounds++
        }
        assertTrue("both finished in $rounds rounds", a.done() && b.done())
        assertArrayEquals("b's object, whole", bObj, a.theirs())
        assertArrayEquals("a's object, whole", aObj, b.theirs())
    }

    @Test
    fun their_progress_arrives_on_a_duplicate() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 1), OpticalExchange.CHUNK_POLY)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(99, 2), OpticalExchange.CHUNK_POLY)
        cross(a, b, compressed = false)
        assertEquals("b holds three of a's", 3, b.received())
        // a reads b's frame twice: the second is every part a duplicate,
        // and it is the delivery that tells a that b holds three of its own
        cross(b, a, compressed = false)
        val again = cross(b, a, compressed = false)
        assertTrue("every part held already", again.all { it == OpticalExchange.Took.DUPLICATE })
        assertEquals("and it still carried b's progress", 3, a.theirReceived())
    }
}

/**
 * **The format's one real risk, measured on the JVM**: a camera delivers
 * `YUV_420_888`, whose chroma is one sample per 2×2 block, so the colour
 * that separates three channels has half the linear resolution of the
 * luminance that resolves a monochrome module. This simulates that
 * pipeline — compose, convert to 4:2:0 as a camera would, convert back —
 * and asks whether the channels still decode.
 *
 * It is not the hardware answer: a real camera adds its own white balance,
 * gamma and noise, and this adds none of them. What it does settle is
 * whether the subsampling *alone* is survivable, which is the half that
 * can be known without a phone.
 */
class PolychromeChromaTest {

    /** A camera's 4:2:0: Y a sample a pixel, U and V one per 2×2 block,
     *  averaged over it as a sensor's binning does. */
    private fun toYuv420(argb: IntArray, w: Int, h: Int): Triple<ByteArray, ByteArray, ByteArray> {
        val y = ByteArray(w * h)
        val cw = (w + 1) / 2
        val ch = (h + 1) / 2
        val u = ByteArray(cw * ch)
        val v = ByteArray(cw * ch)
        val us = IntArray(cw * ch)
        val vs = IntArray(cw * ch)
        val n = IntArray(cw * ch)
        for (py in 0 until h) {
            for (px in 0 until w) {
                val p = argb[py * w + px]
                val r = (p ushr 16) and 0xff
                val g = (p ushr 8) and 0xff
                val b = p and 0xff
                val luma = (0.299f * r + 0.587f * g + 0.114f * b).toInt().coerceIn(0, 255)
                y[py * w + px] = luma.toByte()
                val i = (py / 2) * cw + (px / 2)
                us[i] += (-0.169f * r - 0.331f * g + 0.5f * b + 128f).toInt()
                vs[i] += (0.5f * r - 0.419f * g - 0.081f * b + 128f).toInt()
                n[i]++
            }
        }
        for (i in us.indices) {
            u[i] = (us[i] / n[i]).coerceIn(0, 255).toByte()
            v[i] = (vs[i] / n[i]).coerceIn(0, 255).toByte()
        }
        return Triple(y, u, v)
    }

    private fun channelsThrough420(parts: List<ByteArray>, scale: Int): List<ByteArray?> {
        val (px, w) = Polychrome.compose(parts, scale)
        val (y, u, v) = toYuv420(px, w, w)
        val cw = (w + 1) / 2
        val planes = Array(3) { ByteArray(w * w) }
        Polychrome.planesFromYuv(y, u, v, w, cw, 1, w, w, planes)
        return planes.map { Polychrome.decodePlane(it, w, w) }
    }

    @Test
    fun the_channels_survive_the_subsampling_at_a_generous_module() {
        val parts = (0 until 3).map { Polychrome.header(0, 0) + ByteArray(11) { b -> (it * 11 + b).toByte() } }
        val out = channelsThrough420(parts, scale = 8)
        for (ch in 0 until 3) {
            assertArrayEquals("channel $ch through 4:2:0 at 8 px a module", parts[ch], out[ch])
        }
    }

    /**
     * **And the number that matters**: the smallest module, in pixels, at
     * which all three channels still come back through the subsampling.
     * Printed rather than asserted at a fixed value — it is the figure the
     * hardware test is being set up to beat, and a monochrome symbol needs
     * about 2.5 pixels a module for comparison.
     */
    @Test
    fun the_module_a_colour_frame_needs_is_measured_and_reported() {
        val parts = (0 until 3).map { Polychrome.header(1, 0) + ByteArray(11) { b -> (it * 13 + b).toByte() } }
        var smallest = -1
        for (scale in 2..12) {
            val out = channelsThrough420(parts, scale)
            val all = (0 until 3).all { out[it] != null && out[it]!!.contentEquals(parts[it]) }
            if (all) { smallest = scale; break }
        }
        println("POLYCHROME: all three channels decode from ${smallest} px a module through 4:2:0")
        assertTrue("some module size works at all", smallest in 2..12)
    }
}
