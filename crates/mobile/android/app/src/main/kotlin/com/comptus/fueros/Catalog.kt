package com.comptus.fueros

/**
 * **What the Catalog screen shows** — the pure part, so a test can say it
 * (`wire-format.md` §6.4, §11.1).
 *
 * A node's portion may be **truncated** (it holds more of a type than one
 * page carries) or **stale** (it could not be reached on the last sweep),
 * and §6.4 makes both the client's to say rather than to smooth over.
 */
object Catalog {
    /** One entry's line. */
    fun line(e: Kernel.Offered): String {
        val marks = buildList {
            if (e.truncated) add("more than this held")
            if (e.stale) add("not reached last sweep")
        }
        val tail = if (marks.isEmpty()) "" else " — ${marks.joinToString(", ")}"
        return "${e.serviceType}  ${e.instance}  by ${e.owner}  at ${e.node}$tail"
    }

    /** The whole roll, or what to say when nothing is held. */
    fun roll(held: List<Kernel.Offered>): String =
        if (held.isEmpty()) "nothing held yet" else held.joinToString("\n") { line(it) }

    /** What a resource's answer comes to, in words (§11.1's six statuses). */
    fun answer(a: Kernel.Answered): String {
        val what = when (a.status.toInt()) {
            0 -> "delivered"
            1 -> "refused"
            2 -> "the resource is unavailable"
            3 -> "the request was malformed"
            4 -> "no subtree acknowledgement covers you"
            5 -> "no role of yours matches"
            else -> "status ${a.status}"
        }
        val body = a.body?.let { String(it) } ?: ""
        val first = body.lineSequence().firstOrNull()?.trim().orEmpty()
        return if (first.isEmpty()) what else "$what — $first"
    }
}
