package com.comptus.fueros

import android.app.Activity
import android.graphics.Typeface
import android.os.Bundle
import android.view.Gravity
import android.widget.Button
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
 * Reached from Options, by the row that reads the status this screen
 * changes. Owned by nothing: the kernel holds the
 * session for the life of the process and this binds to its front while
 * visible.
 */
class AttachActivity : Activity() {

    private lateinit var statusLine: TextView
    private lateinit var known: TextView
    private lateinit var said: TextView
    private lateinit var mine: LinearLayout

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
        mine = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        drawMine()

        val detach = Button(this).apply {
            text = "Detach"
            setOnClickListener { Kernel.detachNow() }
        }
        val buttons = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(detach)
        }

        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(Screen.gutter(context), 32, Screen.gutter(context), 48)
            addView(title("Serving node"))
            addView(statusLine)
            addView(note(WHY))
            addView(heading("Nodes this device knows of"))
            addView(mine)
            addView(known)
            addView(buttons)
            addView(said)
        }
        val frame = ScrollView(this).apply { addView(body) }
        Screen.insetting(frame)
        setContentView(frame)
    }

    override fun onStart() {
        super.onStart()
        Kernel.bind(sink)
        // **off the drawing thread, and once per visit**: reading the
        // horizon is a blocking call into the client, which may be parked
        // in `nextEvent` for its poll interval, and this screen must not
        // wait on that where the person is looking
        Thread({
            val roll = Attach.roll(Kernel.instances())
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
        setTextColor(Palette.now.sub)
        setPadding(0, 8, 0, 8)
    }

    private fun mono(t: String) = TextView(this).apply {
        text = t
        textSize = 13f
        typeface = Typeface.MONOSPACE
        setPadding(0, 4, 0, 4)
    }

    /**
     * **The nodes this device administers, offered without any typing**
     * [author, 2026-11-04: "GXV needs the node key and IP to attach. This
     * is supposed to be automatic."].
     *
     * Both halves are already held. The **address** is the instance this
     * device launched, remembered by [OperatorActivity]; the surface is
     * kept there, and the node answers the protocol on
     * [Launch.QUIC_PORT] at the same host. The **key** is this device's
     * own keyhash, because an operator's node speaks as its operator —
     * which is the author's own rule, and why the attach form was asking
     * for something it could have answered itself.
     *
     * **One instance fills the fields outright**; several draw a row each,
     * because choosing between them is a real choice. **The tap on Attach
     * stays either way** [author, 2026-10-29: the tap "is to prevent
     * stealth insertion of a node the user did not choose"] — nothing here
     * attaches, it only stops a person transcribing hex.
     */
    private fun drawMine() {
        mine.removeAllViews()
        val me = Kernel.keyhash()
        val rows = ownInstances()
        if (me == null || rows.isEmpty()) {
            return
        }
        mine.addView(
            note(
                "Serving from a node is one action, not a form: these are nodes this device " +
                    "already knows how to reach, and agreeing once is the whole of it — it is " +
                    "remembered, and this device comes back to it whenever it is detached.",
            ),
        )
        for ((label, addr) in rows) {
            mine.addView(
                Button(this).apply {
                    text = "Serve from $label"
                    setOnClickListener {
                        said.text = "attaching to $addr…"
                        Kernel.serveFrom(this@AttachActivity, me, addr)
                    }
                },
            )
        }
    }

    /**
     * This operator's instances as `label to host:port`, the port being the
     * one a node serves the protocol on rather than the administration
     * surface this device talks to it over.
     */
    private fun ownInstances(): List<Pair<String, String>> =
        getSharedPreferences("operator", MODE_PRIVATE)
            .getString("instances", "")!!
            .lines()
            .filter { it.isNotBlank() }
            .mapNotNull { line ->
                val parts = line.split('\t')
                val at = parts.getOrNull(0)?.trim().orEmpty()
                val host = at.substringBeforeLast(':', "").ifEmpty { return@mapNotNull null }
                val label = parts.getOrElse(2) { at }.ifEmpty { at }
                label to "$host:${Launch.QUIC_PORT}"
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
