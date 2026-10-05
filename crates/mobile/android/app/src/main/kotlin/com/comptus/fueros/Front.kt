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
        /** The counterparty's keyhash in hex, which identifies the thread. */
        val peerKey: String,
        /** What to call them on screen. */
        val peerName: String,
        /** Whether the last message went over the direct path. */
        val direct: Boolean,
        /** The messages, oldest first. */
        val messages: List<Message>,
    )

    private class Convo(var name: String) {
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

    private fun changed() {
        // under the lock, so what a render reads is the state this change
        // produced and not the next one
        ui?.render()
    }

    // ---- accessors: snapshots under the lock ---------------------------

    fun status(): String = synchronized(lock) { statusLine }

    /** Whether the kernel has a session and material published. */
    fun provisioned(): Boolean = synchronized(lock) { provisioned }

    /** The notice lines, oldest first, at most [NOTICE_LINES] of them. */
    fun notices(): List<String> = synchronized(lock) { notices.toList() }

    /** Every conversation this shell holds. */
    fun threads(): List<Thread> =
        synchronized(lock) { convos.entries.map { it.value.snapshot(it.key) } }

    /** The conversation with `peerKey`, where there is one. */
    fun thread(peerKey: String): Thread? =
        synchronized(lock) { convos[peerKey]?.snapshot(peerKey) }

    private fun Convo.snapshot(key: String) = Thread(key, name, direct, messages.toList())

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
    fun peerKnown(peerKey: String, name: String) {
        synchronized(lock) {
            convos.getOrPut(peerKey) { Convo(name) }.name = name
            changed()
        }
    }

    /** Whether a direct path to the peer is held now: **status, never a
     *  choice** — the path order is fixed (design §12.6.3). */
    fun directPath(peerKey: String, held: Boolean) {
        synchronized(lock) {
            convos[peerKey]?.let {
                it.direct = held
                changed()
            }
        }
    }

    /** An arrival, attributed to its sender: the name it came with is the
     *  attribution and the thread's name from here. */
    fun incoming(peerKey: String, name: String, text: String) {
        synchronized(lock) {
            val c = convos.getOrPut(peerKey) { Convo(name) }
            c.name = name
            c.messages.add(Message(seq++, false, name, text, Delivery.SENT, null))
            changed()
        }
    }

    /** An outgoing message, in flight. Returns its id for [settle]. */
    fun outgoing(peerKey: String, text: String): Long {
        synchronized(lock) {
            val c = convos.getOrPut(peerKey) { Convo(peerKey) }
            val id = seq++
            c.messages.add(Message(id, true, "me", text, Delivery.SENDING, null))
            changed()
            return id
        }
    }

    /** The kernel's send returned: sent for carriage, or unsent with why. */
    fun settle(peerKey: String, id: Long, delivery: Delivery, reason: String?) {
        synchronized(lock) {
            val c = convos[peerKey] ?: return
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
