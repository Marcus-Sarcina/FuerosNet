package com.comptus.fueros

/**
 * **What the People screen shows for one contact** — the pure part, so a
 * test can say it (`light-client-requirements.md` §4.3): the alias the
 * party published for the subnet its locator is in, and **the keyhash
 * where none has reached this device**.
 *
 * The screen itself is `PeopleActivity`; nothing here touches a view.
 */
object People {
    /** One contact's line. */
    fun line(p: Kernel.Person): String {
        val marks = buildList {
            if (p.located) add("placed")
            if (p.how == "a node") add("answers somewhere")
        }
        val tail = if (marks.isEmpty()) "" else " — ${marks.joinToString(", ")}"
        // **never a name that has not been received**: the alias where one
        // has, the keyhash otherwise
        return "${p.alias ?: p.who}  ${p.how}  ×${p.count}$tail"
    }

    /** The whole roll, or what to say when it is empty. */
    fun roll(held: List<Kernel.Person>): String =
        if (held.isEmpty()) "nobody yet" else held.joinToString("\n") { line(it) }
}
