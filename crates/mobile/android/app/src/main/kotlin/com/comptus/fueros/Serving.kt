package com.comptus.fueros

/**
 * **The nodes an identity agreed to, one per address** — the tabs of the
 * browser model [author, 2026-11-11: *"one per {identity, subnet}"*], kept
 * in the identity's own scope under [Kernel]'s serving record. Each is
 * attached at bring-up, the first being the connection the shell shows.
 *
 * The record was one `{node, addr}` before the address model; that form
 * still reads as a single tab, so a device upgraded in place keeps the
 * node it agreed to.
 */
object Serving {
    /** A node and where it answers, as the invitation named them. */
    data class Tab(val node: String, val addr: String)

    /** The tabs in a record: the list, or the single pair the record once was. */
    fun read(bytes: ByteArray?): List<Tab> {
        val v = bytes?.let { runCatching { Json.parse(String(it)) }.getOrNull() } ?: return emptyList()
        val o = Json.obj(v)
        val tabs = o["tabs"]
        if (tabs != null) {
            return Json.list(tabs).mapNotNull { t ->
                val m = Json.obj(t)
                val n = Json.string(m["node"]) ?: return@mapNotNull null
                if (n.isEmpty()) null else Tab(n, Json.string(m["addr"]) ?: "")
            }
        }
        val n = Json.string(o["node"]) ?: return emptyList()
        return if (n.isEmpty()) emptyList() else listOf(Tab(n, Json.string(o["addr"]) ?: ""))
    }

    /** The record with [tab] in it: the entry for the same node replaced, appended otherwise. */
    fun with(bytes: ByteArray?, tab: Tab): ByteArray = encode(read(bytes).filter { it.node != tab.node } + tab)

    /** The record without the tab to [node]. */
    fun without(bytes: ByteArray?, node: String): ByteArray = encode(read(bytes).filter { it.node != node })

    /** The list form. */
    fun encode(tabs: List<Tab>): ByteArray =
        tabs.joinToString(",", "{\"tabs\":[", "]}") {
            "{\"node\":${Json.quote(it.node)},\"addr\":${Json.quote(it.addr)}}"
        }.toByteArray()
}
