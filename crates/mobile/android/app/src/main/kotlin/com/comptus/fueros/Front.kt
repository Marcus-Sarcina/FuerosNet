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
    )

    private class Convo(val by: String, val peer: String, var name: String) {
        var direct: Boolean = false
        val messages = mutableListOf<Message>()
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

    /** Written after every change to one identity's threads, where the shell wired one. */
    var persist: ((String) -> Unit)? = null

    private fun changed(by: String? = null) {
        // under the lock, so what a render reads is the state this change
        // produced and not the next one
        ui?.render()
        by?.let { persist?.invoke(it) }
    }

    /**
     * **The threads kept for an identity, back** [author, 2026-11-12]: what
     * a file held, under what is already here, the sequence past every
     * id read.
     */
    fun restore(threads: List<Thread>) {
        synchronized(lock) {
            for (t in threads) {
                val c = convos.getOrPut(key(t.by, t.peerKey)) { Convo(t.by, t.peerKey, t.peerName) }
                if (c.messages.isEmpty()) {
                    c.name = t.peerName
                    c.direct = t.direct
                    c.messages.addAll(t.messages)
                }
                seq = maxOf(seq, (t.messages.maxOfOrNull { it.id } ?: -1L) + 1)
            }
            changed()
        }
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

    private fun Convo.snapshot() = Thread(by, peer, name, direct, messages.toList())

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
            convos.getOrPut(key(by, peerKey)) { Convo(by, peerKey, name) }.name = name
            changed(by)
        }
    }

    /** Whether a direct path to the peer is held now: **status, never a
     *  choice** — the path order is fixed (design §12.6.3). */
    fun directPath(by: String, peerKey: String, held: Boolean) {
        synchronized(lock) {
            convos[key(by, peerKey)]?.let {
                it.direct = held
                changed(by)
            }
        }
    }

    /** An arrival, attributed to its sender: the name it came with is the
     *  attribution and the thread's name from here. */
    fun incoming(by: String, peerKey: String, name: String, text: String) {
        synchronized(lock) {
            val c = convos.getOrPut(key(by, peerKey)) { Convo(by, peerKey, name) }
            c.name = name
            c.messages.add(Message(seq++, false, name, text, Delivery.SENT, null))
            changed(by)
        }
    }

    /** An outgoing message, in flight. Returns its id for [settle]. */
    fun outgoing(by: String, peerKey: String, text: String): Long {
        synchronized(lock) {
            val c = convos.getOrPut(key(by, peerKey)) { Convo(by, peerKey, peerKey) }
            val id = seq++
            c.messages.add(Message(id, true, "me", text, Delivery.SENDING, null))
            changed(by)
            return id
        }
    }

    /** The kernel's send returned: sent for carriage, or unsent with why. */
    fun settle(by: String, peerKey: String, id: Long, delivery: Delivery, reason: String?) {
        synchronized(lock) {
            val c = convos[key(by, peerKey)] ?: return
            val i = c.messages.indexOfFirst { it.id == id }
            if (i >= 0) {
                c.messages[i] = c.messages[i].copy(delivery = delivery, reason = reason)
                changed(by)
            }
        }
    }

    private companion object {
        /** How many notice lines are kept; the oldest goes first. */
        const val NOTICE_LINES = 200
    }
}
