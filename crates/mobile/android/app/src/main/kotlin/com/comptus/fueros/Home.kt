package com.comptus.fueros

/**
 * **What Home's status bar reads** — the pure part, so a test can hold it.
 *
 * The kernel composes the status line and this screen keeps its words
 * verbatim; what is added is the hint that the line is a destination,
 * because attaching to a serving node lives behind it [2026-10-27] and a
 * line with no hint is a line nobody taps.
 */
object Home {

    /** The first line of the kernel's status, with the hint after it. */
    fun statusBar(status: String): String {
        val line = status.substringBefore('\n')
        return "$line   ·   ${hint(line)}"
    }

    /** What tapping does, said for the state the line reports. */
    fun hint(line: String): String =
        when {
            line.startsWith("detached") || line.startsWith("lost") -> "tap to attach"
            line.startsWith("starting") -> "starting…"
            else -> "tap to change"
        }
}
