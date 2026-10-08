package com.comptus.fueros

/**
 * **One §14.3 object across several codes** [author, 2026-10-04]. The
 * object a device shows at D2 — its `OpticalContribution` of about 2 KB,
 * then its `TranscriptConfirm` — is cut into parts of at most [chunk]
 * bytes, and each code carries one part under a five-byte header: how a
 * device puts the object on a screen is the shell's own and no part of
 * the wire format (`wire-format.md` §14.3 fixes the bytes, not the
 * symbol).
 *
 * **Each side shows what the other still lacks.** The header carries how
 * many of the other side's parts this device holds contiguously from the
 * first, and the tracking rows drawn under the symbol carry the full set
 * ([Tracking], [tracking]); from the two, a sender knows which of its
 * parts the other side is still owed and rotates through the lowest
 * [WINDOW] of them, a [turn] at a time on the screen's clock
 * (`Meet.TURN_MS`). The receiver fills holes in any order. [done] is both
 * sides holding everything.
 *
 * Pure Kotlin: the camera and the kernel are the caller's.
 */
class OpticalExchange(
    /** 0 for the contribution, 1 for the transcript: a stale code from the
     *  other exchange is not this one's. */
    val which: Int,
    mine: ByteArray,
    /** How many bytes go in one part. */
    val chunk: Int = CHUNK,
) {
    companion object {
        /** The header's version byte; another version reads as
         *  [Took.NOT_OURS]. */
        const val VERSION = 1
        /** The header's length: version, which, index, count, received. */
        const val HEADER = 5

        /**
         * **Parts of 20 bytes: 102 of them, at 29 modules** [author,
         * 2026-10-07]. Chosen for acquisition and not for range: on
         * hardware the range was the camera's (`QrCamera.middle`,
         * `QrCamera.briskest`) and the module size did not move it, while
         * the coarser symbol took the decode p90 from 1979 ms to 390 and
         * the watchdog restarts to none. Twenty is the last chunk at 29
         * modules (`OpticalChunkTest`); eight would reach 25 modules at
         * 253 parts, within two of the header's one-byte ceiling, so the
         * object could not grow. Aztec was measured against this and read
         * worse (`SymbologySurveyTest`); carrying less optically is
         * declined, since the key travels in the code and is pinned at
         * first contact (`wire-format.md` §14.3.1). `change-log.md`,
         * 2026-10-06 and 2026-10-07, carries the measurements.
         */
        const val CHUNK = 20

        /**
         * **How many of the parts they still lack are rotated through.**
         * Eight: wide enough that a reader missing most frames still has
         * several distinct parts offered, narrow enough that the rotation
         * returns to a part before long. The receiver fills holes in any
         * order, so the only cost of a wider window is a part waiting
         * longer for its turn.
         *
         * **Not tuned to the phones it was measured on** [author,
         * 2026-10-07]: lower-spec equipment has to be supported too, and a
         * value fitted to a camera catching a third of its frames would
         * fail silently on one catching a tenth — the exchange would
         * merely crawl.
         */
        const val WINDOW = 8

        /**
         * `bytes` in parts of at most `chunk`, as evenly as it divides:
         * the count is what a fixed chunk would give, and the remainder is
         * spread a byte at a time rather than left as a short tail.
         *
         * **Every part draws the same symbol.** A short last part encodes
         * to a smaller symbol, and the symbol's module count is what the
         * tracking rows are laid out against ([Tracking.cells]) — a reader
         * assuming the usual size then samples the wrong cells and believes
         * what it finds there. Run `68f1174-bitmap-1` ended that way, with
         * eleven parts owed and the sender sure none were.
         */
        fun evenly(bytes: ByteArray, chunk: Int): List<ByteArray> {
            if (bytes.isEmpty() || chunk <= 0) return listOf(ByteArray(0))
            val n = (bytes.size + chunk - 1) / chunk
            val base = bytes.size / n
            val over = bytes.size % n
            val out = ArrayList<ByteArray>(n)
            var at = 0
            for (i in 0 until n) {
                val len = base + if (i < over) 1 else 0
                out.add(bytes.copyOfRange(at, at + len))
                at += len
            }
            return out
        }

        /** [which] for the first exchange, the contribution. */
        const val CONTRIBUTION = 0
        /** [which] for the second, the transcript confirmation. */
        const val TRANSCRIPT = 1
    }

    /** What a code the camera read came to. */
    enum class Took {
        /** Not this exchange's: too short to carry a header, another
         *  version's, or another exchange's. */
        NOT_OURS,
        /** A part already held — the ordinary case, since every part is
         *  shown more than once; so is the next exchange's code, which
         *  says they hold everything. */
        DUPLICATE,
        /** A part this side needed, and more remain. */
        ACCEPTED,
        /** The last part this side needed: theirs is whole. */
        COMPLETE,
        /** This exchange's header, carrying figures that cannot be: no
         *  parts, an index past the count, a count that changed between
         *  codes, or more of mine held than this side has. */
        MALFORMED,
    }

    private val parts: List<ByteArray> = evenly(mine, chunk)

    /** How many parts this device shows. */
    val count: Int get() = parts.size

    private val lock = Any()
    private var theirCount = -1
    private var theirs: Array<ByteArray?> = emptyArray()
    /** Contiguous parts of theirs held, from the first: what the header
     *  carries, and all it has room to say. */
    private var got = 0
    /** How many of mine their last header said they hold, contiguously
     *  from the first: the floor under [theirHeld]. */
    private var theirGot = 0
    /**
     * **Which of this side's parts the other side holds**, part by part,
     * from their tracking rows. The header's count seeds it: a contiguous
     * count of N means parts 0 to N-1 are held, so a side that cannot read
     * the rows degrades to the header's word and never to a wrong belief.
     */
    private var theirHeld = BooleanArray(0)
    /** How many times each of this side's parts has been seen set in
     *  their rows, against `Tracking.CONFIRM` ([takeTracking]). */
    private var sightings = IntArray(0)
    /** Where the window's rotation stands ([showing]). */
    private var turn = 0

    /** The code to show now: the header, and the part at [showing]. */
    fun frame(): ByteArray = synchronized(lock) {
        byteArrayOf(VERSION.toByte(), which.toByte(), showing().toByte(), count.toByte(), got.toByte()) + parts[showing()]
    }

    /** The rotation advances a step; the screen calls this when a frame
     *  has been up long enough. */
    fun turn() = synchronized(lock) { turn += 1 }

    /** The index of the part [frame] shows: the window's current step, or
     *  the last part where nothing is owed. */
    fun showing(): Int = synchronized(lock) {
        val missing = owed()
        if (missing.isEmpty()) return minOf(theirGot, count - 1)
        missing[turn % missing.size]
    }

    /** **The parts the other side is known to lack**, up to [WINDOW] of
     *  them from the lowest — which is what the rotation walks. */
    private fun owed(): IntArray {
        val out = IntArray(minOf(WINDOW, count))
        var n = 0
        var i = 0
        while (i < count && n < out.size) {
            if (!holds(i)) out[n++] = i
            i++
        }
        return out.copyOf(n)
    }

    /** Whether the other side holds this side's part `i`: their rows
     *  where read, their header's contiguous count as the floor. */
    private fun holds(i: Int): Boolean =
        i < theirGot || (i < theirHeld.size && theirHeld[i])

    /**
     * **The tracking bitmap this side shows**: which of the *other* side's
     * parts are held here, so they know what is still owed. Empty until
     * one of their codes has been read, since until then there is no count
     * to describe.
     *
     * Drawn outside the symbol's error correction, which is safe because
     * it is re-shown every frame and a module only ever turns on: a module
     * misread as *unset* costs a redundant re-show, and one misread as
     * *set* would cost a part — so the reader confirms a set module before
     * it believes it ([takeTracking]).
     */
    fun tracking(): BooleanArray = synchronized(lock) {
        if (theirCount <= 0) return BooleanArray(0)
        BooleanArray(theirCount) { theirs[it] != null }
    }

    /**
     * Their tracking bitmap, as the camera sampled it. Monotonic: a part
     * once known held is never unknown again, so a dropped or misread
     * frame cannot walk the belief backwards. **A set module is believed
     * only after `Tracking.CONFIRM` sightings**: the rows carry no error
     * correction, and reading set when unset costs a part the counterparty
     * still needs, where reading unset when set costs one re-show.
     */
    fun takeTracking(bits: BooleanArray) = synchronized(lock) {
        if (theirHeld.size < count) theirHeld = theirHeld.copyOf(count)
        if (sightings.size < count) sightings = sightings.copyOf(count)
        val n = minOf(bits.size, count)
        for (i in 0 until n) {
            if (!bits[i] || theirHeld[i]) continue
            if (++sightings[i] >= Tracking.CONFIRM) theirHeld[i] = true
        }
    }

    /** How many contiguous parts of theirs this side holds. */
    fun received(): Int = synchronized(lock) { got }

    /** How many parts they say they have, or -1 before any code of theirs
     *  has been read. */
    fun theirCount(): Int = synchronized(lock) { theirCount }

    /** How many of this side's parts the other side holds: their rows
     *  where read, their header's contiguous count as the floor. */
    fun theirReceived(): Int = synchronized(lock) { (0 until count).count { holds(it) } }

    /**
     * A code the camera read. A header of another exchange, or none, is
     * not ours; a part already held is a duplicate; the last part this
     * side needed completes theirs. Their header's count of mine is taken
     * either way, since that is what moves the two on.
     */
    fun take(bytes: ByteArray): Took = synchronized(lock) {
        if (bytes.size < HEADER || bytes[0].toInt() != VERSION) return Took.NOT_OURS
        if (bytes[1].toInt() == which + 1) {
            // **a code of the next exchange is the other side saying it has
            // everything of this one**: it moved on only once it held all of
            // mine, and this side may not have read the header that said so
            // before it did. Without this the side that finishes second
            // waits for a header that is gone
            theirGot = count
            return Took.DUPLICATE
        }
        if (bytes[1].toInt() != which) return Took.NOT_OURS
        val index = bytes[2].toInt() and 0xff
        val n = bytes[3].toInt() and 0xff
        val gotOfMine = bytes[4].toInt() and 0xff
        if (n == 0 || index >= n) return Took.MALFORMED
        if (theirCount < 0) {
            theirCount = n
            theirs = arrayOfNulls(n)
        } else if (n != theirCount) {
            return Took.MALFORMED
        }
        if (gotOfMine > count) return Took.MALFORMED
        theirGot = maxOf(theirGot, gotOfMine)
        if (theirs[index] != null) return Took.DUPLICATE
        theirs[index] = bytes.copyOfRange(HEADER, bytes.size)
        while (got < theirCount && theirs[got] != null) got++
        if (got == theirCount) Took.COMPLETE else Took.ACCEPTED
    }

    /** The other side's object, once every part is held. */
    fun theirs(): ByteArray? = synchronized(lock) {
        if (theirCount <= 0 || got < theirCount) return null
        theirs.fold(ByteArray(0)) { acc, p -> acc + (p ?: ByteArray(0)) }
    }

    /** Both have everything: theirs is held here, and they are known to
     *  hold all of this side's. */
    fun done(): Boolean = synchronized(lock) {
        theirCount > 0 && got == theirCount && (0 until count).all { holds(it) }
    }

    /** Where the two stand, for the diagnostics. */
    fun status(): String {
        val shown = showing() + 1
        return synchronized(lock) {
            val theirsSoFar = if (theirCount < 0) "none of theirs yet" else "$got of $theirCount of theirs"
            "Showing part $shown of $count; received $theirsSoFar; they hold ${theirReceived()} of your $count."
        }
    }
}
