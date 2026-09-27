package com.comptus.fueros

/**
 * The kernel's front: what a screen binds to, kept apart from the kernel
 * so it depends on nothing a JVM test cannot hold — no Android, no
 * binding, no participant.
 *
 * One screen is bound at a time; a screen that has gone is a null sink; a
 * replacement is given the state so far and then everything new. The
 * transcript outlives every screen, so it is bounded — an unbounded one is
 * a leak the process never recovers from.
 */
class Front {

    /** What a bound screen renders. Calls arrive on the kernel's thread. */
    interface Ui {
        fun status(line: String)

        fun say(line: String)
    }

    private val lock = Any()
    private var ui: Ui? = null
    private var statusLine = "starting the kernel…"
    private val transcript = mutableListOf<String>()

    /**
     * Bind a screen: it receives the state so far, then everything new.
     *
     * Under the lock, so that a line the kernel is saying at this moment
     * arrives either in the replay or after it and never in both places or
     * out of order.
     */
    fun bind(u: Ui) {
        synchronized(lock) {
            ui = u
            u.status(statusLine)
            transcript.forEach(u::say)
        }
    }

    /**
     * Unbind a screen, if it is still the one bound: a stale unbind from a
     * screen already replaced must not silence its replacement.
     */
    fun unbind(u: Ui) {
        synchronized(lock) {
            if (ui === u) {
                ui = null
            }
        }
    }

    // **Under the lock, and the bound screen told inside it.**  What a
    // screen has been told and what the transcript holds then cannot part
    // company across a bind, which is the only thing that makes the replay
    // exact.  A sink posts to its own thread and never calls back in here,
    // so nothing waits on anything.
    fun status(line: String) {
        synchronized(lock) {
            statusLine = line
            ui?.status(line)
        }
    }

    fun say(line: String) {
        synchronized(lock) {
            transcript.add(line)
            // the transcript outlives every screen, so it is bounded: an
            // unbounded one is a leak the process never recovers from
            while (transcript.size > TRANSCRIPT_LINES) {
                transcript.removeAt(0)
            }
            ui?.say(line)
        }
    }

    private companion object {
        const val TRANSCRIPT_LINES = 500
    }
}
