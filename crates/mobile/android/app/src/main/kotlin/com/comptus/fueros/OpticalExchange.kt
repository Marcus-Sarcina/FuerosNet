package com.comptus.fueros

/**
 * **One §14.3 object across several codes, in step with the other side**
 * [author, 2026-10-04]. The object a device shows at D2 — its
 * `OpticalContribution` of about 2 KB, then its `TranscriptConfirm` — is
 * cut into parts of at most [chunk] bytes, and each code carries one part
 * under a small header: how the device encodes the object on a screen is
 * the shell's own and no part of the wire format (`wire-format.md` §14.3
 * fixes the bytes, not the symbol).
 *
 * **Lockstep.** A device shows the part the other side needs next: the
 * index is how many of this device's parts the other side's last header
 * said it holds. The header also carries how many of the other side's
 * parts this device holds, which is what lets the other side advance. So
 * each screen changes as the other side reads it, nobody skips a part, and
 * both keep their last part up until the other's header says it has them
 * all; [done] is that, both ways. A smaller code reads at arm's length
 * where one dense code did not, and a screen that changes every second or
 * so tells the person that something is happening.
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
         * **Parts of 76 bytes: 27 of them, for a five-minute ceremony**
         * [author, 2026-10-06].
         *
         * **The quantity that matters is the module count, and nothing
         * here is in pixels** [author, 2026-10-06]. A code is drawn at the
         * screen's width, so its module is that width divided by the
         * symbol's modules; phone *physical* width varies far less across
         * models and generations than resolution does, so modules per
         * screen-width code is the figure that carries over and a pixel
         * count is not. Calibrated on the field runs of 2026-10-06 — a
         * 69-module code read to about 18 inches on a 70 mm screen, so a
         * module of 1.0 mm read to 450 of itself — the rule is:
         *
         * **a code reads at about `450 / modules` times its own width.**
         *
         * Measured with the real encoder over a 2,022-byte contribution,
         * and the boundaries are exact rather than sampled:
         *
         * | chunk | parts | modules | reads at | on a 70 mm screen |
         * |---|---|---|---|---|
         * | 256 | 8 | 69 | 6.5 × width | 18 in |
         * | 128 | 16 | 53 | 8.5 × | 23 in |
         * | 92–96 | 22 | 45 | 10 × | 28 in |
         * | 77–91 | 23–27 | 45 | 10 × | 28 in |
         * | **76** | **27** | **41** | **11 ×** | **30 in** |
         * | 56–75 | 28–37 | 41 | 11 × | 30 in |
         * | 48 | 43 | 37 | 12 × | 34 in |
         * | 20 | 102 | 29 | 15.5 × | 43 in |
         * | 8 | 253 | 25 | 18 × | 50 in |
         *
         * **Why 76 and not any other chunk in the 25-to-35-part band**
         * [author, 2026-10-06]: 41 modules is the best range the band can
         * reach — 37 modules needs 43 parts — and it holds from chunk 56
         * all the way to 76, where the next byte crosses into 45. So 76 is
         * the **cheapest** way to the band's best range, and spending the
         * band's remaining parts buys nothing at all: 32 parts at chunk 64
         * reads no further than 27 at chunk 76.
         *
         * **The five minutes it is chosen for.** Run 4 of 2026-10-06
         * measured 43.0 s from `cer.begin` to the witness request with
         * eight parts, and a median 0.5–0.6 s a part; nineteen more parts
         * is about ten seconds, so the request goes out near 53 s. The
         * witnesses' four-minute floor runs from *their* receipt of it
         * (design §7.1), which puts the record at about **4 min 55 s**
         * after begin, before the permission prompts and the proximity tap
         * that precede it.
         *
         * **Error correction stays at M.** Measured, L buys about a fifth
         * fewer parts at the same module count and nothing in range, which
         * is the dimension that was wanted; at the resolution margin the
         * trade is two-sided anyway, since marginal modules produce the bit
         * errors that correction is what recovers from. The reasoning for M
         * in `Optical` stands.
         *
         * **Four to six feet is not this lever's**, at any part count the
         * header's one-byte fields admit: 25 modules is its floor and
         * costs 253 parts of the 255 available.
         *
         * **Carrying less optically is declined** [author, 2026-10-06]: a
         * 32-byte commitment in the code with the key material following
         * on the bearer would reach four feet in one symbol, and the key
         * travels in the code at the meeting and is pinned at first
         * contact (`wire-format.md` §14.3.1, design §12.3). That is
         * settled and not an avenue.
         *
         * What is left is carrying **more per frame**: three ordinary QRs,
         * one per colour channel, each channel an independent part
         * ([Polychrome]), which divides the part count by three and
         * degrades to monochrome when a camera cannot separate them.
         */
        const val CHUNK = 76

        /**
         * **The part size when a frame carries three of them** — eleven
         * bytes, which under [Polychrome]'s two-byte header is thirteen
         * and so the smallest QR there is: 25 modules with its quiet zone,
         * reading at about 18 × the code's own width, which on a 70 mm
         * screen is about 50 inches. Three a frame carries the 2,022-byte
         * contribution in 62 frames, or about 35 s at the field runs'
         * measured rate [author, 2026-10-06].
         *
         * **Frame 0 carries full headers**, so it is 16 bytes a channel
         * and one symbol larger — 29 modules — which makes the frame that
         * decides whether colour works the easiest of them to read, and
         * carries the part count the compressed headers leave out. If
         * fewer than three of its channels arrive, the sender falls back
         * to [CHUNK] and the three parts it carried are re-sent under the
         * monochrome partitioning.
         */
        const val CHUNK_POLY = 11
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
        /** A part already held.  The other side shows it until it sees
         *  this side's progress, so this is the ordinary case; so is the
         *  next exchange's code, which says they hold everything. */
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

    private val parts: List<ByteArray> = mine.toList().chunked(chunk).map { it.toByteArray() }.ifEmpty { listOf(ByteArray(0)) }

    /** How many parts this device shows. */
    val count: Int get() = parts.size

    private val lock = Any()
    private var theirCount = -1
    private var theirs: Array<ByteArray?> = emptyArray()
    /** Contiguous parts of theirs held, from the first. */
    private var got = 0
    /** How many of mine their last header said they hold. */
    private var theirGot = 0

    /** The code to show now: the part they need next, carrying my progress
     *  through theirs. */
    fun frame(): ByteArray = synchronized(lock) {
        val index = minOf(theirGot, count - 1)
        byteArrayOf(VERSION.toByte(), which.toByte(), index.toByte(), count.toByte(), got.toByte()) + parts[index]
    }

    /**
     * **A colour frame's three parts**, from the part the other side needs
     * next: red carries it, green the one after, blue the one after that
     * (`Polychrome`). Short at the end of the object, where fewer than
     * three remain.
     *
     * `compressed` is false for the frame that decides whether colour
     * works — it carries full headers, so each channel is a self-standing
     * part and one of them brings the count — and true for every frame
     * after it.
     */
    fun colourFrames(compressed: Boolean): List<ByteArray> = synchronized(lock) {
        val first = minOf(theirGot, count - 1)
        val frame = first / Polychrome.CHANNELS
        (0 until Polychrome.CHANNELS).mapNotNull { ch ->
            val i = frame * Polychrome.CHANNELS + ch
            if (i >= count) {
                null
            } else if (compressed) {
                Polychrome.header(frame, got) + parts[i]
            } else {
                byteArrayOf(VERSION.toByte(), which.toByte(), i.toByte(), count.toByte(), got.toByte()) + parts[i]
            }
        }
    }

    /** The index of the part [frame] shows, for the screen and the events. */
    fun showing(): Int = synchronized(lock) { minOf(theirGot, count - 1) }

    /** How many contiguous parts of theirs this side holds. */
    fun received(): Int = synchronized(lock) { got }

    /** How many parts they say they have, or -1 before any code of theirs
     *  has been read. */
    fun theirCount(): Int = synchronized(lock) { theirCount }

    /** How many of mine their last header said they hold. */
    fun theirReceived(): Int = synchronized(lock) { theirGot }

    /**
     * A code the camera read. A header of another exchange, or none, is
     * not ours; a part already held is a duplicate (the other side keeps
     * showing it until it sees this side's progress); the last part this
     * side needed completes theirs. Their header's count of mine is taken
     * either way, since that is what moves the two on.
     */
    fun take(bytes: ByteArray, channel: Int = 0): Took = synchronized(lock) {
        // **a compressed header carries no `which` and no count**
        // (`Polychrome`): the channel it arrived in is its index within the
        // frame, and the count came with the full headers of frame 0. One
        // that arrives before that frame did is not ours to place.
        if (Polychrome.compressed(bytes)) {
            if (bytes.size <= Polychrome.HEADER) return Took.MALFORMED
            if (theirCount < 0) return Took.NOT_OURS
            val index = Polychrome.frameOf(bytes) * Polychrome.CHANNELS + channel
            val gotOfMineC = Polychrome.gotOf(bytes)
            if (index >= theirCount || gotOfMineC > count) return Took.MALFORMED
            theirGot = maxOf(theirGot, gotOfMineC)
            if (theirs[index] != null) return Took.DUPLICATE
            theirs[index] = bytes.copyOfRange(Polychrome.HEADER, bytes.size)
            while (got < theirCount && theirs[got] != null) got++
            return if (got == theirCount) Took.COMPLETE else Took.ACCEPTED
        }
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
            // **a count that changed is the other side re-partitioning**,
            // which is what a fall back from colour to monochrome is: the
            // parts already held are of the old partitioning and are no
            // use, so they go [author, 2026-10-06]. It was MALFORMED when
            // one partitioning was all there was. Nothing rests on
            // trusting it — the assembled object is checked against the
            // screens either way (`wire-format.md` §14.3.2), and a wrong
            // re-partition simply fails to assemble.
            theirCount = n
            theirs = arrayOfNulls(n)
            got = 0
        }
        if (gotOfMine > count) return Took.MALFORMED
        theirGot = maxOf(theirGot, gotOfMine)
        if (theirs[index] != null) return Took.DUPLICATE
        theirs[index] = bytes.copyOfRange(HEADER, bytes.size)
        while (got < theirCount && theirs[got] != null) got++
        if (got == theirCount) Took.COMPLETE else Took.ACCEPTED
    }

    /** Whether every part of theirs is held. */
    fun complete(): Boolean = synchronized(lock) { theirCount > 0 && got == theirCount }

    /** The other side's object, once every part is held. */
    fun theirs(): ByteArray? = synchronized(lock) {
        if (theirCount <= 0 || got < theirCount) return null
        theirs.fold(ByteArray(0)) { acc, p -> acc + (p ?: ByteArray(0)) }
    }

    /** Both have everything: theirs is held here, and their last header
     *  said they hold all of mine. */
    fun done(): Boolean = synchronized(lock) { theirCount > 0 && got == theirCount && theirGot >= count }

    /** Where the two stand, for the screen. */
    fun status(): String = synchronized(lock) {
        val shown = minOf(theirGot, count - 1) + 1
        val theirsSoFar = if (theirCount < 0) "none of theirs yet" else "$got of ${theirCount} of theirs"
        "Showing part $shown of $count; received $theirsSoFar; they hold $theirGot of your $count."
    }
}
