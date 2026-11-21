package com.comptus.fueros

/**
 * **Where the person has been in the shell, and where back goes** [author,
 * 2026-11-20] — the pure part, so a test can hold it, and the same model
 * an iOS shell's edge swipe pops. It names no platform type: the place is
 * whatever the shell calls one ([Chrome.Place] on Android), and the
 * platform's back event is wired to [back] by the screen that draws it.
 *
 * **Back is the previous place; with none left it is home; only from home
 * with none left is it the platform's**, which on Android leaves the app
 * with its own predictive animation. [holding] is that last test, which
 * the screen reads to decide whether to take back at all.
 *
 * What a visit does to the history, by what it is:
 *
 * - **to where you already are**: nothing. A redraw of the same place is
 *   not a visit, so it cannot be backed out of.
 * - **to home**: the history is emptied. Home is the root the rail and the
 *   foot return to, so arriving there is arriving at the bottom.
 * - **to a place already in the history**: the history is cut back to
 *   just before it, as though back had been pressed until it was reached.
 *   So a place is held at most once, and a loop between two places does
 *   not grow it. A title bar's `‹` is back itself [author, 2026-11-20].
 * - **anywhere else**: where you were is kept, newest last, and the oldest
 *   is dropped beyond [depth]. Back past the oldest kept is home.
 */
class History<P : Any>(private val home: P, private val depth: Int = DEPTH) {

    init {
        require(depth > 0) { "a history holds at least one place" }
    }

    /** The place being shown. */
    var here: P = home
        private set

    /** The places before it, oldest first; home, where kept, is the bottom. */
    private val before = ArrayDeque<P>()

    /** The places before [here], oldest first, for a test to read. */
    val trail: List<P> get() = before.toList()

    /** **Whether back is the shell's**: anything to go back to, or not yet home. */
    val holding: Boolean get() = before.isNotEmpty() || here != home

    /** Go to `to`, recording the visit as the class comment says. */
    fun go(to: P) {
        if (to == here) return
        if (to == home) {
            before.clear()
        } else {
            val at = before.indexOf(to)
            if (at >= 0) {
                while (before.size > at) before.removeLast()
            } else {
                before.addLast(here)
                while (before.size > depth) before.removeFirst()
            }
        }
        here = to
    }

    /**
     * **Back one place**: the one before, else home; null where back is
     * not the shell's ([holding] false), the caller then leaving it to the
     * platform.
     */
    fun back(): P? {
        if (!holding) return null
        here = before.removeLastOrNull() ?: home
        return here
    }

    private companion object {
        /** How many places back are kept; more is a path nobody retraces. */
        const val DEPTH = 32
    }
}
