package com.comptus.fueros

import uniffi.rhtn_model.Resource

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
    /** One entry, the owner and the serving node shortened here and nowhere else. */
    fun line(e: Resource): String {
        val marks = buildList {
            if (e.truncated) add("more than this held")
            if (e.stale) add("not reached last sweep")
        }
        val tail = if (marks.isEmpty()) "" else " — ${marks.joinToString(", ")}"
        return "${e.serviceType}  ${e.instance}  by ${e.owner.hex().take(8)}  at ${e.node.hex().take(8)}$tail"
    }

    /** The whole roll, or what to say when nothing is held. */
    fun roll(held: List<Resource>): String =
        if (held.isEmpty()) "nothing held yet" else held.joinToString("\n") { line(it) }

    /**
     * **What a resource's answer comes to, for somebody debugging a
     * node**: §11.1's delivery status and, where one was delivered, the
     * application's own status and what it declared.
     *
     * **The page itself is not here.** [ResourcePage] decides what may be
     * shown of a body and [ResourceActivity] shows it; this line is the
     * diagnostic beside that, and it says the same words about a status
     * so the two never disagree about one.
     */
    fun answer(a: Kernel.Answered): String {
        val what = ResourcePage.delivery(a.status)
        a.unreadable?.let { return "$what, and unreadable: $it" }
        val p = a.page ?: return what
        val declared = p.media ?: "nothing declared"
        return "$what — HTTP ${p.httpStatus}, $declared, ${p.body.size} bytes"
    }
}
