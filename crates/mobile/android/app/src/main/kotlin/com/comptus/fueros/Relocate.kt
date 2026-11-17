package com.comptus.fueros

import uniffi.rhtn_model.Relocation
import uniffi.rhtn_model.Standing

/**
 * **A patron's server said to have moved, as a person is asked about it**
 * (`light-client-requirements.md` §4; `rhtn_client::relocation`) [author,
 * 2026-11-16].
 *
 * The kernel holds the move and decides what it is; this is the words
 * and the one rule a screen needs: **a subnet whose node has a move the
 * person must answer is marked on its orb, and its page cannot be
 * reached until they do.** An unproved move is nothing to the person —
 * nothing at the new address has answered as the node yet — so it marks
 * nothing.
 *
 * Pure, so a JVM test holds the words as they are.
 */
object Relocate {

    /**
     * Whether `r` stands between the person and the subnet: a move that
     * proved itself, asked and not yet accepted. **A failed one is never
     * the person's** [author, 2026-11-16]: the operator is told, and
     * nothing changes here.
     */
    fun blocks(r: Relocation?): Boolean = when (r?.standing) {
        Standing.PROVED, Standing.DECLINED -> true
        Standing.UNPROVED, Standing.FAILED, null -> false
    }

    /**
     * **Whose server it is, as the question names them**: the name held
     * for them, else the one their invitation carried, else their key's
     * first eight characters. Never a role, which is ambiguous to someone
     * under several operators.
     */
    fun owner(held: String?, label: String?, keyHex: String): String =
        held?.takeIf { it.isNotBlank() } ?: label?.takeIf { it.isNotBlank() } ?: "${keyHex.take(8)}…"

    /** The address without its port: what a person reads as the server's IP. */
    fun host(address: String): String = address.substringBeforeLast(':', address)

    /**
     * **The question, in the author's words** [author, 2026-11-16]: who
     * moved it, which of their subnets, from where to where, and whether
     * the new one is home. **A first invitation is a move from nowhere**
     * [author, 2026-11-17: "The wording is correct"], asked in the same
     * shape without the old address.
     */
    // AUTHOR COPY [author, 2026-11-16; 2026-11-17]: both questions
    fun question(patron: String, subnet: String, from: String, to: String): String =
        if (from.isEmpty()) {
            "$patron has invited you to their server for $patron's $subnet at ${host(to)}.\n" +
                "Do you accept the server at ${host(to)} as your home for $patron's $subnet?"
        } else {
            "$patron has relocated their server for $patron's $subnet from ${host(from)} to ${host(to)}.\n" +
                "Do you accept the server at ${host(to)} as your new home for $patron's $subnet?"
        }

    /** The two answers, accepting first. */
    // AUTHOR COPY [author, 2026-11-16]
    val ANSWERS: Pair<String, String> = "Accept" to "Do not connect"
}
