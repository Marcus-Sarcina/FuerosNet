package com.comptus.fueros

import android.content.Context
import android.graphics.Typeface

/**
 * **The two typefaces, as the shell draws them** [author, 2026-11-18]: a
 * sans for people's words and the fixed navigation and headings, and a
 * mono for the machine's voice — system messages, logs, tooltips, a key
 * or an address. Geist and Geist Mono ([Tokens.SANS], [Tokens.MONO]),
 * read from `crates/brand/res/font` where the desktop reads them too.
 *
 * **A script they do not cover falls back to the system's own**, which is
 * the platform's font fallback doing what it does for any family: nothing
 * here chooses it.
 *
 * Loaded once when the application starts, because a [Typeface] from a
 * resource wants a [Context] and a screen setting `typeface = Type.mono`
 * should not have to pass one. Until then — a unit test, which never
 * starts the application — each is the platform's own.
 */
object Type {
    /** The sans at its regular weight. */
    @Volatile
    var sans: Typeface = Typeface.DEFAULT
        private set

    /** The sans at the weight a heading or a label that must stand out takes. */
    @Volatile
    var bold: Typeface = Typeface.DEFAULT_BOLD
        private set

    /** The mono, for the machine's voice. */
    @Volatile
    var mono: Typeface = Typeface.MONOSPACE
        private set

    /** Read the bundled faces; the platform's stand where one cannot be read. */
    fun load(context: Context) {
        val res = context.resources
        runCatching { res.getFont(R.font.geist) }.onSuccess {
            sans = it
            bold = Typeface.create(it, 700, false)
        }
        runCatching { res.getFont(R.font.geist_mono) }.onSuccess { mono = it }
    }
}
