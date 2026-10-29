package com.comptus.fueros

import android.app.Activity
import android.content.Intent
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **The shell: one screen, a rail down the left and a bar along the
 * foot** (`Robot/light-client-screens.md`, *the shell*).
 *
 * What this replaces: thirteen Activities, one screen each, no chrome of
 * their own and the system gesture for a way back. The design is a
 * persistent rail and foot with the content between them changing —
 * which is a different shape, not another screen, so it is built beside
 * the old one and takes over when it carries more than the old one does.
 *
 * **Reachable from Home while it is partial** [2026-10-28]. Making it
 * the launcher now would take away screens it cannot yet draw; a row on
 * Home costs nothing and the launcher moves in one line when the content
 * views have landed. The old Activities keep working throughout.
 *
 * **[Chrome] decides and this draws.** Which places keep the chrome, what
 * the rail holds, what the foot says and what an orb's mark is are all
 * there, where a test can hold them; the only decisions here are about
 * pixels.
 *
 * **What it does not do yet**, each one named rather than faked: the
 * content region holds a placeholder per place, because the content
 * views are the next piece; and the unseen counts are zero everywhere,
 * nothing in the kernel or this shell tracking read state. **All three
 * crowns are real**, out of `subnet_standing` [2026-10-28].
 */
class ShellActivity : Activity() {

    private lateinit var railColumn: LinearLayout
    private lateinit var content: FrameLayout
    private lateinit var footBar: LinearLayout
    private lateinit var rail: ScrollView

    private var place: Chrome.Place = Chrome.Place.Home

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val c = Palette.now
        // **the platform's chrome is what this replaces** [2026-10-28]:
        // the title bar said *Fueros* above every screen and the system
        // bars kept their own colours, so the first thing the drawn
        // design met was a light grey bar under its dark foot. The title
        // goes, and the bars take the shell's own.
        requestWindowFeature(android.view.Window.FEATURE_NO_TITLE)
        window.statusBarColor = c.chrome
        window.navigationBarColor = c.sunken

        railColumn = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, dp(10), 0, dp(10))
        }
        rail = ScrollView(this).apply {
            setBackgroundColor(Palette.RAIL)
            isVerticalScrollBarEnabled = false
            layoutParams = LinearLayout.LayoutParams(dp(RAIL_DP), LinearLayout.LayoutParams.MATCH_PARENT)
            addView(railColumn)
        }
        content = FrameLayout(this).apply {
            setBackgroundColor(c.ground)
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT)
                .apply { weight = 1f }
        }
        val middle = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
            ).apply { weight = 1f }
            addView(rail)
            addView(content)
        }
        footBar = foot()

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(c.ground)
            addView(middle)
            addView(footBar)
        }
        Screen.insetting(root)
        setContentView(root)
        // **after the content, not before**: the decor view does not
        // exist until then, and `window.insetsController` is null until
        // it does — which is a crash rather than a no-op
        bars(light = Palette.mode == Palette.Mode.LIGHT)
        go(Chrome.Place.Home)
    }

    override fun onStart() {
        super.onStart()
        // **off the drawing thread**: the rail is built from the subnets
        // the kernel holds, and reading them is a blocking call into a
        // client that may be parked in `nextEvent`
        Thread({
            val rows = Kernel.scopes()
            val standing = Kernel.standings().associateBy { it.subnet }
            val mine = Kernel.ownAlias()
            runOnUiThread {
                if (!isFinishing) {
                    drawRail(rows, standing)
                    drawFoot(mine)
                }
            }
        }, "fueros-rail").start()
    }

    /** Go to `where`, and show or hide the chrome as [Chrome] says. */
    private fun go(where: Chrome.Place) {
        place = where
        val on = Chrome.chrome(where)
        rail.visibility = if (on) View.VISIBLE else View.GONE
        footBar.visibility = if (on) View.VISIBLE else View.GONE
        content.removeAllViews()
        content.addView(placeholder(where))
    }

    /**
     * **Back is out of a chromeless place and no further.** From Home it
     * is the system's, which leaves the app; from a thread or a
     * resource it is the way the rail would have been.
     */
    @Deprecated("the platform's own, which this overrides deliberately")
    override fun onBackPressed() {
        if (Chrome.chrome(place)) {
            @Suppress("DEPRECATION")
            super.onBackPressed()
        } else {
            go(Chrome.Place.Home)
        }
    }

    // ---- the rail ------------------------------------------------------

    private fun drawRail(subnets: List<Meet.Scope>, standing: Map<String, Kernel.Standing>) {
        railColumn.removeAllViews()
        val rungs = Chrome.rail(
            // **zero, and not a guess**: nothing tracks read state yet
            unseenDirect = 0,
            subnets = subnets.map {
                val s = standing[it.id]
                Chrome.Rung.Subnet(
                    id = it.id,
                    alias = it.alias,
                    unseen = 0,
                    crown = Chrome.crown(
                        root = s?.root == true,
                        operator = s?.operator == true,
                        hasSubordinate = (s?.subordinates ?: 0) > 0,
                    ),
                )
            },
        )
        for (rung in rungs) railColumn.addView(rungView(rung))
    }

    private fun rungView(rung: Chrome.Rung): View = when (rung) {
        is Chrome.Rung.Divider -> View(this).apply {
            setBackgroundColor(Palette.RAIL_DIVIDER)
            layoutParams = LinearLayout.LayoutParams(dp(30), dp(1))
                .apply { setMargins(0, dp(3), 0, dp(13)) }
        }
        is Chrome.Rung.Direct -> orb(rung, "DM") { go(Chrome.Place.Direct) }
        is Chrome.Rung.Meet -> orb(rung, "◎") {
            startActivity(Intent(this, MeetActivity::class.java))
        }
        is Chrome.Rung.Subnet -> orb(rung, Chrome.mark(rung.alias, rung.id)) {
            go(Chrome.Place.Subnet(rung.id))
        }
    }

    /**
     * One orb, with its crown beneath it and its count above.
     *
     * **The orb fills the rail less a hair each side** [author,
     * 2026-10-27], and the crown is smaller than the orb and low, so the
     * mark appears to flower out of it rather than be ringed by it.
     */
    private fun orb(rung: Chrome.Rung, mark: String, go: () -> Unit): View {
        val d = dp(RAIL_DP - 2)
        val slot = FrameLayout(this).apply {
            layoutParams = LinearLayout.LayoutParams(d, d)
                .apply { setMargins(0, 0, 0, dp(11)) }
            contentDescription = Chrome.said(rung)
            isClickable = true
            setOnClickListener { go() }
        }
        val crown = when (rung) {
            is Chrome.Rung.Subnet -> rung.crown
            else -> Chrome.Crown.NONE
        }
        slot.addView(
            View(this).apply {
                background = Crowns.behind(crown)
                layoutParams = FrameLayout.LayoutParams(d, d)
            },
        )
        slot.addView(
            TextView(this).apply {
                text = mark
                textSize = 21f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor(Palette.RAIL_INK)
                gravity = Gravity.CENTER
                background = Crowns.disc(Palette.RAIL_ORB)
                layoutParams = FrameLayout.LayoutParams(d, d)
            },
        )
        slot.addView(
            View(this).apply {
                background = Crowns.front(crown)
                layoutParams = FrameLayout.LayoutParams(d, d)
            },
        )
        Chrome.badge(unseenOf(rung))?.let { n ->
            slot.addView(
                TextView(this).apply {
                    text = n
                    textSize = 10f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Palette.RAIL_INK)
                    gravity = Gravity.CENTER
                    background = Crowns.disc(Palette.COUNT)
                    layoutParams = FrameLayout.LayoutParams(dp(18), dp(18), Gravity.TOP or Gravity.END)
                },
            )
        }
        return slot
    }

    private fun unseenOf(rung: Chrome.Rung): Int = when (rung) {
        is Chrome.Rung.Direct -> rung.unseen
        is Chrome.Rung.Subnet -> rung.unseen
        else -> 0
    }

    // ---- the foot ------------------------------------------------------

    private fun foot(): LinearLayout {
        val c = Palette.now
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(c.sunken)
            setPadding(dp(10), dp(8), dp(10), dp(8))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            )
        }
    }

    private fun drawFoot(mine: Pair<String?, String>) {
        val c = Palette.now
        val (alias, me) = mine
        val (name, says) = Chrome.foot(alias, me)
        footBar.removeAllViews()
        footBar.addView(
            TextView(this).apply {
                text = Chrome.mark(alias, me)
                textSize = 14f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor(c.accent)
                gravity = Gravity.CENTER
                background = Crowns.rounded(c.control, dp(13))
                layoutParams = LinearLayout.LayoutParams(dp(40), dp(40))
                contentDescription = "You, and this device's options"
                isClickable = true
                setOnClickListener { go(Chrome.Place.Options) }
            },
        )
        footBar.addView(
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                background = Crowns.rounded(c.ground, dp(12))
                setPadding(dp(12), dp(10), dp(12), dp(10))
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT)
                    .apply { weight = 1f; setMargins(dp(10), 0, 0, 0) }
                isClickable = true
                setOnClickListener { go(Chrome.Place.Home) }
                addView(
                    TextView(this@ShellActivity).apply {
                        text = name
                        textSize = 14f
                        typeface = Typeface.DEFAULT_BOLD
                        setTextColor(c.ink)
                        layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT)
                            .apply { weight = 1f }
                    },
                )
                addView(
                    TextView(this@ShellActivity).apply {
                        text = says
                        textSize = 11f
                        setTextColor(c.sub)
                    },
                )
            },
        )
    }

    // ---- the content region, which is the next piece -------------------

    /**
     * **Named, not faked.** Each place says what it will hold and that it
     * does not hold it yet, so the container can be walked and judged
     * without anything pretending to be a screen.
     */
    private fun placeholder(where: Chrome.Place): View {
        val c = Palette.now
        val what = when (where) {
            is Chrome.Place.Home ->
                "Home — every subnet's notices, newest first, each saying where it is from.\n\n" +
                    "Owed: the kernel's notices are a ring of twelve strings in memory, " +
                    "unpersisted and without the patron, subnet, resource and type this line needs."
            is Chrome.Place.Subnet ->
                "A subnet — its resources across the top, its board beneath.\n\n" +
                    "Owed: which resources belong to a subnet is not a question the catalog " +
                    "can answer yet, and the board is the shell's own and not yet built."
            is Chrome.Place.Direct ->
                "Direct — recent threads, searched by alias or hash, and the contacts list.\n\n" +
                    "The contacts are held; a thread list with its last message is not."
            is Chrome.Place.Options ->
                "Options — your name and picture, this device's settings, backup and recovery.\n\n" +
                    "Most of it crosses the boundary already; it wants this shell's chrome."
            is Chrome.Place.SubnetOptions -> "This subnet's options."
            is Chrome.Place.Thread -> "One thread, full screen."
            is Chrome.Place.Resource -> "A resource's page, in its frame."
            is Chrome.Place.Meet -> "Meeting somebody."
        }
        return ScrollView(this).apply {
            addView(
                TextView(this@ShellActivity).apply {
                    text = what
                    textSize = 14f
                    setTextColor(c.muted)
                    setPadding(dp(16), dp(20), dp(16), dp(32))
                },
            )
        }
    }

    /**
     * Whether the system bars draw their icons dark, which they must
     * where the bar behind them is light. **Asked of the mode rather
     * than of the colour**, so one switch moves both.
     */
    private fun bars(light: Boolean) {
        val mask = android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
            android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
        window.insetsController?.setSystemBarsAppearance(if (light) mask else 0, mask)
    }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()

    private companion object {
        /** The rail's width, which the orbs fill less a hair each side. */
        const val RAIL_DP = 68
    }
}

/**
 * **The rail's shapes, drawn rather than drawable** — the shell carries no
 * XML, so an orb and its crown are a `Path` on a `Canvas`.
 *
 * The crown is in two halves: the band's back passes behind the orb's
 * foot, and its front sweep crosses just over it, so the orb sits in it as
 * in a stand [author, 2026-10-27]. Only the full crown has points, and
 * only its three frontmost show.
 */
object Crowns {

    /** A filled circle, for an orb or a count. */
    fun disc(colour: Int): Drawable = object : Drawable() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { this.color = colour }
        override fun draw(canvas: Canvas) {
            val b = bounds
            canvas.drawCircle(b.exactCenterX(), b.exactCenterY(), minOf(b.width(), b.height()) / 2f, paint)
        }
        override fun setAlpha(alpha: Int) {}
        override fun setColorFilter(f: android.graphics.ColorFilter?) {}
        @Deprecated("the platform's own abstract member")
        override fun getOpacity(): Int = android.graphics.PixelFormat.TRANSLUCENT
    }

    /** A rounded rectangle, for the foot's bar and its icon. */
    fun rounded(colour: Int, radius: Int): Drawable = object : Drawable() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { this.color = colour }
        override fun draw(canvas: Canvas) {
            val b = bounds
            canvas.drawRoundRect(
                b.left.toFloat(), b.top.toFloat(), b.right.toFloat(), b.bottom.toFloat(),
                radius.toFloat(), radius.toFloat(), paint,
            )
        }
        override fun setAlpha(alpha: Int) {}
        override fun setColorFilter(f: android.graphics.ColorFilter?) {}
        @Deprecated("the platform's own abstract member")
        override fun getOpacity(): Int = android.graphics.PixelFormat.TRANSLUCENT
    }

    /** The half of the band that passes behind the orb. */
    fun behind(crown: Chrome.Crown): Drawable = band(crown, front = false)

    /** The sweep that crosses in front of its foot, with the points. */
    fun front(crown: Chrome.Crown): Drawable = band(crown, front = true)

    private fun colourOf(crown: Chrome.Crown): Int? = when (crown) {
        Chrome.Crown.NONE -> null
        Chrome.Crown.SILVER -> Palette.SILVER
        Chrome.Crown.GOLD, Chrome.Crown.FULL -> Palette.GOLD
    }

    private fun band(crown: Chrome.Crown, front: Boolean): Drawable = object : Drawable() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeCap = Paint.Cap.ROUND
        }
        private val fill = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL }

        override fun draw(canvas: Canvas) {
            val colour = colourOf(crown) ?: return
            paint.color = colour
            fill.color = colour
            val b = bounds
            val w = b.width().toFloat()
            val h = b.height().toFloat()
            paint.strokeWidth = if (crown == Chrome.Crown.SILVER) w * 0.023f else w * 0.030f
            // the band's ends sit on the orb's lower third, and it bows
            // away from the orb behind and across it in front
            val x0 = b.left + w * 0.197f
            val x1 = b.left + w * 0.803f
            val y = b.top + h * 0.788f
            val bow = if (front) b.top + h * 1.0f else b.top + h * 0.576f
            val p = Path().apply {
                moveTo(x0, y)
                quadTo(b.exactCenterX(), bow, x1, y)
            }
            if (front && crown == Chrome.Crown.FULL) {
                // the three frontmost of eight points; the rest are behind
                // an orb much larger than the crown
                for ((a, b2, c) in POINTS) {
                    canvas.drawPath(
                        Path().apply {
                            moveTo(b.left + w * a.first, b.top + h * a.second)
                            lineTo(b.left + w * b2.first, b.top + h * b2.second)
                            lineTo(b.left + w * c.first, b.top + h * c.second)
                            close()
                        },
                        fill,
                    )
                }
            }
            canvas.drawPath(p, paint)
        }

        override fun setAlpha(alpha: Int) {}
        override fun setColorFilter(f: android.graphics.ColorFilter?) {}
        @Deprecated("the platform's own abstract member")
        override fun getOpacity(): Int = android.graphics.PixelFormat.TRANSLUCENT
    }

    /**
     * The three visible points, as fractions of the orb's box: each a
     * plain filled triangle whose base follows the band.
     *
     * **Lowered after seeing them on a phone** [2026-10-28]: the middle
     * point peaked at 0.65 of the orb and reached into the mark's
     * glyphs, which is more than *only the lowermost part of the circle*
     * [author, 2026-10-27]. The peaks sit a sixteenth lower, clear of a
     * centred mark.
     */
    private val POINTS = listOf(
        Triple(0.203f to 0.792f, 0.286f to 0.757f, 0.370f to 0.874f),
        Triple(0.409f to 0.885f, 0.500f to 0.712f, 0.591f to 0.885f),
        Triple(0.630f to 0.874f, 0.714f to 0.757f, 0.797f to 0.792f),
    )
}
