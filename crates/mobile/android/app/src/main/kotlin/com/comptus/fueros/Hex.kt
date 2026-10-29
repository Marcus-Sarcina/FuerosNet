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
