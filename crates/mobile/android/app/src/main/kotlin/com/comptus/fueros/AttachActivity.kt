package com.comptus.fueros

import android.app.Activity
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.view.Gravity
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **Attach this device to a serving node** (the screens sheet's B, the
 * capability matrix's *attach to the serving node*; `light-client-
 * requirements.md` §4).
 *
 * The gap this closes: the only attach was the one the kernel performs at
 * a cold start from the bench's provision blob, so the installed app could
 * meet whoever was in front of it and could never reach a node — no
 * verifier beyond the people present, no mailbox, no catalog, no sweep.
 * Every run that needed a node was a run a script had set up [2026-10-27].
 *
 * **A node is a key and an address, and the screen says why both.** The
 * address is where to knock; the keyhash is who must answer, which is what
 * makes a wrong address a failed attach rather than an attach to somebody
 * else. Where a node's own record has already reached this device its
 * details are listed to be read off rather than typed.
 *
 * Reached by tapping the connection line on Home, where the status this
 * screen changes already lives. Owned by nothing: the kernel holds the
 * session for the life of the process and this binds to its front while
 * visible.
 */
class AttachActivity : Activity() {

    private lateinit var statusLine: TextView
    private lateinit var known: TextView
    private lateinit var said: TextView
    private lateinit var nodeField: EditText
    private lateinit var addressField: EditText

    private val sink = object : Front.Ui {
        override fun render() = runOnUiThread {
            statusLine.text = Kernel.front.status()
            said.text = Kernel.front.notices().takeLast(NOTICE_LINES).joinToString("\n")
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        statusLine = mono("…")
        known = mono("")
        said = mono("")
        nodeField = field("the node's key, 64 hex characters")
        addressField = field("where it answers, like 192.168.0.8:7447")

        val attach = Button(this).apply {
            text = "Attach"
            setOnClickListener {
                val n = nodeField.text.toString()
                val a = addressField.text.toString()
                val why = Attach.refusal(n, a)
                if (why != null) {
                    said.text = why
                    return@setOnClickListener
                }
                said.text = "attaching…"
                Kernel.attachTo(n, a)
            }
        }
        val detach = Button(this).apply {
            text = "Detach"
            setOnClickListener { Kernel.detachNow() }
        }
        val buttons = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(attach)
            addView(detach)
        }

        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 32, 48, 48)
            addView(title("Serving node"))
            addView(statusLine)
            addView(note(WHY))
            addView(heading("Nodes this device knows of"))
            addView(known)
            addView(heading("Or name one"))
            addView(nodeField)
            addView(addressField)
            addView(buttons)
            addView(said)
        }
        setContentView(ScrollView(this).apply { addView(body); fitsSystemWindows = true })
    }

    override fun onStart() {
        super.onStart()
        Kernel.bind(sink)
        // **off the drawing thread, and once per visit**: reading the
        // horizon is a blocking call into the client, which may be parked
        // in `nextEvent` for its poll interval, and this screen must not
        // wait on that where the person is looking
        Thread({
            val roll = Attach.roll(Kernel.nodesKnown())
            runOnUiThread { known.text = roll }
        }, "fueros-nodes").start()
    }

    override fun onStop() {
        Kernel.unbind(sink)
        super.onStop()
    }

    private fun title(t: String) = TextView(this).apply {
        text = t
        textSize = 26f
        typeface = Typeface.DEFAULT_BOLD
        setPadding(0, 0, 0, 16)
    }

    private fun heading(t: String) = TextView(this).apply {
        text = t
        textSize = 16f
        typeface = Typeface.DEFAULT_BOLD
        setPadding(0, 32, 0, 8)
    }

    private fun note(t: String) = TextView(this).apply {
        text = t
        textSize = 14f
        setTextColor(Color.GRAY)
        setPadding(0, 8, 0, 8)
    }

    private fun mono(t: String) = TextView(this).apply {
        text = t
        textSize = 13f
        typeface = Typeface.MONOSPACE
        setPadding(0, 4, 0, 4)
    }

    private fun field(hintText: String) = EditText(this).apply {
        hint = hintText
        textSize = 15f
        setSingleLine()
    }

    private companion object {
        /** Why a person would attach at all, in the screen's own words. */
        const val WHY =
            "Attached, this device publishes its payload material, collects what is " +
                "queued for it, and can reach verifiers and a catalog. Detached, it " +
                "still meets whoever is in front of it over the bearer alone."

        /** How many of the kernel's notice lines this screen shows. */
        const val NOTICE_LINES = 12
    }
}
