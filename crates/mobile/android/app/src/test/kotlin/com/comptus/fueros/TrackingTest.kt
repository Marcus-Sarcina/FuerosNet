package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **The tracking rows' geometry, round-tripped through a rendered frame.**
 *
 * The sampler takes its basis from the three finder centres the decoder
 * hands back, so the thing that can be silently wrong is the arithmetic
 * between those points and the cells below the symbol. This renders the
 * rows where the screen would, computes the finder centres where the
 * drawing puts them, and asks the sampler to read the bits back.
 */
class TrackingTest {

    private val modules = 29
    private val margin = Optical.MARGIN
    private val scale = 8

    /** The finder centres of a symbol drawn at this scale, in the pixels
     *  of an image whose origin is the drawn matrix's top-left. */
    private fun finders(): Triple<Tracking.At, Tracking.At, Tracking.At> {
        val symbol = modules - 2 * margin
        fun at(col: Float, row: Float) =
            Tracking.At((margin + col) * scale, (margin + row) * scale)
        val tl = at(3.5f, 3.5f)
        val tr = at(symbol - 3.5f, 3.5f)
        val bl = at(3.5f, symbol - 3.5f)
        return Triple(tl, tr, bl)
    }

    /**
     * **Every part of the real object must draw the same symbol.** The
     * tracking rows are laid out against the symbol's module count, so a
     * part that encodes smaller moves the grid under a reader that
     * assumes otherwise — which is how run `68f1174-bitmap-1` ended with
     * one side believing the other held 102 parts when it held 91.
     *
     * The cause was a short tail: 2,022 bytes chunked at 20 is 101 parts
     * of twenty and one of **two**, and two bytes is a smaller symbol.
     * `OpticalExchange.evenly` spreads the remainder instead.
     */
    @Test
    fun every_part_of_the_contribution_draws_the_same_symbol() {
        val contribution = ByteArray(2022) { (it * 31 % 251).toByte() }
        val split = OpticalExchange.evenly(contribution, OpticalExchange.CHUNK)
        val sizes = split.mapIndexed { i, part ->
            // the frame for part i, as the screen would draw it
            val frame = byteArrayOf(
                OpticalExchange.VERSION.toByte(), OpticalExchange.CONTRIBUTION.toByte(),
                i.toByte(), split.size.toByte(), 0,
            ) + part
            Optical.matrix(frame).width
        }.toSet()
        assertEquals("one symbol size for every part, got $sizes", 1, sizes.size)
    }

    /** And the partition is still the object: nothing lost, nothing
     *  duplicated, and no part longer than the chunk. */
    @Test
    fun the_even_partition_is_the_object() {
        for (size in listOf(1, 19, 20, 21, 2022, 4095)) {
            val obj = ByteArray(size) { (it * 7).toByte() }
            val parts = OpticalExchange.evenly(obj, OpticalExchange.CHUNK)
            assertEquals(
                "count matches a fixed chunk's",
                (size + OpticalExchange.CHUNK - 1) / OpticalExchange.CHUNK,
                parts.size,
            )
            assertTrue("no part over the chunk", parts.all { it.size <= OpticalExchange.CHUNK })
            assertTrue("within a byte of each other", (parts.maxOf { it.size } - parts.minOf { it.size }) <= 1)
            org.junit.Assert.assertArrayEquals(obj, parts.fold(ByteArray(0)) { a, b -> a + b })
        }
    }

    /** The marks read as themselves where the grid is right, and a reader
     *  at the wrong pitch sees them wrong — which is the whole point of
     *  them. */
    @Test
    fun the_marks_catch_a_grid_read_at_the_wrong_pitch() {
        val count = 102
        val held = BooleanArray(count) { it < 40 }
        val (px, w) = Tracking.pixels(held, count, modules, scale)
        val h = Tracking.rows(count, modules) * Tracking.SCALE * scale
        val (tl, tr, bl) = finders()
        fun sampleAt(assumed: Int): Pair<Int, Boolean> {
            val cells = Tracking.cells(count, assumed, margin, tl, tr, bl)
            var set = 0
            var marksOk = true
            for (i in 0 until Tracking.slots(count)) {
                val x = Math.round(cells[i].x)
                val yy = Math.round(cells[i].y) - modules * scale
                val on = if (yy in 0 until h && x in 0 until w) px[yy * w + x] == android.graphics.Color.BLACK else false
                if (i < count) { if (on) set++ } else if (on != Tracking.markAt(count, i)) marksOk = false
            }
            return Pair(set, marksOk)
        }
        val (right, rightMarks) = sampleAt(modules)
        assertEquals("the right pitch reads what was drawn", 40, right)
        assertTrue("and its marks read as themselves", rightMarks)
        val (_, wrongMarks) = sampleAt(25)
        assertTrue("a 25-module assumption is caught by the marks", !wrongMarks)
    }

    @Test
    fun the_layout_is_what_the_part_count_needs() {
        assertEquals("fourteen cells across a 29-module symbol", 14, Tracking.across(29))
        assertEquals("102 parts and two marks in eight rows", 8, Tracking.rows(102, 29))
        assertEquals(Pair(0, 0), Tracking.cellOf(0, 29))
        assertEquals(Pair(13, 0), Tracking.cellOf(13, 29))
        assertEquals(Pair(0, 1), Tracking.cellOf(14, 29))
    }

    /** **Every cell is read back exactly as it was drawn.** The test that
     *  would catch an off-by-one in the basis, the margin or the row
     *  offset — each of which would silently report the wrong parts held. */
    @Test
    fun the_cells_read_back_as_they_were_drawn() {
        val count = 102
        val drawn = BooleanArray(count) { it % 3 == 0 || it == 101 }
        val (px, w) = Tracking.pixels(drawn, count, modules, scale)
        val rows = Tracking.rows(count, modules)
        val h = rows * Tracking.SCALE * scale
        val (tl, tr, bl) = finders()
        val cells = Tracking.cells(count, modules, margin, tl, tr, bl)
        assertEquals("the bitmap and its marks", Tracking.slots(count), cells.size)
        // the cells are below the symbol, so their y is past the matrix
        val top = modules * scale
        for (i in 0 until Tracking.slots(count)) {
            val x = Math.round(cells[i].x)
            val y = Math.round(cells[i].y) - top
            assertTrue("cell $i at ${cells[i].x},${cells[i].y} is inside the block", y in 0 until h && x in 0 until w)
            val black = px[y * w + x] == android.graphics.Color.BLACK
            val want = if (i < count) drawn[i] else Tracking.markAt(count, i)
            assertEquals("cell $i", want, black)
        }
    }

    /** The reference points land where their colours are: the finder's
     *  core is black and the quiet zone white, which is the threshold the
     *  cells are judged against. */
    @Test
    fun the_reference_points_are_a_black_and_a_white_of_the_same_symbol() {
        val (tl, tr, bl) = finders()
        val (black, white) = Tracking.reference(modules, margin, tl, tr, bl)!!
        assertEquals("black is the top-left finder's centre", tl, black)
        // white is out in the quiet zone: inside the drawn matrix, outside
        // the symbol
        assertTrue("white x ${white.x} is in the margin", white.x >= 0f && white.x < margin * scale)
        assertTrue("white y ${white.y} is in the margin", white.y >= 0f && white.y < margin * scale)
    }

    /** A basis from a symbol that is scaled and offset still lands on the
     *  right cells: the sampler sees the code wherever it is in frame. */
    @Test
    fun the_basis_survives_the_symbol_being_elsewhere_in_the_frame() {
        val count = 40
        val drawn = BooleanArray(count) { it % 2 == 0 }
        val (tl, tr, bl) = finders()
        fun moved(a: Tracking.At) = Tracking.At(a.x * 1.5f + 300f, a.y * 1.5f + 120f)
        val plain = Tracking.cells(count, modules, margin, tl, tr, bl)
        val shifted = Tracking.cells(count, modules, margin, moved(tl), moved(tr), moved(bl))
        for (i in 0 until count) {
            assertEquals("cell $i x", plain[i].x * 1.5f + 300f, shifted[i].x, 0.01f)
            assertEquals("cell $i y", plain[i].y * 1.5f + 120f, shifted[i].y, 0.01f)
        }
        assertTrue(drawn.isNotEmpty())
    }
}
