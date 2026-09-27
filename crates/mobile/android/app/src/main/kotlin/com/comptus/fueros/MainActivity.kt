package com.comptus.fueros

import android.app.Activity
import android.graphics.Typeface
import android.os.Bundle
import android.view.Gravity
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * The payload screen: the kernel's status, what arrived, and a box to
 * send from.
 *
 * **The screen owns none of it.** A font change recreates this Activity,
 * and a Participant holds a connection, an event loop and a maintenance
 * clock that must survive that; [Kernel] owns them for the life of the
 * process and this binds to it while it is visible. What the screen shows
 * after a recreation is replayed from the kernel, not fetched again.
 *
 * **This device mints its own identity and keeps it.**  The seeds are made
 * in the kernel on first launch and never leave; what travels is the public
 * half, printed for whoever must admit this device.  A provision blob — the
 * `provision` intent extra, or a file of the same name in the kernel's
 * storage — then names the node, where it serves, and the peer to talk to,
 * every field of it public.  Until one arrives the kernel comes up, shows
 * what it presents, and can reach no one.
 *
 * All of this is the ceremony's stand-in.  The ceremony is how two devices
 * are actually introduced; this is a hand-carried substitute for it while
 * the payload path is what is being built.
 */
class MainActivity : Activity() {

    private lateinit var status: TextView
    private lateinit var messages: TextView
    private lateinit var scroll: ScrollView

    /**
     * What the kernel talks to, on the kernel's thread.
     *
     * Held for the life of this Activity so that the unbind in [onStop]
     * names the same sink the bind in [onStart] installed, and the kernel
     * can tell a screen that has gone from the one that replaced it.
     */
    private val sink = object : Front.Ui {
        override fun status(line: String) = runOnUiThread { status.text = line }

        override fun say(line: String) = runOnUiThread { append(line) }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        status = TextView(this).apply {
            textSize = 14f
            typeface = Typeface.MONOSPACE
            text = "starting the kernel…"
        }
        messages = TextView(this).apply { textSize = 16f }
        scroll = ScrollView(this).apply {
            addView(messages)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f
            )
        }
        val box = EditText(this).apply {
            hint = "payload"
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }
        val send = Button(this).apply { text = "Send" }
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(box)
            addView(send)
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            fitsSystemWindows = true
            setPadding(32, 16, 32, 16)
            addView(status)
            addView(scroll)
            addView(row)
        }
        setContentView(root)

        send.setOnClickListener {
            val text = box.text.toString()
            if (text.isEmpty()) return@setOnClickListener
            box.setText("")
            Kernel.send(text)
        }

        intent.getStringExtra("provision")?.let { Kernel.provision(this, it) }
        Kernel.start(this)
    }

    override fun onStart() {
        super.onStart()
        // the kernel replays its transcript on every bind, so the view it
        // replays into starts empty rather than accumulating a second copy
        messages.text = ""
        Kernel.bind(sink)
    }

    override fun onStop() {
        // the kernel keeps running; what stops is anything reaching this
        // screen, so a recreated Activity is never a second listener
        Kernel.unbind(sink)
        super.onStop()
    }

    /** Append a line the kernel said, on the UI thread. */
    private fun append(line: String) {
        messages.append(line + "\n")
        scroll.post { scroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }
}
