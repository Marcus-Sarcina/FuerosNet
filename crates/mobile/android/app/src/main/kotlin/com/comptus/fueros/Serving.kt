package com.comptus.fueros

/**
 * **The shell's serving record, read for its import alone** [review,
 * 2026-11-13]. The nodes an identity agreed to are the kernel's to keep
 * (`Participant.agreed`); a phone from before that holds this file in the
 * identity's scope, as a list of tabs or the single pair it once was, and
 * [Kernel] takes it into the kernel once and forgets it.
 */
object Serving {
    /** A node and where it answers, as the record named them. */
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
}
