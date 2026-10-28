package com.comptus.fueros

import android.app.Activity
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **Authority: the lines this key holds, and what ending one costs**
 * (the screens sheet's E3; `light-client-requirements.md` §6, PRD-02,
 * UX-006).
 *
 * The gap this closes: §6 wants the person warned, before a change that
 * ends an authority relationship, which of the resources they use it
 * takes away. The computation landed on 2026-10-28 and had no screen, so
 * the only way to read it was a test — and `depart` was reachable from
 * nothing at all, which made the warning moot in the other direction.
 *
 * **The cost is read before the act, and the act is not gated on it.**
 * Leaving is a right rather than a request (design §6.2): the old patron
 * does not sign and cannot refuse. So this screen informs and does not
 * guard. It asks twice because a departure cannot be taken back, not
 * because anybody must approve it.
 *
 * **An empty list is not the same as a free departure**, which is the one
 * thing this screen must not blur. A key whose own adoption has not
 * propagated back to it holds no edge to subtract, and the kernel says so
 * rather than answering zero; [Leaving] leads with that sentence instead
 * of *nothing goes*, and prints the limits whether or not anything is
 * lost.
 *
 * Reached from People, where the rest of the neighbourhood is shown.
 */
class LeavingActivity : Activity() {

    private lateinit var roll: TextView
    private lateinit var warning: TextView
    private lateinit var said: TextView
    private lateinit var lines: LinearLayout
    private lateinit var leave: Button

    /** The patron whose cost is on screen, and whether the button is armed. */
    private var chosen: String? = null
    private var armed = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        roll = mono("…")
        warning = body("")
        said = mono("")
        lines = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        leave = Button(this).apply {
            text = "Leave"
            visibility = android.view.View.GONE
            setOnClickListener { pressed() }
        }

        val content = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(Screen.GUTTER, 32, Screen.GUTTER, 48)
            addView(title("Authority"))
            addView(note(WHY))
            addView(heading("Who you are under"))
            addView(roll)
            addView(lines)
            addView(heading("What leaving would cost"))
            addView(warning)
            addView(leave)
            addView(said)
        }
        val frame = ScrollView(this).apply { addView(content) }
        Screen.insetting(frame)
        setContentView(frame)
    }

    override fun onStart() {
        super.onStart()
        draw()
    }

    /**
     * **Off the drawing thread**: reading the relationships is a blocking
     * call into the client, which may be parked in `nextEvent` for its
     * poll interval.
     */
    private fun draw() {
        warning.text = "Choose a line above."
        leave.visibility = android.view.View.GONE
        chosen = null
        armed = false
        Thread({
            val held = Kernel.lines()
            runOnUiThread {
                if (isFinishing) return@runOnUiThread
                roll.text = if (held.isEmpty()) Leaving.NONE else ""
                lines.removeAllViews()
                for (l in held) lines.addView(row(l))
            }
        }, "fueros-lines").start()
    }

    /** One line, tappable: tapping it asks what leaving it would cost. */
    private fun row(l: Kernel.Line): TextView = mono(Leaving.line(l)).apply {
        setPadding(0, 20, 0, 20)
        isClickable = true
        setOnClickListener { cost(l.patron) }
    }

    private fun cost(patron: String) {
        chosen = patron
        armed = false
        warning.text = "working it out…"
        leave.visibility = android.view.View.GONE
        said.text = ""
        Thread({
            val (c, why) = Kernel.costOfLeaving(patron)
            runOnUiThread {
                if (isFinishing || chosen != patron) return@runOnUiThread
                if (c == null) {
                    // **the kernel's own reason, not a guess at it.** The
                    // refusal for a patron this key holds no relationship
                    // with reads differently from a kernel that has not
                    // come up, and the screen is not the place to decide
                    // which happened
                    warning.text = "No cost for ${patron.take(Leaving.SHORT)}…: $why"
                    leave.visibility = android.view.View.GONE
                    return@runOnUiThread
                }
                warning.text = Leaving.words(c)
                leave.text = Leaving.button(patron, false)
                leave.visibility = android.view.View.VISIBLE
            }
        }, "fueros-cost").start()
    }

    /**
     * The first press arms, the second leaves. Nothing is re-read between
     * them: the cost on screen is the cost the person decided on, and
     * re-computing under their thumb would show them one figure and act
     * on another.
     */
    private fun pressed() {
        val patron = chosen ?: return
        if (!armed) {
            armed = true
            leave.text = Leaving.button(patron, true)
            said.text = "This cannot be taken back."
            return
        }
        leave.visibility = android.view.View.GONE
        said.text = "leaving…"
        Thread({
            val out = Kernel.departFrom(patron)
            runOnUiThread {
                if (isFinishing) return@runOnUiThread
                said.text = out
                draw()
            }
        }, "fueros-depart").start()
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

    private fun body(t: String) = TextView(this).apply {
        text = t
        textSize = 15f
        setPadding(0, 4, 0, 12)
    }

    private fun mono(t: String) = TextView(this).apply {
        text = t
        textSize = 13f
        typeface = Typeface.MONOSPACE
        setPadding(0, 4, 0, 4)
    }

    private companion object {
        /** Why a person is on this screen at all, in its own words. */
        const val WHY =
            "A patron is who placed you, and leaving one is yours alone to do: they " +
                "do not sign it and cannot refuse it. What it changes is what you can " +
                "reach, so that is worked out first."
    }
}
