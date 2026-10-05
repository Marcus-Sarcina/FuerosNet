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
        /** Parts of 256 bytes: the 2,022-byte contribution is eight codes
         *  of 61 modules, against one of 173. */
        const val CHUNK = 256
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
