package com.comptus.fueros

import android.app.Activity
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * Conversations (C1): the threads this device holds, one row per peer.
 *
 * Each row is the peer's name, a **direct-path chip that is status only** —
 * the path order is fixed and nobody chooses it (design §12.6.3) — and the
 * last line said. While unprovisioned the list is empty and the kernel's
 * provisioning guidance stands in its place.
 */
class ConversationsActivity : Activity() {

    private lateinit var list: LinearLayout

    private val sink = object : Front.Ui {
        override fun render() = runOnUiThread { redraw() }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        list = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        val scroll = ScrollView(this).apply {
            addView(list)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.MATCH_PARENT,
            )
        }
        val title = TextView(this).apply {
            text = "Conversations"
            textSize = 24f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 0, 0, 24)
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            fitsSystemWindows = true
            setPadding(48, 48, 48, 48)
            addView(title)
            addView(scroll)
        }
        setContentView(root)
    }

    override fun onStart() {
        super.onStart()
        Kernel.bind(sink)
    }

    override fun onStop() {
        Kernel.unbind(sink)
        super.onStop()
    }

    private fun redraw() {
        list.removeAllViews()
        val threads = Kernel.front.threads()
        if (threads.isEmpty()) {
            // the empty state is the provisioning guidance, not a blank
            list.addView(
                TextView(this).apply {
                    text = "No conversations yet."
                    textSize = 18f
                    setPadding(0, 0, 0, 24)
                },
            )
            Kernel.front.notices().forEach { line ->
                list.addView(
                    TextView(this).apply {
                        text = line
                        textSize = 13f
                        typeface = Typeface.MONOSPACE
                        setTextColor(Color.GRAY)
                        setPadding(0, 4, 0, 4)
                    },
                )
            }
            return
        }
        threads.forEach { t -> list.addView(row(t)) }
    }

    private fun row(t: Front.Thread): View {
        val name = TextView(this).apply {
            text = t.peerName
            textSize = 20f
        }
        val chip = TextView(this).apply {
            text = if (t.direct) "· direct" else "· relayed"
            textSize = 12f
            setTextColor(Color.GRAY)
        }
        val head = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(name, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
            addView(chip)
        }
        val last = t.messages.lastOrNull()
        val preview = TextView(this).apply {
            text = when {
                last == null -> "no messages"
                last.mine -> "me: ${last.text}"
                else -> "${last.who}: ${last.text}"
            }
            textSize = 14f
            setTextColor(Color.GRAY)
            maxLines = 1
        }
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, 28, 0, 28)
            isClickable = true
            addView(head)
            addView(preview)
            setOnClickListener {
                startActivity(
                    Intent(this@ConversationsActivity, ConversationActivity::class.java)
                        .putExtra("peer", t.peerKey),
                )
            }
        }
    }
}
