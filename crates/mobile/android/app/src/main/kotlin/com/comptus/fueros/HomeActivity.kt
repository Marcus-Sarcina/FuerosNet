package com.comptus.fueros

import android.app.Activity
import android.content.Intent
import android.graphics.Color
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
            runOnUiThread { statusBar.text = line.substringBefore('\n') }
        }
    }

    /** The destinations. Meet, People, Catalog and Settings are later
     *  slices; they lead to an honest placeholder rather than pretending.
     *  Operator is absent by design until the kernel can say an identity
     *  runs an instance (the mode query is owed). */
    private val destinations = listOfNotNull(
        "Conversations" to { open(ConversationsActivity::class.java) },
        "Meet" to { open(MeetActivity::class.java) },
        "People" to { placeholder("People", "Your horizon — later.") },
        "Catalog" to { placeholder("Catalog", "Resources your node serves — later.") },
        "Settings" to { placeholder("Settings", "Wake endpoint, backup — later.") },
        // the diagnostics bundle, fieldtest flavour only
        // (`Robot/field-test-diagnostics.md`, section 4)
        if (BuildConfig.FIELD_TEST) "Send diagnostics (field test)" to { Report.send(this) } else null,
    )

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        statusBar = TextView(this).apply {
            textSize = 12f
            typeface = Typeface.MONOSPACE
            setTextColor(Color.GRAY)
            setPadding(0, 0, 0, 24)
            text = "starting the kernel…"
        }
        val title = TextView(this).apply {
            textSize = 28f
            typeface = Typeface.DEFAULT_BOLD
            text = "Fueros"
            setPadding(0, 0, 0, 24)
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            fitsSystemWindows = true
            setPadding(48, 48, 48, 48)
            addView(statusBar)
            addView(title)
            destinations.forEach { (label, go) -> addView(navRow(label, go)) }
        }
        setContentView(root)

        // where a provision arrives, and where the kernel is brought up:
        // both are the launcher's, once
        intent.getStringExtra("provision")?.let { Kernel.provision(this, it) }
        Kernel.start(this)
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
