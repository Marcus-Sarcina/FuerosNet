package com.comptus.fueros

import android.app.Activity
import android.content.Intent
import android.graphics.Typeface
import android.os.Bundle
import android.view.Gravity
import android.widget.LinearLayout
import android.widget.TextView

/**
 * Home: navigation first, status on the periphery [author, 2026-09-27].
 *
 * The launcher and the one screen that brings the kernel up and hands it a
 * provision. Everything else is a destination reached from here. The
 * connection status sits in a thin bar at the top — present, not central —
 * and the body is the list of places to go.
 *
 * **The screen owns none of the kernel.** A font change recreates this
 * Activity; [Kernel] owns the participant for the life of the process and
 * this binds to its front while visible, for the status bar alone.
 */
class HomeActivity : Activity() {

    private lateinit var statusBar: TextView

    private val sink = object : Front.Ui {
        override fun render() {
            val line = Kernel.front.status()
            runOnUiThread { statusBar.text = Home.statusBar(line) }
        }
    }

    /** The destinations. People, Catalog and Settings are later slices;
     *  they lead to an honest placeholder rather than pretending.
     *
     *  **Operator is not mode-keyed yet, and for this slice should not be**
     *  [2026-10-10]: `Robot/light-client-screens.md` has H keyed on an
     *  instance being attached, but enrolling one is how an identity comes
     *  to have an instance at all. Gating the only way in on already being
     *  in would leave no way in. */
    private val destinations = listOfNotNull(
        // **the drawn design, while it is partial** [2026-10-28]: it is
        // a row here rather than the launcher because making it the way
        // in now would take away screens it cannot yet draw. One line
        // moves it when the content views have landed.
        "The shell (in progress)" to { open(ShellActivity::class.java) },
        "Conversations" to { open(ConversationsActivity::class.java) },
        "Meet" to { open(MeetActivity::class.java) },
        "People" to { open(PeopleActivity::class.java) },
        "Catalog" to { open(CatalogActivity::class.java) },
        "Settings" to { placeholder("Settings", "Wake endpoint, backup — later.") },
        "Operator" to { open(OperatorActivity::class.java) },
        // the diagnostics bundle, fieldtest flavour only
        // (`Robot/field-test-diagnostics.md`, section 4)
    )

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // **the connection line is the way to the connection** [2026-10-27]:
        // attaching to a serving node had no screen at all, and the
        // navigation below is the six destinations the screens sheet fixes,
        // so the status this screen already shows is where changing it
        // belongs rather than a seventh row
        statusBar = TextView(this).apply {
            textSize = 12f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.sub)
            setPadding(0, 0, 0, 24)
            text = Home.statusBar("starting the kernel…")
            isClickable = true
            setOnClickListener { open(AttachActivity::class.java) }
        }
        val title = TextView(this).apply {
            textSize = 28f
            typeface = Typeface.DEFAULT_BOLD
            text = "Fueros"
            setPadding(0, 0, 0, 24)
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(Screen.gutter(context), 48, Screen.gutter(context), 48)
            addView(statusBar)
            addView(title)
            destinations.forEach { (label, go) -> addView(navRow(label, go)) }
        }
        Screen.insetting(root)
        setContentView(root)

        // where a provision arrives, and where the kernel is brought up:
        // both are the launcher's, once
        intent.getStringExtra("provision")?.let { Kernel.provision(this, it) }
        // the bench's live stream target, taken in the fieldtest flavour
        // alone (`FuerosApp.streamTo`); the releasable never reads it
        if (BuildConfig.FIELD_TEST) {
            intent.getStringExtra("diag_stream")?.let {
                FuerosApp.streamTo(this, it, intent.getStringExtra("diag_serial"))
            }
        }
        Kernel.start(this)
        benchLevers(intent)
    }

    /**
     * **A second start lands here, not in [onCreate]**: `am start` on a
     * task whose root this activity already is delivers the intent to the
     * running instance (`singleTop`), which is how the bench reached a
     * phone sitting on its Done screen and nothing happened (run
     * `0c8c8f8-a3-node-1`, 2026-10-24).
     */
    override fun onNewIntent(intent: android.content.Intent) {
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
    private fun benchLevers(intent: android.content.Intent) {
        if (!BuildConfig.FIELD_TEST) return
        intent.getStringExtra("forget_contact")?.let { Kernel.forgetContact(it) }
        intent.getStringExtra("forget_captures")?.let { Kernel.forgetCaptures(it) }
        if (intent.getBooleanExtra("contact_peers", false)) Kernel.contactPeersNow()
    }

    override fun onStart() {
        super.onStart()
        Consent.host(this)
        Kernel.bind(sink)
    }

    override fun onStop() {
        Consent.release(this)
        Kernel.unbind(sink)
        super.onStop()
    }

    private fun navRow(label: String, go: () -> Unit): TextView =
        TextView(this).apply {
            text = label
            textSize = 20f
            setPadding(0, 32, 0, 32)
            gravity = Gravity.CENTER_VERTICAL
            isClickable = true
            setOnClickListener { go() }
        }

    private fun open(screen: Class<out Activity>) = startActivity(Intent(this, screen))

    private fun placeholder(section: String, note: String) =
        startActivity(
            Intent(this, PlaceholderActivity::class.java)
                .putExtra("section", section)
                .putExtra("note", note),
        )
}
