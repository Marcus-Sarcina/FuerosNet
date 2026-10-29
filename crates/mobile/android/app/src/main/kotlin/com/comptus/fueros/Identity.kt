package com.comptus.fueros

import java.security.MessageDigest
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

/**
 * **An identity as words, and the words back as an identity** — the pure
 * part, so a test can hold it (`light-client-requirements.md` §9: *a seed
 * phrase, if you offer one, is a feature of its own and nothing depends
 * on it*; the screens sheet's A1–A2).
 *
 * **The phrase is BIP-39's shape**: 32 bytes of entropy, an 8-bit
 * checksum off its SHA-256, 24 words of eleven bits each from a list of
 * 2048 — the form every wallet a person has used reads and writes, so
 * nothing here is a format of this app's own. **The wordlist is a file**
 * (`assets/bip39/english.txt`), not code: the list is the standard's.
 *
 * **What the phrase encodes is a master, not the kernel's seeds.** The
 * kernel takes 64 bytes, two seeds of 32 (`Participant.start`); a phrase
 * of 24 words carries 32. So the master is what is minted and shown, and
 * the two seeds are derived from it here, by HKDF-SHA256 under a fixed
 * label — the same derivation on every device that restores the phrase,
 * and the kernel given 64 bytes as it always was. **Above the kernel**,
 * as §9 requires: delete this file and the kernel still starts on 64
 * bytes handed to it another way.
 *
 * **Shown once, at creation, and never redisplayed** [§9]: a phrase a
 * screen will show again is a phrase anyone holding the unlocked device
 * can take. Nothing here keeps the words; the shell keeps the derived
 * seeds under its own custody and the master is dropped.
 */
object Identity {

    /** Words in a phrase, and bytes of entropy behind them. */
    const val WORDS = 24
    const val ENTROPY_BYTES = 32

    /** How many words the list holds; eleven bits index one. */
    const val LIST = 2048

    /** The kernel's seeds: two of 32 bytes. */
    const val SEED_BYTES = 64

    /** What an alias may be: 1 to 64 bytes (`wire-format.md` §7.11). */
    const val ALIAS_BYTES = 64

    /** The label the two seeds derive under; changing it is a new identity. */
    private const val SEEDS_INFO = "fueros identity seeds v1"

    /**
     * The 24 words for 32 bytes of entropy. The list is given rather than
     * held, so a test can hand in one of its own and the real one is read
     * from the asset where the screen is.
     */
    fun phrase(entropy: ByteArray, words: List<String>): List<String> {
        require(entropy.size == ENTROPY_BYTES) { "entropy is $ENTROPY_BYTES bytes" }
        require(words.size == LIST) { "a wordlist is $LIST words" }
        val check = MessageDigest.getInstance("SHA-256").digest(entropy)[0]
        // 256 bits of entropy then 8 of checksum: 264 bits, 24 × 11
        val bits = StringBuilder(264)
        for (b in entropy) bits.append(bitsOf(b))
        bits.append(bitsOf(check))
        return (0 until WORDS).map { i -> words[bits.substring(i * 11, i * 11 + 11).toInt(2)] }
    }

    /** What reading a phrase back came to. */
    sealed class Read {
        /** The 32 bytes the words encode. */
        data class Entropy(val bytes: ByteArray) : Read()

        /** Why they could not be read: said for the person to act on. */
        data class Refused(val why: String) : Read()
    }

    /**
     * The entropy behind 24 words, or why not. **The reason names the
     * fault** — a word not on the list, by position; a count other than
     * 24; a checksum that does not hold — because a phrase typed wrong is
     * fixed by knowing where.
     */
    fun entropy(typed: List<String>, words: List<String>): Read {
        require(words.size == LIST) { "a wordlist is $LIST words" }
        val index = words.withIndex().associate { (i, w) -> w to i }
        val given = typed.map { it.trim().lowercase() }.filter { it.isNotEmpty() }
        if (given.size != WORDS) {
            return Read.Refused("a phrase is $WORDS words; that is ${given.size}")
        }
        val unknown = given.withIndex().filter { (_, w) -> w !in index }
        if (unknown.isNotEmpty()) {
            val said = unknown.joinToString(", ") { (i, w) -> "${i + 1}: $w" }
            return Read.Refused("not on the list — $said")
        }
        val bits = StringBuilder(264)
        for (w in given) bits.append(index.getValue(w).toString(2).padStart(11, '0'))
        val entropy = ByteArray(ENTROPY_BYTES) { i -> bits.substring(i * 8, i * 8 + 8).toInt(2).toByte() }
        val check = MessageDigest.getInstance("SHA-256").digest(entropy)[0]
        if (bitsOf(check) != bits.substring(256, 264)) {
            return Read.Refused("the words do not check: one of them is wrong, or two are swapped")
        }
        return Read.Entropy(entropy)
    }

    /**
     * The kernel's two seeds from the master: HKDF-SHA256, no salt, the
     * fixed label, 64 bytes out. Deterministic, so the same phrase on
     * another device is the same identity.
     */
    fun seeds(master: ByteArray): ByteArray {
        require(master.size == ENTROPY_BYTES) { "a master is $ENTROPY_BYTES bytes" }
        val mac = Mac.getInstance("HmacSHA256")
        // extract with a zero salt of the hash length
        mac.init(SecretKeySpec(ByteArray(32), "HmacSHA256"))
        val prk = mac.doFinal(master)
        // expand: T(1) || T(2), 64 bytes exactly
        mac.init(SecretKeySpec(prk, "HmacSHA256"))
        val info = SEEDS_INFO.toByteArray()
        val t1 = mac.doFinal(info + byteArrayOf(1))
        mac.init(SecretKeySpec(prk, "HmacSHA256"))
        val t2 = mac.doFinal(t1 + info + byteArrayOf(2))
        return t1 + t2
    }

    /**
     * Why a name will not do, or null where it will. A default alias is
     * required at creation [author, 2026-10-29] and bounded as every
     * alias is.
     */
    fun aliasRefusal(name: String): String? {
        val t = name.trim()
        if (t.isEmpty()) return "A name, so people have something to call you."
        val bytes = t.toByteArray().size
        if (bytes > ALIAS_BYTES) return "A name is at most $ALIAS_BYTES bytes; that is $bytes."
        return null
    }

    /** The phrase laid out for a screen: numbered, three to a line. */
    fun laidOut(words: List<String>): String =
        words.withIndex().joinToString("") { (i, w) ->
            "%2d %-10s".format(i + 1, w) + if (i % 3 == 2) "\n" else " "
        }.trimEnd()

    /** The standard list, read from the asset as one word per line. */
    fun wordlist(lines: Sequence<String>): List<String> =
        lines.map { it.trim() }.filter { it.isNotEmpty() }.toList()

    private fun bitsOf(b: Byte): String = (b.toInt() and 0xff).toString(2).padStart(8, '0')
}
