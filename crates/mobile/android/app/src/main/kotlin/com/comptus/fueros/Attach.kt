package com.comptus.fueros

/**
 * **What the attach screen reads and says** — the pure part, so a test can
 * hold it (`light-client-requirements.md` §4; the screens sheet's B status
 * line, and the capability matrix's *attach to the serving node*).
 *
 * Until now the only way this device attached to a node was the bench's
 * provision blob, read once at a cold start: an installed app could meet
 * whoever was in front of it and could never reach a serving node, so no
 * verifier, no witness past the two present, no catalog and no mailbox
 * [2026-10-27]. The screen is `AttachActivity`; nothing here touches a
 * view.
 *
 * **A node is named by its keyhash and reached at an address**, and the
 * two are different claims: the address says where to knock and the
 * keyhash is who must answer, which is what makes a wrong address a failed
 * attach rather than an attach to somebody else.
 */
object Attach {

    /** How many hex characters a keyhash is. */
    const val KEY_CHARS = 64

    /**
     * A node's keyhash, normalised: spaces and an `0x` prefix taken off,
     * lower-cased. Null where it is not 32 bytes of hex.
     */
    fun node(text: String): String? {
        val t = text.trim().removePrefix("0x").filterNot { it == ' ' || it == ':' }.lowercase()
        if (t.length != KEY_CHARS) return null
        return if (t.all { it in "0123456789abcdef" }) t else null
    }

    /**
     * An address the kernel will take: an IP and a port, `1.2.3.4:7447` or
     * `[::1]:7447`. **A name is not one** — the kernel parses a socket
     * address and resolves nothing — so this says no to one rather than
     * letting the refusal arrive from three layers down.
     */
    fun address(text: String): String? {
        val t = text.trim()
        val cut = t.lastIndexOf(':')
        if (cut <= 0 || cut == t.length - 1) return null
        val host = t.substring(0, cut)
        val port = t.substring(cut + 1).toIntOrNull() ?: return null
        if (port !in 1..65535) return null
        if (host.startsWith("[")) {
            if (!host.endsWith("]") || host.length < 4) return null
            val inner = host.substring(1, host.length - 1)
            return if (inner.all { it in "0123456789abcdefABCDEF:." }) t else null
        }
        val quads = host.split(".")
        if (quads.size != 4) return null
        return if (quads.all { q -> q.isNotEmpty() && q.toIntOrNull() in 0..255 }) t else null
    }

    /**
     * Why this pair is not worth trying, or null where it is. **One reason
     * at a time, naming the field**: a screen that says "invalid" about
     * two fields at once tells a person to check both.
     */
    fun refusal(node: String, address: String): String? {
        if (node.isBlank() && address.isBlank()) return "The node's key and its address, both."
        if (node(node) == null) {
            return "A node's key is $KEY_CHARS hex characters; that is ${node.trim().length}."
        }
        if (address.isBlank()) return "An address too: where this node answers."
        if (address(address) == null) {
            return "An address is an IP and a port, like 192.168.0.8:7447. " +
                "A name is not one here: the kernel resolves nothing."
        }
        return null
    }

    /** One node this device already knows how to reach. */
    fun line(n: Kernel.Node): String =
        "${n.node.take(16)}…  ${if (n.addresses.isEmpty()) "no address held" else n.addresses.joinToString(" ")}"

    /**
     * The nodes whose endpoint records this device holds, which is what a
     * person can attach to without typing anything. Empty before any
     * record has reached it, which is the ordinary state of a fresh device:
     * a node's code is taken on the People screen, or its address typed
     * here.
     */
    fun roll(known: List<Kernel.Node>): String =
        if (known.isEmpty()) {
            "No node's record has reached this device yet. Take one on the People " +
                "screen, or type where a node answers below."
        } else {
            known.joinToString("\n") { line(it) }
        }
}
