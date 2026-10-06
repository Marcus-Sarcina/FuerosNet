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
        assertEquals(4, mono.count)
        assertEquals(OpticalExchange.Took.ACCEPTED, b.take(mono.frame()))
        assertEquals("the old partitioning's parts went with it", 1, b.received())
        assertEquals(4, b.theirCount())
        var guard = 0
        while (!b.complete() && guard++ < 8) {
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
