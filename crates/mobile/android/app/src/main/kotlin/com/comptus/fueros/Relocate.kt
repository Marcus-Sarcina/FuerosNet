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

    /** The address without its port: what a person reads as the server's IP. */
    fun host(address: String): String = address.substringBeforeLast(':', address)

    /**
     * **The question, in the author's words** [author, 2026-11-16]: who
     * moved it, which of their subnets, from where to where, and whether
     * the new one is home.
     */
    fun question(patron: String, subnet: String, from: String, to: String): String =
        "$patron has relocated their server for $patron's $subnet from ${host(from)} to ${host(to)}.\n" +
            "Do you accept the server at ${host(to)} as your new home for $patron's $subnet?"

    /** The two answers, accepting first. */
    val ANSWERS: Pair<String, String> = "Accept" to "Do not connect"

    /** What the invitation screen says while the new address is dialled. */
    fun checking(at: String): String =
        "You already serve from this node, at another address: it says it has moved to ${host(at)}. " +
            "Checking that it answers there as itself…"

    /** What the invitation screen says once the dial has answered, or not. */
    fun found(held: Boolean, standing: Standing?): String = when {
        !held -> "Nothing changes: that is the address you already serve from."
        standing == Standing.PROVED ->
            "It answered there as itself. Open Fueros: the subnet it serves is marked, and asks you whether to accept the new server."
        standing == Standing.UNPROVED ->
            "Nothing answers as that node at the new address yet. It is tried again, and you are asked once it does."
        // **a failed check is not the person's** [author, 2026-11-16]: the
        // operator is told, and the record goes once they have been
        else -> "Nothing changes."
    }
}
