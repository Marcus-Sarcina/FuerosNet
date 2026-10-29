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
 * What this replaced: thirteen Activities, one screen each, no chrome of
 * their own and the system gesture for a way back. The design is a
 * persistent rail and foot with the content between them changing.
 *
 * **The launcher since 2026-10-29** [author]. The old Home — a list of
 * destinations — went with the screens that had no place in this
 * navigation: the Settings placeholder, and People, whose scanner and
 * own-code button are the meeting screen's and whose contacts are the
 * Direct screen's. What survives is reached from where the design puts
 * it: the meeting from the rail's own orb, direct messages from the one
 * above it, the catalog from inside a subnet, the serving node and the
 * operator's instances from Options, and authority from a subnet's own
 * options. So the launcher's duties are this screen's: the kernel is
 * brought up here, a provision arrives here, and the bench's levers land
 * here.
 *
 * **[Chrome] decides and this draws.** Which places keep the chrome, what
 * the rail holds, what the foot says and what an orb's mark is are all
 * there, where a test can hold them; the only decisions here are about
 * pixels.
 *
 * **What it does not do yet**, each one named rather than faked: the
 * content views are partial — a place carries the rows that reach the
 * screens already built and says what it does not hold — and the unseen
 * counts are zero everywhere, nothing in the kernel or this shell
 * tracking read state. **All three crowns are real**, out of
 * `subnet_standing` [2026-10-28].
 */
class ShellActivity : Activity() {

    private lateinit var railColumn: LinearLayout
    private lateinit var content: FrameLayout
    private lateinit var footBar: LinearLayout
    private lateinit var rail: ScrollView

    private var place: Chrome.Place = Chrome.Place.Home

    /** The subnets as the rail last read them, for a place that names one. */
    private var scopes: List<uniffi.rhtn_model.Subnet> = emptyList()

    /** The Options row that reads the connection, while Options is shown. */
    private var connection: TextView? = null

    /**
     * Whether the rail and the foot have been drawn from a kernel that was
     * up. **The launcher starts the kernel and draws before it has come
     * up** — a foot read then is a blank mark and an ellipsis for a name,
     * which is what the first launch on glass showed [2026-10-29] — so
     * the first status the kernel reports redraws them.
     */
    private var drawnFromKernel = false
    private val reading = java.util.concurrent.atomic.AtomicBoolean(false)

    /** The status the rail was last read at: a change is an attach, a
     *  detach or a failover, each of which can bring names with it. */
    @Volatile private var readAt: String? = null

    private val sink = object : Front.Ui {
        override fun render() {
            val line = Kernel.front.status()
            runOnUiThread { connection?.text = Attach.statusLine(line) }
            // **the rail follows the connection**: the attach fetches
            // profiles, and a subnet named by its root arrived after the
            // rail was drawn and stayed eight hex characters until the
            // next start [bench, 2026-10-29]
            if (!drawnFromKernel || line != readAt) readRail()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val c = Palette.now
        // **the chrome is the theme's** [2026-10-29]: `@style/Fueros` drops
        // the title bar and colours the system bars once for every screen

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

        // where a provision arrives, and where the kernel is brought up:
        // both are the launcher's, once
        intent.getStringExtra("provision")?.let { Kernel.provision(this, it) }
        // **no identity yet is the first run's screen** [author,
        // 2026-10-29], not a kernel minting one silently: the person
        // chooses new or restored, sees the material and the phrase, and
        // names themselves; this screen is what they land on after
        if (!Kernel.hasIdentity(this)) {
            startActivity(Intent(this, FirstRunActivity::class.java))
            finish()
            return
        }
        // the bench's live stream target, taken in the fieldtest flavour
        // alone (`FuerosApp.streamTo`); the releasable never reads it
        if (BuildConfig.FIELD_TEST) {
            intent.getStringExtra("diag_stream")?.let {
                FuerosApp.streamTo(this, it, intent.getStringExtra("diag_serial"))
            }
        }
        Kernel.start(this)
        benchLevers(intent)
        go(Chrome.Place.Home)
    }

    /**
     * **A second start lands here, not in [onCreate]**: `am start` on a
     * task whose root this activity already is delivers the intent to the
     * running instance (`singleTop`), which is how the bench reached a
     * phone sitting on its Done screen and nothing happened (run
     * `0c8c8f8-a3-node-1`, 2026-10-24).
     */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        benchLevers(intent)
    }

    /**
     * The bench's levers on the peer exchange (design §6.3), fieldtest
     * alone: `--es forget_contact <keyhash>` takes one contact out of the
     * dataset, `--es forget_captures <keyhash>` drops every sealed capture
     * held of that party, `--ez contact_peers true` reaches for the peers
     * now.
     */
    private fun benchLevers(intent: Intent) {
        if (!BuildConfig.FIELD_TEST) return
        intent.getStringExtra("forget_contact")?.let { Kernel.forgetContact(it) }
        intent.getStringExtra("forget_captures")?.let { Kernel.forgetCaptures(it) }
        if (intent.getBooleanExtra("contact_peers", false)) Kernel.contactPeersNow()
    }

    override fun onStart() {
        super.onStart()
        Consent.host(this)
        Kernel.bind(sink)
        readRail()
    }

    /**
     * **Off the drawing thread**: the rail is built from the subnets the
     * kernel holds, and reading them is a blocking call into a client
     * that may be parked in `nextEvent`. One read at a time; a render
     * that arrives while one is out is not a second one.
     */
    private fun readRail() {
        if (!reading.compareAndSet(false, true)) return
        readAt = Kernel.front.status()
        Thread({
            // **ask for what each contact and each subnet published**
            // (`light-client-requirements.md` §4.3): this is the screen
            // that shows names, so it is where the asking belongs — it was
            // the People screen's until that went [2026-10-29]
            Kernel.requestProfiles()
            val rows = Kernel.subnets()
            val mine = Kernel.ownAlias()
            // **a kernel up with no default name is a first run that was
            // cut short**: the name is required [author, 2026-10-29], and
            // the seeds are sealed before it is asked, so the name step
            // is where this device goes
            val unnamed = mine.second.isNotEmpty() && Kernel.defaultAlias() == null
            runOnUiThread {
                reading.set(false)
                if (isFinishing) return@runOnUiThread
                if (unnamed) {
                    startActivity(
                        Intent(this, FirstRunActivity::class.java)
                            .putExtra(FirstRunActivity.NAME_ONLY, true),
                    )
                    finish()
                    return@runOnUiThread
                }
                // a keyhash is what a kernel that is up answers with
                if (mine.second.isNotEmpty()) drawnFromKernel = true
                scopes = rows
                drawRail(rows)
                drawFoot(mine)
            }
        }, "fueros-rail").start()
    }

    override fun onStop() {
        Kernel.unbind(sink)
        Consent.release(this)
        super.onStop()
    }

    /** Go to `where`, and show or hide the chrome as [Chrome] says. */
    private fun go(where: Chrome.Place) {
        place = where
        val on = Chrome.chrome(where)
        rail.visibility = if (on) View.VISIBLE else View.GONE
        footBar.visibility = if (on) View.VISIBLE else View.GONE
        connection = null
        content.removeAllViews()
        content.addView(view(where))
    }

    /**
     * **Back is out of a chromeless place and no further.** From Home it
     * is the system's, which leaves the app; from a subnet's options it
     * is that subnet; from anywhere else chromeless it is Home, the way
     * the rail would have been.
     */
    @Deprecated("the platform's own, which this overrides deliberately")
    override fun onBackPressed() {
        val here = place
        when {
            Chrome.chrome(here) -> {
                @Suppress("DEPRECATION")
                super.onBackPressed()
            }
            here is Chrome.Place.SubnetOptions -> go(Chrome.Place.Subnet(here.id))
            else -> go(Chrome.Place.Home)
        }
    }

    // ---- the rail ------------------------------------------------------

    private fun drawRail(subnets: List<uniffi.rhtn_model.Subnet>) {
        railColumn.removeAllViews()
        val rungs = Chrome.rail(
            // **zero, and not a guess**: nothing tracks read state yet
            unseenDirect = 0,
            subnets = subnets.map {
                Chrome.Rung.Subnet(
                    id = it.id.hex(),
                    alias = it.alias,
                    unseen = 0,
                    crown = Chrome.crown(
                        root = it.standing.root,
                        operator = it.standing.operator,
                        hasSubordinate = it.standing.subordinates > 0u,
                    ),
                )
            },
        )
        for (rung in rungs) railColumn.addView(rungView(rung))
    }

    /**
     * **Two orbs open a screen of their own rather than a place**, until
     * the content views land: direct messages are still the
     * conversations screen, and a meeting owns the screen in any case.
     */
    private fun rungView(rung: Chrome.Rung): View = when (rung) {
        is Chrome.Rung.Divider -> View(this).apply {
            setBackgroundColor(Palette.RAIL_DIVIDER)
            layoutParams = LinearLayout.LayoutParams(dp(30), dp(1))
                .apply { setMargins(0, dp(3), 0, dp(13)) }
        }
        is Chrome.Rung.Direct -> orb(rung, "DM") {
            startActivity(Intent(this, ConversationsActivity::class.java))
        }
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

    // ---- the content region --------------------------------------------

    /**
     * **What each place holds, and what it says it does not.** A place
     * carries the rows that reach the screens already built, where the
     * design puts them, and names the rest rather than faking it, so the
     * shell can be walked and judged without anything pretending to be a
     * view it is not.
     */
    private fun view(where: Chrome.Place): View {
        val column = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        when (where) {
            is Chrome.Place.Home -> {
                column.addView(
                    owed(
                        "Home — every subnet's notices, newest first, each saying where it " +
                            "is from.\n\nOwed: the kernel's notices are a ring of twelve strings " +
                            "in memory, unpersisted and without the patron, subnet, resource " +
                            "and type this line needs.",
                    ),
                )
            }
            is Chrome.Place.Subnet -> {
                val scope = scopes.firstOrNull { it.id.hex() == where.id }
                column.addView(
                    bar(scope?.alias ?: "subnet ${where.id.take(8)}…", back = null) {
                        go(Chrome.Place.SubnetOptions(where.id))
                    }.apply { contentDescription = "This subnet's options" },
                )
                column.addView(
                    row(
                        "Resources",
                        "the catalog as swept from your serving node — which of them " +
                            "belong to this subnet is not a question it can answer yet",
                    ) { startActivity(Intent(this, CatalogActivity::class.java)) },
                )
                column.addView(
                    owed(
                        "The board — one per subnet, every post saying how far it " +
                            "travels — is the shell's own and not yet built.",
                    ),
                )
            }
            is Chrome.Place.Direct -> {
                // not reached: the rail's orb opens the conversations
                // screen until the Direct view is built
                column.addView(owed("Direct — recent threads, searched by alias or hash, and the contacts list."))
            }
            is Chrome.Place.Options -> {
                column.addView(bar("Options", back = Chrome.Place.Home))
                val line = TextView(this).apply {
                    textSize = 12f
                    typeface = Typeface.MONOSPACE
                    setTextColor(Palette.now.sub)
                    text = Attach.statusLine(Kernel.front.status())
                }
                connection = line
                column.addView(
                    row("Serving node", null, line) {
                        startActivity(Intent(this, AttachActivity::class.java))
                    },
                )
                column.addView(
                    row("Instances you run", "enrol one, launch one at a provider, its pages, its run") {
                        startActivity(Intent(this, OperatorActivity::class.java))
                    },
                )
                column.addView(
                    owed(
                        "Owed here: your name and picture, appearance (dark or light), " +
                            "retention, the wake endpoint, backup and recovery. Most of it " +
                            "crosses the boundary already; it wants this screen.",
                    ),
                )
            }
            is Chrome.Place.SubnetOptions -> {
                val scope = scopes.firstOrNull { it.id.hex() == where.id }
                val s = scope?.standing
                column.addView(
                    bar(scope?.alias ?: "subnet ${where.id.take(8)}…", back = Chrome.Place.Subnet(where.id)),
                )
                column.addView(
                    row(
                        "Authority — who you are under, and what leaving costs",
                        "every line this key holds, not this subnet's alone; tap one for the cost of ending it",
                    ) { startActivity(Intent(this, LeavingActivity::class.java)) },
                )
                column.addView(owed(Shell.downline(s)))
                if (s?.root == true) column.addView(nameSubnet(where.id, scope?.alias))
                if (s?.operator == true) {
                    column.addView(
                        row("Your instance here", "its administration pages, through the operator's own section") {
                            startActivity(Intent(this, OperatorActivity::class.java))
                        },
                    )
                }
                column.addView(
                    owed(
                        "Owed here: this subnet's notices, on or off; the upline drawn as a " +
                            "line to the root; and for the root, the name and the logo.",
                    ),
                )
            }
            is Chrome.Place.Thread -> column.addView(owed("One thread, full screen."))
            is Chrome.Place.Resource -> column.addView(owed("A resource's page, in its frame."))
            is Chrome.Place.Meet -> column.addView(owed("Meeting somebody."))
        }
        return ScrollView(this).apply { addView(column) }
    }

    /**
     * A place's title, with a back control where the chrome is hidden
     * ([Chrome.back]) and nothing where it is not; `go` is what tapping
     * the title does, where it does anything.
     */
    private fun bar(title: String, back: Chrome.Place?, go: (() -> Unit)? = null): View {
        val c = Palette.now
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(8), dp(14), dp(16), dp(10))
            if (back != null) {
                addView(
                    TextView(this@ShellActivity).apply {
                        text = "‹"
                        textSize = 26f
                        setTextColor(c.ink)
                        setPadding(dp(8), 0, dp(14), dp(2))
                        contentDescription = "Back"
                        isClickable = true
                        setOnClickListener { this@ShellActivity.go(back) }
                    },
                )
            } else {
                setPadding(dp(16), dp(14), dp(16), dp(10))
            }
            addView(
                TextView(this@ShellActivity).apply {
                    text = title
                    textSize = 19f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(c.ink)
                    if (go != null) {
                        isClickable = true
                        setOnClickListener { go() }
                    }
                },
            )
        }
    }

    /** One row that goes somewhere: a label, and beneath it what that is. */
    private fun row(label: String, sub: String?, subView: TextView? = null, go: () -> Unit): View {
        val c = Palette.now
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(16), dp(14), dp(16), dp(14))
            isClickable = true
            setOnClickListener { go() }
            addView(
                TextView(this@ShellActivity).apply {
                    text = label
                    textSize = 16f
                    setTextColor(c.ink)
                },
            )
            val under = subView ?: sub?.let {
                TextView(this@ShellActivity).apply {
                    text = it
                    textSize = 12f
                    setTextColor(c.sub)
                }
            }
            under?.let { addView(it.apply { setPadding(0, dp(3), 0, 0) }) }
        }
    }

    /**
     * **The root names the subnet** (`Robot/light-client-screens.md`,
     * subnet options: *the subnet's name becomes editable* for the root)
     * — by convention, which `light-client-requirements.md` §4.4 makes
     * the root's while nothing enforces it, so the row says so. The same
     * publication the Done screen asks for at formation; here for a
     * subnet formed before that prompt existed, or renamed.
     */
    private fun nameSubnet(id: String, current: String?): View {
        val c = Palette.now
        val column = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(16), dp(10), dp(16), dp(10))
        }
        val field = android.widget.EditText(this).apply {
            hint = current ?: "the subnet's name"
            textSize = 16f
            setSingleLine()
        }
        val said = TextView(this).apply {
            textSize = 12f
            setTextColor(c.sub)
            text = "You are its root, so its name is yours to set — by convention; nothing enforces it."
        }
        column.addView(said)
        column.addView(field)
        column.addView(
            android.widget.Button(this).apply {
                text = if (current == null) "Name the subnet" else "Rename the subnet"
                setOnClickListener {
                    val typed = field.text.toString().trim()
                    Identity.aliasRefusal(typed)?.let { said.text = it; return@setOnClickListener }
                    said.text = "publishing…"
                    Thread({
                        val why = Kernel.publishSubnetAlias(id, typed)
                        runOnUiThread {
                            if (isFinishing) return@runOnUiThread
                            said.text = why?.let { "not published: $it" } ?: "Named $typed; published to your node."
                            if (why == null) readRail()
                        }
                    }, "fueros-subnet-name").start()
                }
            },
        )
        return column
    }

    /** What a place does not hold yet, said in the muted ink. */
    private fun owed(what: String): View = TextView(this).apply {
        text = what
        textSize = 14f
        setTextColor(Palette.now.muted)
        setPadding(dp(16), dp(12), dp(16), dp(16))
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
