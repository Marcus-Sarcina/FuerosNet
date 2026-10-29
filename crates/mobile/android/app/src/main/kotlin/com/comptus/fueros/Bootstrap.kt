package com.comptus.fueros

/**
 * **The bootstrap QR's payload** (D1a; `Robot/light-client-screens.md`):
 * this device's identifier, the kind of transaction, and — since
 * 2026-10-15 — which subnet an adoption is into and what the inviter calls
 * itself and the subnet there, **as the inviter's claim**
 * (`light-client-requirements.md` §7; `wire-format.md` §7.11). Nothing of
 * the ceremony's own anchor crosses here.
 *
 * **This is not a §14.3 object and the shell owns it.** A version byte
 * first, so a later framing can differ and be told apart; version 1 is
 * the 35-byte form without the subnet, still read.
 *
 * Version 2: `2, kind, backup, me(32), subnet(32, zeros where none),
 * aliasLen, alias, subnetAliasLen, subnetAlias`.
 *
 * **The alias is what the inviter calls itself, and it crosses here so
 * the name is visible from the scan** [author, 2026-10-29]: the name it
 * published for the subnet named, else its default name. A profile
 * reaches another device only per subnet, so two strangers have nothing
 * to fetch, and this is the least that carries it: self-declared, and
 * the screens say so where it matters.
 */
object Bootstrap {
    const val V1: Byte = 1
    const val V2: Byte = 2

    /** What a code carries, read. */
    data class Read(
        /** The inviter's keyhash, hex. */
        val me: String,
        /** 0 none, 1 they will be the patron, 2 they will be the client. */
        val kind: Int,
        val peering: Boolean,
        /** The subnet the adoption is into, hex, or null where the inviter
         *  named none (a lone root, or a regular meeting). */
        val subnet: String?,
        /** The inviter's own alias there, as claimed. */
        val alias: String?,
        /** The subnet's alias, as claimed. */
        val subnetAlias: String?,
    )

    private val ZERO = "0".repeat(64)

    fun encode(
        me: ByteArray,
        kind: Int,
        peering: Boolean,
        scope: Meet.Scope?,
        /** What this device calls itself: the subnet's own name for it
         *  where one is published, else the default. */
        ownAlias: String? = scope?.ownAlias,
    ): ByteArray {
        val subnet = scope?.id?.takeIf { it.length == 64 }?.let { Provision.unhex(it) } ?: ByteArray(32)
        val alias = (ownAlias ?: "").toByteArray().take(64).toByteArray()
        val subnetAlias = (scope?.alias ?: "").toByteArray().take(64).toByteArray()
        return byteArrayOf(V2, kind.toByte(), (if (peering) 1 else 0).toByte()) +
            me + subnet +
            byteArrayOf(alias.size.toByte()) + alias +
            byteArrayOf(subnetAlias.size.toByte()) + subnetAlias
    }

    /** Null where the bytes are not a bootstrap this build knows. */
    fun decode(bytes: ByteArray): Read? {
        if (bytes.size < 35) return null
        val kind = bytes[1].toInt()
        if (kind !in 0..2) return null
        val backup = bytes[2].toInt() == 1
        val me = Provision.hex(bytes.copyOfRange(3, 35))
        return when (bytes[0]) {
            V1 -> if (bytes.size == 35) Read(me, kind, backup, null, null, null) else null
            V2 -> {
                if (bytes.size < 69) return null
                val subnet = Provision.hex(bytes.copyOfRange(35, 67)).takeIf { it != ZERO }
                var at = 67
                val alias = string(bytes, at) ?: return null
                at += 1 + alias.toByteArray().size
                val subnetAlias = string(bytes, at) ?: return null
                at += 1 + subnetAlias.toByteArray().size
                if (at != bytes.size) return null
                Read(me, kind, backup, subnet, alias.ifEmpty { null }, subnetAlias.ifEmpty { null })
            }
            else -> null
        }
    }

    private fun string(bytes: ByteArray, at: Int): String? {
        if (at >= bytes.size) return null
        val n = bytes[at].toInt() and 0xff
        if (n > 64 || at + 1 + n > bytes.size) return null
        return String(bytes.copyOfRange(at + 1, at + 1 + n))
    }
}
