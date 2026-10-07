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
 * all; [done] is that, both ways. A smaller code is acquired in a
 * fraction of the time one dense code took — and a screen that changes
 * every second or so tells the person that something is happening.
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
         * **Parts of 20 bytes: 102 of them, for acquisition** — and *not*
         * for read distance, which this does not buy.
         *
         * **The quantity that matters is the module count, and nothing
         * here is in pixels** [author, 2026-10-06]. A code is drawn at the
         * screen's width, so its module is that width divided by the
         * symbol's modules; phone *physical* width varies far less across
         * models and generations than resolution does, so modules per
         * screen-width code is the figure that carries over and a pixel
         * count is not. That part holds.
         *
         * **What does not hold: `450 / modules` as a read distance.** A
         * rule was fitted here to one observation — a 69-module code read
         * to about 18 inches on a 70 mm screen — and used to predict
         * range. **Three hardware runs have contradicted it:**
         *
         * | run | modules | module | measured |
         * |---|---|---|---|
         * | 2026-10-06 | 69 | 0.97 mm | ~18 in |
         * | `mono102`, 2026-10-07 | 29 | 2.14 mm | ~12 in |
         * | `aimed-1`, 2026-10-07 | 29 | 2.14 mm | **~24 in** |
         *
         * The module grew 2.2-fold between the first two rows and the
         * distance *fell*; the third row is the same symbol as the second
         * and reads twice as far, because the camera was told where to
         * focus and meter (`QrCamera.middle`). **Within 1.0 to 2.1 mm the
         * module did not determine the read distance and the camera's
         * control loop did** [measured, 2026-10-07]. At 12 inches a
         * 2.14 mm module already lands about eight pixels a module, four
         * times what a decoder needs, so pixel resolution was never the
         * binding constraint and the rule was fitting noise.
         *
         * No distance is predicted here any more, and `qr.shown` no
         * longer reports one. A figure wrong in the optimistic direction
         * sent two build-and-run cycles after the wrong variable.
         *
         * **What a coarser symbol does buy, measured: acquisition.**
         * Against chunk 76's 27 parts of 41 modules, on the same phones
         * and the same object:
         *
         * | | 27 × 41 modules | 102 × 29 modules |
         * |---|---|---|
         * | optical pass | 83.6 s | **57.3 s** |
         * | a part | 3.1 s | **0.56 s** |
         * | decode median | 365 ms | 340 ms |
         * | decode p90 | 1979 ms | **390 ms** |
         * | watchdog restarts | 3 | **0** |
         *
         * Four times the parts and **26 s faster end to end**. The p90
         * collapsing and the restarts going to zero are the same fact: at
         * 41 modules a real fraction of reads were marginal and each
         * stall cost twenty seconds, and at 29 almost none are. So the
         * parts are cheap and the margin is what they buy — which is a
         * better reason for this chunk than the one it was chosen for.
         *
         * **Why not 8, which is coarser still.** Not a judgement call:
         * 253 parts is within two of the header's ceiling, since [index]
         * and the count are a byte each, so the object could not grow.
         * Widening them does not help, because a 25-module symbol holds
         * exactly thirteen bytes — the five-byte header and eight of
         * payload — so a seven-byte header spills into 29 modules.
         *
         * **What this gives up.** The 25-to-35-part band of 2026-10-06,
         * deliberately [author, 2026-10-07]. And the four-to-six-foot
         * target is not reached: 24 inches is where `aimed-1` landed, and
         * the lever that moved it was the camera. The exposure is the next
         * one — that run metered 42 to 66 ms, which smears a handheld read
         * of millimetre features whatever the symbol is.
         *
         * **What it costs in time.** At the measured 0.56 s a part the
         * optical pass is about **57 s**. The witnesses' four-minute floor
         * runs from *their* receipt of the witness request (design §7.1)
         * and the request goes out after the pass, so the floor is what
         * sets the record's time either way.
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
         * **Aztec was measured against this and rejected** [2026-10-07].
         * It carries 2.6 times the payload at the same module and reads to
         * 22 inches against QR's 42, with decode p90 at 2,561 ms against
         * 466 — and the pass came out *longer* for carrying less, twice
         * over. A quiet zone made no difference. `SymbologySurveyTest`
         * carries the figures and the reason: the per-part cost is a
         * lockstep round-trip and not a decode, so payload a frame is the
         * wrong term to attack.
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
        const val CHUNK = 20

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
        /**
         * **How many of the parts they cannot have yet are rotated
         * through.** Eight: wide enough that a reader missing most frames
         * still has several distinct parts offered to it, narrow enough
         * that the rotation returns to a part before the reader has
         * forgotten why it wanted it. The receiver fills holes in any
         * order, so the only cost of a wider window is that a part waits
         * longer for its turn.
         */
        const val WINDOW = 8

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
    /** Contiguous parts of theirs held, from the first: what the header
     *  carries, and all the header has room to say. */
    private var got = 0
    /** How many of mine their last header said they hold, contiguously
     *  from the first: the floor under [theirHeld]. */
    private var theirGot = 0
    /**
     * **Which of this side's parts the other side holds**, part by part.
     *
     * The header has room for one number and it is the *contiguous* count
     * — so a receiver holding parts 0, 1, 2, 5 and 7 reports three, and
     * the sender cannot tell 5 and 7 from the holes. That is why rotating
     * a window over `theirGot` upward does not work: half its turns go to
     * parts already held. Simulated at the measured frame rates it came
     * out *slower* than showing one part at a time.
     *
     * So the full set travels, in tracking rows drawn below the symbol and
     * outside its error correction ([tracking]). **The header's count
     * still seeds this**, which is what makes the rows safe to lose: a
     * contiguous count of N means parts 0 to N-1 are held, so a side that
     * cannot read the rows degrades to exactly the old behaviour rather
     * than to a wrong belief.
     */
    private var theirHeld = BooleanArray(0)
    /** How many times each of this side's parts has been *seen* set in
     *  their rows, against `Tracking.CONFIRM` ([takeTracking]). */
    private var sightings = IntArray(0)
    /** Where the window's rotation stands ([frame]). */
    private var turn = 0

    /**
     * **The code to show now, rotating through a window of the parts they
     * cannot have yet.**
     *
     * It used to be exactly one part — `min(theirGot, count - 1)` — held
     * up until their header reported progress. That is what made the
     * exchange cost a round-trip a part, and `qr.looks` of 2026-10-07
     * measured the waste: **85 to 93% of the frames this side decoded
     * successfully were the same part over again**, while a third to three
     * quarters of all camera frames decoded fine. The reading was never
     * the problem; the waiting was.
     *
     * **The window needs nothing new on the wire.** `theirGot` is a
     * *contiguous* count, so every part from it upward is one they cannot
     * have — the sender may show any of them and the receiver fills holes
     * out of order, its contiguous count jumping as runs complete. So the
     * rotation is bounded by [WINDOW] and not by what the header can
     * describe, and the tracking rows that would replace the header's
     * single byte are a later refinement rather than a prerequisite.
     *
     * **How long each part is shown for belongs to the screen**, not
     * here: one [turn] is one part, and the screen ticks slowly enough
     * that a reader missing most frames still catches each one
     * (`Meet.TURN_MS`).
     */
    fun frame(): ByteArray = synchronized(lock) {
        byteArrayOf(VERSION.toByte(), which.toByte(), showing().toByte(), count.toByte(), got.toByte()) + parts[showing()]
    }

    /** The rotation advances a step; the screen calls this when it has
     *  shown a frame long enough. */
    fun turn() = synchronized(lock) { turn += 1 }

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
     *
     * **A short frame at the object's end goes out under full headers
     * even when the exchange is compressed.** The sender has to put
     * something in every channel, and the presentation pads a short frame
     * by repeating the last part it was given — but a compressed header
     * says only which *frame* a part belongs to, and the reader takes the
     * index from the channel it arrived in. So a repeated part read in
     * the next channel along decodes as the part after the last one,
     * which does not exist, and the reader stops the meeting on it. Run 2
     * of 2026-10-07 died exactly there, one part short of 184 with
     * `"a code of the exchange did not read as one"`: 184 is 61 × 3 + 1,
     * so its final frame carried one real part and two poisoned copies.
     * Under full headers the copies carry their own true index and read
     * as the duplicates they are.
     */
    fun colourFrames(compressed: Boolean): List<ByteArray> = synchronized(lock) {
        val first = minOf(theirGot, count - 1)
        val frame = first / Polychrome.CHANNELS
        val base = frame * Polychrome.CHANNELS
        val short = base + Polychrome.CHANNELS > count
        (0 until Polychrome.CHANNELS).mapNotNull { ch ->
            val i = base + ch
            if (i >= count) {
                null
            } else if (compressed && !short) {
                Polychrome.header(frame, got) + parts[i]
            } else {
                byteArrayOf(VERSION.toByte(), which.toByte(), i.toByte(), count.toByte(), got.toByte()) + parts[i]
            }
        }
    }

    /** The index of the part [frame] shows, for the screen and the events:
     *  the window's current step, dwelling [DWELL] turns on each. */
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

    /** Whether the other side holds this side's part `i`: their bitmap
     *  where it has been read, and their header's contiguous count as the
     *  floor under it. */
    private fun holds(i: Int): Boolean =
        i < theirGot || (i < theirHeld.size && theirHeld[i])

    /**
     * **The tracking bitmap this side shows**: which of the *other* side's
     * parts are held here, so they know what is still owed. Empty until
     * one of their codes has been read, since until then there is no count
     * to describe.
     *
     * Drawn outside the symbol's error correction, which is safe because
     * it is re-shown every frame and because a module only ever turns on:
     * a module misread as *unset* costs a redundant re-show, and one
     * misread as *set* would cost a part — so the reader confirms a set
     * module before it believes it ([takeTracking]).
     */
    fun tracking(): BooleanArray = synchronized(lock) {
        if (theirCount <= 0) return BooleanArray(0)
        BooleanArray(theirCount) { theirs[it] != null }
    }

    /**
     * Their tracking bitmap, as the camera sampled it. Monotonic: a part
     * once known held is never unknown again, so a dropped or misread
     * frame cannot walk the belief backwards.
     */
    fun takeTracking(bits: BooleanArray) = synchronized(lock) {
        if (theirHeld.size < count) theirHeld = theirHeld.copyOf(count)
        if (sightings.size < count) sightings = sightings.copyOf(count)
        val n = minOf(bits.size, count)
        for (i in 0 until n) {
            if (!bits[i] || theirHeld[i]) continue
            // **a set module is believed only after `Tracking.CONFIRM`
            // sightings.** The rows carry no error correction, and the two
            // directions of misreading are not equally cheap: read unset
            // when set costs one redundant re-show, read set when unset
            // costs a part the counterparty still needs and will never be
            // offered again. So the costly direction is confirmed and the
            // cheap one is not. The rows are re-shown every frame, so a
            // module genuinely set reaches the threshold in a frame or
            // two, while a noise hit has to recur in the same cell.
            if (++sightings[i] >= Tracking.CONFIRM) theirHeld[i] = true
        }
    }

    /** How many of this side's parts the other side is known to hold. */
    fun theirHeldCount(): Int = synchronized(lock) { (0 until count).count { holds(it) } }

    /** How many contiguous parts of theirs this side holds. */
    fun received(): Int = synchronized(lock) { got }

    /** How many parts they say they have, or -1 before any code of theirs
     *  has been read. */
    fun theirCount(): Int = synchronized(lock) { theirCount }

    /** **How many of this side's parts the other side holds** — their
     *  bitmap where it has been read, their header's contiguous count as
     *  the floor. The figure the screen and the diagnostics want; the raw
     *  contiguous count is [theirContiguous]. */
    fun theirReceived(): Int = theirHeldCount()

    /** What their *header* last reported, contiguous from the first: the
     *  floor under the bitmap, and all a side that cannot read the rows
     *  ever learns. */
    fun theirContiguous(): Int = synchronized(lock) { theirGot }

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

    /**
     * **Whether the other side is partitioned coarser than a colour frame
     * carries** — so, while this side is presenting colour, whether they
     * have fallen back to monochrome.
     *
     * No signal is needed for it: a colour part is at most [CHUNK_POLY]
     * bytes and a monochrome one is up to [CHUNK], and the two formats
     * share the five-byte header, so the length of a part that arrives is
     * the only thing that distinguishes them — and it distinguishes them
     * exactly. A short last part cannot make a monochrome partitioning
     * look like a colour one, because it is short and not long.
     *
     * **The fall back has to be symmetric** [author, 2026-10-06]: the two
     * presentations are lock-step, and a side left in colour after the
     * other has gone monochrome reads nothing of theirs at all, so it is
     * the one side that can never time out on its own.
     */
    fun theirsAreCoarse(): Boolean = synchronized(lock) {
        theirCount > 0 && theirs.any { it != null && it.size > CHUNK_POLY }
    }

    /** Whether every part of theirs is held. */
    fun complete(): Boolean = synchronized(lock) { theirCount > 0 && got == theirCount }

    /** The other side's object, once every part is held. */
    fun theirs(): ByteArray? = synchronized(lock) {
        if (theirCount <= 0 || got < theirCount) return null
        theirs.fold(ByteArray(0)) { acc, p -> acc + (p ?: ByteArray(0)) }
    }

    /** Both have everything: theirs is held here, and they are known to
     *  hold all of this side's — from their bitmap where it reads, from
     *  their header's contiguous count otherwise. */
    fun done(): Boolean = synchronized(lock) {
        theirCount > 0 && got == theirCount && (0 until count).all { holds(it) }
    }

    /** Where the two stand, for the screen. */
    fun status(): String {
        val shown = showing() + 1
        return synchronized(lock) {
            val theirsSoFar = if (theirCount < 0) "none of theirs yet" else "$got of $theirCount of theirs"
            "Showing part $shown of $count; received $theirsSoFar; they hold ${theirHeldCount()} of your $count."
        }
    }
}
