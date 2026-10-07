package com.comptus.fueros

/**
 * **The tracking rows: which parts this side holds, drawn under the
 * symbol and outside its error correction** [author, 2026-10-07].
 *
 * The header has room for one number and it is the *contiguous* count of
 * parts held, which cannot describe holes — so a sender rotating through
 * what it believes is owed wastes half its turns on parts already in
 * hand. Simulated at the measured frame rates that came out slower than
 * showing one part at a time (`WindowTest`). The full set has to travel,
 * and there is nowhere in a 25-byte symbol to put it.
 *
 * So it goes beside the symbol rather than inside it. **One module a
 * part**: set means held, and a module only ever turns on.
 *
 * **Why outside the error correction is acceptable.** The rows are
 * re-shown every frame, so a module misread *unset* costs one redundant
 * re-show and nothing else. A module misread *set* would cost a part —
 * the sender would stop offering something still needed — so a set module
 * is believed only after [CONFIRM] sightings, and the modules are drawn
 * at [SCALE] times the symbol's own so they are the easiest thing on the
 * screen to read.
 *
 * **And losing them entirely is safe.** The header still carries its
 * contiguous count, which is a floor under the bitmap
 * (`OpticalExchange.takeTracking`): a side that cannot read the rows
 * degrades to exactly the behaviour that shipped, not to a wrong belief.
 */
object Tracking {
    /** How many of the symbol's modules one tracking module spans. Two:
     *  the rows carry no error correction, so they are made the easiest
     *  thing on the screen to resolve. */
    const val SCALE = 2

    /** How many sightings of a set module before it is believed. One
     *  sighting of a *set* module costs a part if it is wrong; one of an
     *  unset module costs a re-show. So the costly direction is confirmed
     *  and the cheap one is not. */
    const val CONFIRM = 2

    /** How many tracking modules fit across a symbol `modules` wide. */
    fun across(modules: Int): Int = modules / SCALE

    /** How many rows of tracking modules `count` parts need. */
    fun rows(count: Int, modules: Int): Int {
        val w = across(modules)
        return if (w <= 0) 0 else (count + w - 1) / w
    }

    /** Where part `i` sits: its column and row among the tracking
     *  modules, left to right and top to bottom. */
    fun cellOf(i: Int, modules: Int): Pair<Int, Int> {
        val w = across(modules)
        return Pair(i % w, i / w)
    }

    /** A point in the camera's frame. */
    data class At(val x: Float, val y: Float)

    /**
     * **Where each tracking cell lands in the camera's frame**, from the
     * three finder centres the decoder already handed back
     * (`Result.getResultPoints`).
     *
     * This is why the rows need no bounding mark and no detector of their
     * own: a symbol that *decoded* has told us where its finders are, and
     * three finders fix a module basis. `modules` is the drawn matrix
     * including its quiet zone, `margin` the quiet zone each side.
     *
     * **Affine, not perspective.** Three points cannot give the fourth
     * degree of freedom, so tilt is uncorrected — acceptable because the
     * extrapolation is a few module-rows below a symbol that filled the
     * frame, and because a misread cell costs a re-show and not the
     * exchange ([CONFIRM], `OpticalExchange.takeTracking`).
     */
    fun cells(
        count: Int,
        modules: Int,
        margin: Int,
        topLeft: At,
        topRight: At,
        bottomLeft: At,
    ): List<At> {
        val symbol = modules - 2 * margin
        val span = (symbol - 7).toFloat()
        if (span <= 0f) return emptyList()
        // one module along each axis, from the distance between finder centres
        val ux = (topRight.x - topLeft.x) / span
        val uy = (topRight.y - topLeft.y) / span
        val vx = (bottomLeft.x - topLeft.x) / span
        val vy = (bottomLeft.y - topLeft.y) / span
        // the symbol's own (0,0) corner: the top-left finder's centre sits
        // three and a half modules into it on both axes
        val ox = topLeft.x - 3.5f * ux - 3.5f * vx
        val oy = topLeft.y - 3.5f * uy - 3.5f * vy
        fun at(col: Float, row: Float) = At(ox + col * ux + row * vx, oy + col * uy + row * vy)
        // the rows are drawn flush under the whole matrix, so their left
        // edge is the quiet zone's and their first row is the matrix's last
        val left = -margin.toFloat()
        val top = (modules - margin).toFloat()
        return (0 until count).map { i ->
            val (cx, cy) = cellOf(i, modules)
            at(left + (cx + 0.5f) * SCALE, top + (cy + 0.5f) * SCALE)
        }
    }

    /** A known-black and a known-white point of the same symbol, for the
     *  threshold the cells are judged against: the top-left finder's core
     *  is black and its quiet zone is white. */
    fun reference(modules: Int, margin: Int, topLeft: At, topRight: At, bottomLeft: At): Pair<At, At>? {
        val symbol = modules - 2 * margin
        val span = (symbol - 7).toFloat()
        if (span <= 0f) return null
        val ux = (topRight.x - topLeft.x) / span
        val uy = (topRight.y - topLeft.y) / span
        val vx = (bottomLeft.x - topLeft.x) / span
        val vy = (bottomLeft.y - topLeft.y) / span
        // a module out from the symbol's corner, into the quiet zone
        val white = At(
            topLeft.x - 4.5f * ux - 4.5f * vx,
            topLeft.y - 4.5f * uy - 4.5f * vy,
        )
        return Pair(topLeft, white)
    }

    /**
     * **The rows as pixels**, to be drawn directly below the symbol at the
     * same module scale. `modules` is the symbol's width in modules and
     * `scale` the pixels a symbol module takes, so the block comes out
     * `modules * scale` wide — flush with the symbol above it.
     */
    fun pixels(bits: BooleanArray, count: Int, modules: Int, scale: Int): Pair<IntArray, Int> {
        val w = modules * scale
        val h = rows(count, modules) * SCALE * scale
        val px = IntArray(w * h) { android.graphics.Color.WHITE }
        val cell = SCALE * scale
        for (i in 0 until count) {
            if (i >= bits.size || !bits[i]) continue
            val (cx, cy) = cellOf(i, modules)
            val x0 = cx * cell
            val y0 = cy * cell
            for (y in y0 until minOf(y0 + cell, h)) {
                for (x in x0 until minOf(x0 + cell, w)) px[y * w + x] = android.graphics.Color.BLACK
            }
        }
        return Pair(px, w)
    }
}
