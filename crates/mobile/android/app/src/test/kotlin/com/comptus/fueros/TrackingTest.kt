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

    @Test
    fun the_layout_is_what_the_part_count_needs() {
        assertEquals("fourteen cells across a 29-module symbol", 14, Tracking.across(29))
        assertEquals("102 parts in eight rows", 8, Tracking.rows(102, 29))
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
        assertEquals(count, cells.size)
        // the cells are below the symbol, so their y is past the matrix
        val top = modules * scale
        for (i in 0 until count) {
            val x = Math.round(cells[i].x)
            val y = Math.round(cells[i].y) - top
            assertTrue("cell $i at ${cells[i].x},${cells[i].y} is inside the block", y in 0 until h && x in 0 until w)
            val black = px[y * w + x] == android.graphics.Color.BLACK
            assertEquals("cell $i", drawn[i], black)
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
