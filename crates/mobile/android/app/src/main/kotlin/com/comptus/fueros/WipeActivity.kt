package com.comptus.fueros

import android.app.Activity
import android.content.Intent
import android.graphics.Typeface
import android.os.Bundle
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **Taking the identity off this device** ([Wipe]; PRD-03): the most
 * destructive act in the app, built as a door of its own [author,
 * 2026-11-07] rather than fused with starting a new identity.
 *
 * **Two steps, and the second one names the act.** The first screen says
 * what goes, what cannot come back, and what a new identity afterwards
 * is; the button under it is not *continue* but [Wipe.confirm]'s own
 * sentence, so nobody confirms a question they were not asked.
 *
 * **Then the process dies.** A live kernel writes through the shell's
 * custody, so the only guarantee that nothing in flight puts a file back
 * is to stop running: this clears, launches [FirstRunActivity] in a task
 * of its own, and exits. The bench already treats a restart as ordinary.
 */
class WipeActivity : Activity() {

    private lateinit var body: LinearLayout

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val g = Screen.gutter(this)
        body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(g, g, g, g)
        }
        val root = ScrollView(this).apply { addView(body) }
        Screen.insetting(root)
        setContentView(root)
        ask()
    }

    /** What it takes, before anything is confirmed. */
    private fun ask() {
        body.removeAllViews()
        body.addView(
            TextView(this).apply {
                textSize = 18f
                setTypeface(typeface, Typeface.BOLD)
                setTextColor(Palette.now.ink)
                text = "Wipe this device"
            },
        )
        for (said in Wipe.words()) {
            body.addView(
                TextView(this).apply {
                    textSize = 14f
                    setTextColor(Palette.now.ink)
                    text = said
                },
            )
        }
        body.addView(
            Button(this).apply {
                text = Wipe.confirm()
                setOnClickListener { go() }
            },
        )
        body.addView(
            Button(this).apply {
                text = "Keep it"
                setOnClickListener { finish() }
            },
        )
    }

    /** Clear, say what happened, and stop running. */
    private fun go() {
        val left = Kernel.wipe(this)
        body.removeAllViews()
        body.addView(
            TextView(this).apply {
                textSize = 14f
                setTextColor(Palette.now.ink)
                text = Wipe.left(left) ?: Wipe.done()
            },
        )
        body.addView(
            Button(this).apply {
                text = "Close"
                setOnClickListener { restart() }
            },
        )
    }

    private fun restart() {
        startActivity(
            Intent(this, FirstRunActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
            },
        )
        finishAffinity()
        // **the one guarantee**: no thread of the old identity's kernel
        // outlives the wipe, whatever it was holding when it was dropped
        Runtime.getRuntime().exit(0)
    }
}
