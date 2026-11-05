package com.comptus.fueros

/**
 * **Identities cross the boundary as bytes** (`rhtn_model.Hash32`): a
 * keyhash is thirty-two of them, never eight characters of hex. The
 * shell compares and shows them as hex; where it shortens one, it does
 * so on the screen that shows it and nowhere in the model.
 */
fun ByteArray.hex(): String = joinToString("") { "%02x".format(it) }

/** The bytes a hex string names; the caller has checked it is one. */
fun String.unhex(): ByteArray =
    ByteArray(length / 2) { ((this[2 * it].digitToInt(16) shl 4) + this[2 * it + 1].digitToInt(16)).toByte() }

/** Whether `this` is the thirty-two bytes of a keyhash or a txid, as hex:
 *  what [unhex] requires of its caller, asked once here rather than at
 *  each boundary that crosses one. */
fun String.isHash32(): Boolean = length == 64 && all { it in "0123456789abcdef" }
