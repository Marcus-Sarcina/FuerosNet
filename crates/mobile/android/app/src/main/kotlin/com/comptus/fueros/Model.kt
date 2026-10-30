package com.comptus.fueros

import uniffi.rhtn_model.Connection
import uniffi.rhtn_model.Contact
import uniffi.rhtn_model.Instance
import uniffi.rhtn_model.Line
import uniffi.rhtn_model.Meeting
import uniffi.rhtn_model.Noun
import uniffi.rhtn_model.Resource
import uniffi.rhtn_model.Subnet

/**
 * **The shell's copy of the nouns, kept in step with the kernel**
 * (`Robot/implementation-plan.md` section 8, stage two; the nouns are
 * `rhtn_model`'s, generated into `uniffi.rhtn_model`).
 *
 * One snapshot per noun, read from the kernel's ports off the drawing
 * thread and replaced whole. **What refreshes it is the kernel saying
 * so**: the kernel marks a noun at each place its store changes it, the
 * FFI hands those up as `Event.Changed` at the next quiet turn of the
 * event loop, and [Kernel] calls [refresh] with exactly those. A screen
 * reads the snapshot and watches for the nouns it draws, instead of
 * calling the kernel at every `onStart` and redrawing on every status
 * line, which is what the rail did until now.
 *
 * **Nothing here decides what a noun is called on a screen.** The
 * snapshots are the model's records; the screens' pure parts word them.
 */
object Model {

    /** Told which nouns changed, on the thread the refresh ran on. */
    fun interface Watcher {
        fun changed(nouns: Set<Noun>)
    }

    @Volatile var contacts: List<Contact> = emptyList(); private set
    @Volatile var subnets: List<Subnet> = emptyList(); private set
    @Volatile var lines: List<Line> = emptyList(); private set
    @Volatile var instances: List<Instance> = emptyList(); private set
    @Volatile var resources: List<Resource> = emptyList(); private set

    /** The meetings this device took part in, newest first: outcomes, not the ceremony's state. */
    @Volatile var meetings: List<Meeting> = emptyList(); private set
    @Volatile var connection: Connection = Connection.Detached; private set

    /** The aliases held, by keyhash in hex, which is how a screen names a party. */
    @Volatile var aliases: Map<String, String> = emptyMap(); private set

    private val lock = Any()
    private val watchers = mutableListOf<Watcher>()

    fun watch(w: Watcher) = synchronized(lock) { watchers += w }

    fun unwatch(w: Watcher) = synchronized(lock) { watchers -= w }

    /**
     * Re-read `nouns` from the kernel and tell the watchers. **Off the
     * caller's thread**: each read is a blocking call into a client that
     * may be parked in `nextEvent` for its poll interval.
     */
    fun refresh(nouns: Set<Noun>) {
        if (nouns.isEmpty()) return
        Thread({
            if (Noun.CONTACTS in nouns || Noun.PROFILES in nouns) {
                val held = Kernel.contacts()
                contacts = held
                aliases = held.mapNotNull { c -> c.alias?.let { c.keyhash.hex() to it } }.toMap()
            }
            if (Noun.SUBNETS in nouns || Noun.PROFILES in nouns) subnets = Kernel.subnets()
            if (Noun.LINES in nouns) lines = Kernel.lines()
            if (Noun.INSTANCES in nouns) instances = Kernel.instances()
            if (Noun.RESOURCES in nouns) resources = Kernel.resources()
            if (Noun.MEETINGS in nouns) meetings = Kernel.meetings()
            Diag.event("model.refreshed", "nouns" to nouns.joinToString(",") { it.name.lowercase() })
            tell(nouns)
        }, "fueros-model").start()
    }

    /** Every noun, which is what a kernel just up calls for. */
    fun refreshAll() = refresh(Noun.entries.toSet())

    /** The connection as the kernel last reported it. */
    fun connection(c: Connection) {
        connection = c
        tell(setOf(Noun.CONNECTION))
    }

    private fun tell(nouns: Set<Noun>) {
        val now = synchronized(lock) { watchers.toList() }
        for (w in now) w.changed(nouns)
    }
}
