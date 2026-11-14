package com.comptus.fueros

/**
 * The kernel's front: the UI state a screen renders, kept apart from the
 * kernel so it depends on nothing a JVM test cannot hold — no Android, no
 * binding, no participant.
 *
 * A screen binds, is rendered once against the state so far, and rendered
 * again on every change; a screen that has gone is a null sink; a stale
 * unbind from a replaced screen does not silence its replacement. State and
 * the bound screen move together under one lock, so a screen never renders
 * a half-applied change and a bind never races a mutation.
 */
class Front {

    /**
     * What a bound screen is told: re-read the accessors and redraw. Called
     * on the kernel's thread, under the lock, so a screen posts to its own
     * thread and never reads back synchronously into the kernel.
     */
    interface Ui {
        /** Re-read the accessors and redraw. */
        fun render()
    }

    /**
     * How far an outgoing message got. **Never delivered or read**: the
     * protocol carries no receipt, so the furthest honest state is that the
     * kernel accepted the message for carriage — a submission, not a
     * delivery (`light-client-requirements.md` §9; the screens sheet's C2).
     */
    enum class Delivery {
        /** Handed to the kernel, which has not yet said. */
        SENDING,
        /** The kernel accepted it for carriage. */
        SENT,
        /** The kernel refused it, and [Message.reason] says why. */
        UNSENT,
    }

    /** One message on a thread, as a screen shows it. */
    data class Message(
        /** This shell's own ordinal, which gives the thread its order. */
        val id: Long,
        /** Whether this side sent it. */
        val mine: Boolean,
        /** Who sent it, as a screen names them. */
        val who: String,
        /** The text. */
        val text: String,
        /** How far it got, for one this side sent. */
        val delivery: Delivery,
        /** Why it was refused, where it was. */
        val reason: String?,
    )

    /** One conversation, as a screen shows it. */
    data class Thread(
        /**
         * **Whose conversation this is** — step 2b of the browser model
         * [author, 2026-11-11]: each {identity, subnet} is sandboxed from
         * every other on the device, so a thread is keyed by the identity
         * that holds it and the peer, never by the peer alone — two
         * identities' conversations with one party are two threads, and a
         * reply goes out as the one that holds it ([Kernel.send]).
         */
        val by: String,
        /** The counterparty's keyhash in hex, which with [by] identifies the thread. */
        val peerKey: String,
        /** What to call them on screen. */
        val peerName: String,
        /** Whether the last message went over the direct path. */
        val direct: Boolean,
        /** The messages, oldest first. */
        val messages: List<Message>,
        /** Earlier messages exist on disk that this snapshot does not hold ([earlier] reads them). */
        val more: Boolean = false,
    )

    private class Convo(val by: String, val peer: String, var name: String) {
        var direct: Boolean = false
        /** The messages read or written in this process: the newest page and what came after it. */
        val messages = mutableListOf<Message>()
        /** Whether a page has been read; a thread is read when its conversation is opened, not at start. */
        var loaded = false
        /** Where the next earlier page ends; null once the thread's start has been read. */
        var before: Long? = null
        /** Delivery changes met for messages that lie in a page not read yet. */
        val pending = HashMap<Long, Threads.Record.Settled>()
    }

    private val lock = Any()
    private var ui: Ui? = null
    private var statusLine = "starting the kernel…"
    private var provisioned = false
    private val notices = mutableListOf<String>()
    // insertion order is thread order: the peer provisioned first sits first
    private val convos = linkedMapOf<String, Convo>()
    private var seq = 0L

    // ---- binding, the S05 lifecycle ------------------------------------

    /** Bind a screen and render it once against the state so far. */
    fun bind(u: Ui) {
        synchronized(lock) {
            ui = u
            u.render()
        }
    }

    /** Unbind, if this is still the screen bound: a late unbind from a
     *  screen already replaced must not silence its replacement. */
    fun unbind(u: Ui) {
        synchronized(lock) {
            if (ui === u) {
                ui = null
            }
        }
    }

    /**
     * **The store, as the shell wires it** (`light-client-requirements.md`
     * §9, APP-015) [author, 2026-11-14]: a record appended as it
     * accumulates, the index written when a thread is added or renamed,
     * a page read when a conversation is opened. Absent, conversations
     * live in this process alone.
     */
    var appender: ((by: String, peer: String, record: Threads.Record) -> Long?)? = null
    var indexer: ((by: String) -> Unit)? = null
    var loader: ((by: String, peer: String, before: Long?, pending: Map<Long, Threads.Record.Settled>) -> Threads.Page?)? = null

    private fun changed() {
        // under the lock, so what a render reads is the state this change
        // produced and not the next one
        ui?.render()
    }

    /** An identity's threads as its index names them: listed here, read when opened. */
    fun restore(by: String, entries: List<Threads.Entry>) {
        synchronized(lock) {
            for (e in entries) {
                val c = convos.getOrPut(key(by, e.peer)) { Convo(by, e.peer, e.name) }
                c.name = e.name
                c.direct = e.direct
            }
            changed()
        }
    }

    /** What the index of `by` should say now. */
    fun entries(by: String): List<Threads.Entry> =
        synchronized(lock) { convos.values.filter { it.by == by }.map { Threads.Entry(it.peer, it.name, it.direct) } }

    /** **Open a conversation**: its newest page read, once, from the store. */
    fun open(by: String, peerKey: String) {
        val load = loader ?: return
        val (peer, pending) = synchronized(lock) {
            val c = convos[key(by, peerKey)] ?: return
            if (c.loaded) return
            Pair(c.peer, HashMap(c.pending))
        }
        val page = load(by, peer, null, pending) ?: return
        synchronized(lock) {
            val c = convos[key(by, peerKey)] ?: return
            if (c.loaded) return
            c.loaded = true
            c.messages.addAll(0, page.messages)
            c.before = page.before
            c.pending.clear()
            c.pending.putAll(page.pending)
            changed()
        }
    }

    /** **Read the page before the earliest one held**, when the person asks for it. */
    fun earlier(by: String, peerKey: String) {
        val load = loader ?: return
        val (peer, before, pending) = synchronized(lock) {
            val c = convos[key(by, peerKey)] ?: return
            val b = c.before ?: return
            Triple(c.peer, b, HashMap(c.pending))
        }
        val page = load(by, peer, before, pending) ?: return
        synchronized(lock) {
            val c = convos[key(by, peerKey)] ?: return
            if (c.before != before) return
            c.messages.addAll(0, page.messages)
            c.before = page.before
            c.pending.clear()
            c.pending.putAll(page.pending)
            changed()
        }
    }

    /** A record written for a conversation; the id the store gave it, or the process's own where there is no store. */
    private fun written(c: Convo, record: Threads.Record): Long {
        val id = appender?.invoke(c.by, c.peer, record) ?: seq++
        if (!c.loaded) {
            // what was written in this process is the newest, and what the
            // file held before it is the page before
            c.loaded = true
            c.before = if (appender != null && id > 0) id else null
        }
        return id
    }

    // ---- accessors: snapshots under the lock ---------------------------

    fun status(): String = synchronized(lock) { statusLine }

    /** Whether the kernel has a session and material published. */
    fun provisioned(): Boolean = synchronized(lock) { provisioned }

    /** The notice lines, oldest first, at most [NOTICE_LINES] of them. */
    fun notices(): List<String> = synchronized(lock) { notices.toList() }

    /** Every conversation this shell holds. */
    fun threads(): List<Thread> =
        synchronized(lock) { convos.values.map { it.snapshot() } }

    /** The conversation with `peerKey`, where there is one. */
    fun thread(by: String, peerKey: String): Thread? =
        synchronized(lock) { convos[key(by, peerKey)]?.snapshot() }

    /** The one key a conversation has: the identity that holds it and the peer. */
    private fun key(by: String, peerKey: String) = "$by/$peerKey"

    private fun Convo.snapshot() = Thread(by, peer, name, direct, messages.toList(), more = before != null)

    // ---- mutators: the kernel's side -----------------------------------

    fun setStatus(line: String) {
        synchronized(lock) {
            statusLine = line
            changed()
        }
    }

    /** Say whether the kernel is provisioned, and redraw. */
    fun setProvisioned(yes: Boolean) {
        synchronized(lock) {
            provisioned = yes
            changed()
        }
    }

    /** A system line — provisioning guidance, a refusal not tied to one
     *  message. Bounded: an unbounded log is a leak the process never
     *  recovers from. */
    fun note(line: String) {
        synchronized(lock) {
            notices.add(line)
            while (notices.size > NOTICE_LINES) {
                notices.removeAt(0)
            }
            changed()
        }
    }

    /** The peer this device was provisioned to talk to is known: its thread
     *  exists from here, empty until something is said. */
    fun peerKnown(by: String, peerKey: String, name: String) {
        synchronized(lock) {
            val k = key(by, peerKey)
            val had = convos[k]
            val c = convos.getOrPut(k) { Convo(by, peerKey, name) }
            val renamed = had == null || c.name != name
            c.name = name
            if (renamed) indexer?.invoke(by)
            changed()
        }
    }

    /** Whether a direct path to the peer is held now: **status, never a
     *  choice** — the path order is fixed (design §12.6.3). */
    fun directPath(by: String, peerKey: String, held: Boolean) {
        synchronized(lock) {
            convos[key(by, peerKey)]?.let {
                if (it.direct != held) {
                    it.direct = held
                    indexer?.invoke(by)
                }
                changed()
            }
        }
    }

    /** An arrival, attributed to its sender: the name it came with is the
     *  attribution and the thread's name from here. */
    fun incoming(by: String, peerKey: String, name: String, text: String) {
        synchronized(lock) {
            val k = key(by, peerKey)
            val had = convos[k]
            val c = convos.getOrPut(k) { Convo(by, peerKey, name) }
            val listed = had == null || c.name != name
            c.name = name
            val id = written(c, Threads.Record.Said(false, name, text, Delivery.SENT, null))
            c.messages.add(Message(id, false, name, text, Delivery.SENT, null))
            if (listed) indexer?.invoke(by)
            changed()
        }
    }

    /** An outgoing message, in flight. Returns its id for [settle]. */
    fun outgoing(by: String, peerKey: String, text: String): Long {
        synchronized(lock) {
            val k = key(by, peerKey)
            val had = convos[k]
            val c = convos.getOrPut(k) { Convo(by, peerKey, peerKey) }
            val id = written(c, Threads.Record.Said(true, "me", text, Delivery.SENDING, null))
            c.messages.add(Message(id, true, "me", text, Delivery.SENDING, null))
            if (had == null) indexer?.invoke(by)
            changed()
            return id
        }
    }

    /** The kernel's send returned: sent for carriage, or unsent with why. */
    fun settle(by: String, peerKey: String, id: Long, delivery: Delivery, reason: String?) {
        synchronized(lock) {
            val c = convos[key(by, peerKey)] ?: return
            appender?.invoke(by, c.peer, Threads.Record.Settled(id, delivery, reason))
            val i = c.messages.indexOfFirst { it.id == id }
            if (i >= 0) {
                c.messages[i] = c.messages[i].copy(delivery = delivery, reason = reason)
                changed()
            }
        }
    }

    private companion object {
        /** How many notice lines are kept; the oldest goes first. */
        const val NOTICE_LINES = 200
    }
}
