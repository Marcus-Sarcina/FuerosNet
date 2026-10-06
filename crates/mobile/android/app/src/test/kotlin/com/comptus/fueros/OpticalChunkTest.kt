package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **What the chunk size costs and buys**, pinned so a change to it shows
 * its price (`OpticalExchange.CHUNK`).
 *
 * **The figure is the module count, not a pixel count** [author,
 * 2026-10-06]: a code is drawn at the screen's width, so its module is a
 * fixed fraction of that width, and physical width varies far less across
 * phones than resolution does. The field runs of 2026-10-06 calibrate it
 * — a 69-module code read to about 18 inches on a 70 mm screen, so a code
 * reads to about `450 / modules` times its own width — and these
 * assertions are the encoder's own output, so a chunk changed without
 * reading `CHUNK`'s table fails here.
 */
class OpticalChunkTest {

    /** A contribution's size: a full `KeyMaterial` and a 16-byte
     *  contribution, as the field runs' `cer.optical.shown` reported. */
    private val contribution = ByteArray(2022) { (it * 31 % 251).toByte() }

    @Test
    fun the_chosen_chunk_yields_the_symbol_the_range_was_chosen_for() {
        val x = OpticalExchange(OpticalExchange.CONTRIBUTION, contribution)
        assertEquals("parts at CHUNK = ${OpticalExchange.CHUNK}", 27, x.count)
        assertEquals(
            "the symbol's modules, which fix the module as a fraction of the " +
                "code's width and so the range",
            41,
            Optical.matrix(x.frame()).width,
        )
    }

    /** **The chosen chunk leaves the object room to grow**, which the
     *  next step down does not: 25 modules needs 8-byte parts and 253 of
     *  them, against a ceiling of 255. */
    /** **76 is the last chunk at 41 modules**, which is what makes it the
     *  cheapest point at the best range the 25-to-35-part band can reach:
     *  one byte more crosses into 45 modules and loses an inch of range,
     *  and the fewer parts below it buy nothing [author, 2026-10-06]. */
    @Test
    fun the_chosen_chunk_is_the_last_one_at_its_module_count() {
        fun modules(chunk: Int) =
            Optical.matrix(OpticalExchange(OpticalExchange.CONTRIBUTION, contribution, chunk).frame()).width
        assertEquals("at CHUNK", 41, modules(OpticalExchange.CHUNK))
        assertEquals(
            "one byte more is a denser symbol",
            45,
            modules(OpticalExchange.CHUNK + 1),
        )
    }

    @Test
    fun the_chosen_chunk_leaves_headroom_under_the_header_s_ceiling() {
        val ceiling = 255 * OpticalExchange.CHUNK
        assertTrue(
            "an object of ${contribution.size} bytes against a ceiling of $ceiling",
            ceiling >= contribution.size * 2,
        )
    }

    @Test
    fun the_header_s_one_byte_fields_cap_the_part_count() {
        // index, count and received are a byte each, so a chunk small
        // enough to need more than 255 parts cannot be carried at all
        val smallest = (contribution.size + 254) / 255
        assertTrue("a part of $smallest bytes is the floor", smallest > 0)
        val x = OpticalExchange(OpticalExchange.CONTRIBUTION, contribution, smallest)
        assertTrue("${x.count} parts fits the header", x.count <= 255)
        val tooMany = OpticalExchange(OpticalExchange.CONTRIBUTION, contribution, smallest - 1)
        assertTrue("${tooMany.count} parts does not", tooMany.count > 255)
    }

    @Test
    fun a_lower_density_symbol_is_what_more_parts_buys() {
        // the relationship the table rests on: fewer bytes per code, fewer
        // modules, a larger module at the same screen width
        var previous = Int.MAX_VALUE
        for (chunk in listOf(256, 128, 64, 48, 32)) {
            val x = OpticalExchange(OpticalExchange.CONTRIBUTION, contribution, chunk)
            val modules = Optical.matrix(x.frame()).width
            assertTrue(
                "chunk $chunk gives $modules modules, not fewer than $previous",
                modules < previous,
            )
            previous = modules
        }
    }
}
