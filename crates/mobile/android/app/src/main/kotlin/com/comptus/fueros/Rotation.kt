package com.comptus.fueros

/**
 * **The pure part of changing a key** (PRD-03), so a test can hold it, as
 * [Chrome], [Engagement] and [Backup] are.
 *
 * There is little to decide and one thing easy to get wrong: what to call
 * a party in a warning. PRD-14 forbids showing a name this device was not
 * given, and a warning naming nobody is a warning about nothing, so this
 * is where the two meet.
 */
object Rotation {

    /** How many characters of a keyhash stand in for a name. */
    const val SHORT = 8

    /**
     * **What to call a party in a sentence**: the alias they published,
     * where this device holds one, and their keyhash otherwise.
     *
     * **Never a name this device invented** (PRD-14,
     * `light-client-requirements.md` §4.3): an alias is a party's own
     * published claim, and where none has arrived the keyhash is what
     * this device honestly has. A blank, or an invented "your patron",
     * would make the author's sentences name nobody.
     */
    fun name(keyhash: String, alias: String?): String {
        val said = alias?.trim().orEmpty()
        return if (said.isEmpty()) "${keyhash.take(SHORT)}…" else said
    }
}
