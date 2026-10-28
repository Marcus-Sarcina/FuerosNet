package com.comptus.fueros

/**
 * **Every colour the shell draws, named once** (the screens sheet's G:
 * *dark by default, light an option in the user's own options* [author,
 * 2026-10-27]).
 *
 * What this replaces: nothing. The app has been drawing in the
 * platform's defaults — a light `#F1F0F7` ground it never chose, a
 * `TextView`'s own ink, and `Color.GRAY` where something needed to look
 * secondary. Two of those were under the contrast floor, which is most of
 * why the app reads as a diagnostic harness rather than an application.
 *
 * **The dark set is the drawing's own**, sampled from the canvas
 * artboards rather than re-invented. **The light set is derived**, the
 * same hues carried onto a light ground: there are no light artboards, so
 * it is a proposal to be redrawn rather than a design — it exists because
 * the ruling says the option must.
 *
 * **The floor is checked rather than asserted** ([PaletteTest]): every
 * token meant for text clears 4.5:1 against the surfaces it is drawn on.
 * That test found the one token that cannot — see [RULE_ONLY].
 *
 * **The rail keeps its own colours in both modes.** The russet field and
 * the crown on it are the network's mark (`Robot/light-client-screens.md`,
 * the crowns), not a themed surface: a light-mode rail would be a
 * different mark rather than the same one lit differently.
 */
object Palette {

    /** Which set is in force. Dark is what a person meets first. */
    enum class Mode { DARK, LIGHT }

    /**
     * **The one grey that cannot carry text**, kept so the drawing's
     * hairlines and disabled strokes have a name.
     *
     * The artboards use `#6E757E` for timestamps and small notes at 11px.
     * It is **3.55:1** on the dark ground, under the 4.5 floor, and
     * nothing between it and `sub` clears that floor — so there is no
     * third grey for text, only two, and this is what is left for rules
     * and for marks that say nothing.
     */
    const val RULE_ONLY = 0xFF6E757E.toInt()

    /** The colours of one mode. */
    data class Set(
        /** The page behind everything. */
        val ground: Int,
        /** The bar at the head of a screen, and the foot. */
        val chrome: Int,
        /** A raised surface: a field, a code block, a well. */
        val sunken: Int,
        /** A control's own ground. */
        val control: Int,
        /** Body text and anything that must be read. */
        val ink: Int,
        /** Secondary text: a subtitle, a value, a label. */
        val muted: Int,
        /** The dimmest text the floor allows **on the ground**, and
         *  nowhere else: see [Palette.DRAWN]. */
        val sub: Int,
        /** A hairline between rows. */
        val rule: Int,
        /** The one accent, for what a person may act on. */
        val accent: Int,
        /** On the accent, where it is a filled button. */
        val onAccent: Int,
        /** What wants attention without alarm. */
        val warn: Int,
        /** A field for an avatar with no picture. */
        val avatar: Int,
    )

    /** Sampled from the canvas artboards. */
    val DARK = Set(
        ground = 0xFF1C1F24.toInt(),
        chrome = 0xFF15171A.toInt(),
        sunken = 0xFF101215.toInt(),
        control = 0xFF2E3540.toInt(),
        ink = 0xFFE8EAED.toInt(),
        muted = 0xFF9AA1AA.toInt(),
        sub = 0xFF808891.toInt(),
        rule = 0xFF23272E.toInt(),
        accent = 0xFF5FB3A1.toInt(),
        onAccent = 0xFF10211E.toInt(),
        warn = 0xFFD08A3E.toInt(),
        avatar = 0xFF2E3540.toInt(),
    )

    /** Derived, not drawn: the same hues on a light ground. */
    val LIGHT = Set(
        ground = 0xFFF4F3F8.toInt(),
        chrome = 0xFFE6E5EE.toInt(),
        sunken = 0xFFFFFFFF.toInt(),
        control = 0xFFE0DFE9.toInt(),
        ink = 0xFF15171A.toInt(),
        muted = 0xFF4A5059.toInt(),
        sub = 0xFF5B626B.toInt(),
        rule = 0xFFD2D1DC.toInt(),
        accent = 0xFF246D5C.toInt(),
        onAccent = 0xFFFFFFFF.toInt(),
        warn = 0xFF8A4A08.toInt(),
        avatar = 0xFFD8D7E2.toInt(),
    )

    // ---- the rail, which is the mark and not a surface ----------------

    /** The russet field the crown sits on [author, 2026-10-27]. */
    const val RAIL = 0xFF6B2F2B.toInt()

    /** An orb with no picture of its own. */
    const val RAIL_ORB = 0xFF532320.toInt()

    /** Ink on the rail, warm against the russet. */
    const val RAIL_INK = 0xFFF0E2D6.toInt()

    /** The divider between what is pinned and what scrolls. */
    const val RAIL_DIVIDER = 0xFF8A4238.toInt()

    /** An unseen count's own ground, on an orb. */
    const val COUNT = 0xFF3E1A17.toInt()

    /** A golden circlet: a subnet where this person runs a node, and the
     *  full crown where they are its root. */
    const val GOLD = 0xFFD9A441.toInt()

    /** A silver circlet: a subnet where they have a subordinate. */
    const val SILVER = 0xFFC2C8CF.toInt()

    // ---- what is in force ---------------------------------------------

    /**
     * The mode in force. **Held here and nowhere else**, so that one
     * assignment restyles the shell; the person's choice is read from
     * their options at start and written back here.
     */
    @Volatile
    var mode: Mode = Mode.DARK

    /** The set in force. */
    val now: Set get() = if (mode == Mode.DARK) DARK else LIGHT

    /** The contrast floor for anything that must be read (WCAG AA). */
    const val FLOOR = 4.5

    /**
     * **Which ink goes on which surface, declared** [found 2026-10-28].
     *
     * Not every ink works on every surface, and no tuning makes it so:
     * [Set.sub] is the dimmest text the floor allows **on the ground**,
     * so any surface raised above the ground is already too close to it
     * — `sub` on `control` is 3.44:1 and the value that would fix it is
     * darker than the ground itself. The honest model is therefore a
     * declared set of pairs rather than a palette where anything may be
     * put on anything.
     *
     * **A pair the shell draws and this does not name is a pair nothing
     * checked**, which is what [PaletteTest] holds. Add the pair here
     * when a screen needs it, and the test will say whether it reads.
     */
    val DRAWN: List<Pair<String, String>> = listOf(
        // the page
        "ink" to "ground",
        "muted" to "ground",
        "sub" to "ground",
        "accent" to "ground",
        "warn" to "ground",
        // the head of a screen, and the foot
        "ink" to "chrome",
        "muted" to "chrome",
        "sub" to "chrome",
        "accent" to "chrome",
        // the foot's own well, and a code block
        "ink" to "sunken",
        "muted" to "sunken",
        "sub" to "sunken",
        "accent" to "sunken",
        // a control: a button, a field, an avatar with no picture.
        // **`sub` and `warn` are absent deliberately**: `sub` cannot work
        // on a raised surface at all, and the amber is 4.35:1 there — the
        // drawing uses it as ink on the page and as a filled button, never
        // as a label on a control, so the pair is undeclared rather than a
        // lighter amber invented for a case nothing asks for. A screen
        // that wants one adds the pair here and the test says what it
        // costs.
        "ink" to "control",
        "muted" to "control",
        "accent" to "control",
    )

    /** One named colour out of a set. */
    fun of(set: Set, name: String): Int = when (name) {
        "ground" -> set.ground
        "chrome" -> set.chrome
        "sunken" -> set.sunken
        "control" -> set.control
        "ink" -> set.ink
        "muted" -> set.muted
        "sub" -> set.sub
        "rule" -> set.rule
        "accent" -> set.accent
        "onAccent" -> set.onAccent
        "warn" -> set.warn
        "avatar" -> set.avatar
        else -> throw IllegalArgumentException("no colour named $name")
    }

    /** One channel of a colour, linearised. */
    private fun channel(c: Int): Double {
        val v = c / 255.0
        return if (v <= 0.03928) v / 12.92 else Math.pow((v + 0.055) / 1.055, 2.4)
    }

    /**
     * A colour's relative luminance (WCAG 2.2).
     *
     * **The channels are taken by hand rather than through
     * `android.graphics.Color`**: that class is a stub in a JVM unit
     * test and throws, and the point of keeping this arithmetic here is
     * that a test can run it. Every other pure part of this shell
     * touches no Android type for the same reason.
     */
    fun luminance(argb: Int): Double =
        0.2126 * channel((argb shr 16) and 0xFF) +
            0.7152 * channel((argb shr 8) and 0xFF) +
            0.0722 * channel(argb and 0xFF)

    /**
     * The contrast between two colours. **Kept here rather than in the
     * test** so the number the shell is held to is the number the shell
     * itself can state.
     */
    fun contrast(a: Int, b: Int): Double {
        val (x, y) = luminance(a) to luminance(b)
        val hi = maxOf(x, y)
        val lo = minOf(x, y)
        return (hi + 0.05) / (lo + 0.05)
    }
}
