package com.comptus.fueros

import android.app.Activity
import android.content.Intent
import android.graphics.Typeface
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import uniffi.rhtn_model.How

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
            setPadding(Screen.gutter(context), 48, Screen.gutter(context), 48)
            addView(title)
            addView(scroll)
        }
        Screen.insetting(root)
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
                        setTextColor(Palette.now.sub)
                        setPadding(0, 4, 0, 4)
                    },
                )
            }
            contacts(threads)
            return
        }
        threads.forEach { t -> list.addView(row(t)) }
        contacts(threads)
    }

    /**
     * **The focused identity's contacts without a thread of its own** —
     * starting a thread is the focused identity's act [author, 2026-11-11],
     * and until it has one the list is the only door to a conversation. A
     * contact met by another identity held here is not offered: the thread
     * would be that identity's. Infrastructure is reached, not written to.
     */
    private fun contacts(threads: List<Front.Thread>) {
        val active = Kernel.activeIdentity() ?: return
        val open = threads.filter { it.by == active }.map { it.peerKey }.toSet()
        val fresh = Kernel.contacts()
            .filter { it.how != How.INFRA }
            .map { it.keyhash.hex() }
            .filter { it !in open }
        if (fresh.isEmpty()) return
        list.addView(
            TextView(this).apply {
                text = "Contacts"
                textSize = 13f
                setTextColor(Palette.now.sub)
                setPadding(0, 36, 0, 8)
            },
        )
        for (key in fresh) {
            val name = TextView(this).apply {
                text = Kernel.aliasOf(key) ?: "${key.take(8)}…"
                textSize = 20f
            }
            list.addView(
                LinearLayout(this).apply {
                    orientation = LinearLayout.HORIZONTAL
                    gravity = Gravity.CENTER_VERTICAL
                    setPadding(0, 20, 0, 20)
                    isClickable = true
                    addView(
                        View(this@ConversationsActivity).apply {
                            setBackgroundColor(Identities.colourOf(colours, active))
                            layoutParams = LinearLayout.LayoutParams(px(6), px(26)).apply { setMargins(0, 0, px(10), 0) }
                        },
                    )
                    addView(name, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
                    setOnClickListener {
                        startActivity(
                            Intent(this@ConversationsActivity, ConversationActivity::class.java)
                                .putExtra("peer", key)
                                .putExtra("by", active),
                        )
                    }
                },
            )
        }
    }

    /** Which identity holds a thread, said on the row only where the
     *  device holds more than one — a label, not an explanation. */
    private fun holderLabel(by: String): String {
        val held = Kernel.identities(this)
        if (held.size <= 1) return ""
        val h = held.firstOrNull { it.id == by } ?: return ""
        return " · " + Identities.label(h)
    }

    private val colours: Map<String, Int> by lazy { Kernel.identityColours(this) }

    private fun px(v: Int): Int = (v * resources.displayMetrics.density).toInt()

    private fun row(t: Front.Thread): View {
        val name = TextView(this).apply {
            // the alias they published where one has reached this device,
            // else what the thread was opened as
            text = (Kernel.aliasOf(t.peerKey) ?: t.peerName) + holderLabel(t.by)
            textSize = 20f
        }
        val chip = TextView(this).apply {
            text = if (t.direct) "· direct" else "· relayed"
            textSize = 12f
            setTextColor(Palette.now.sub)
        }
        val head = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            // the identity this thread is held as, by its colour
            // [author, 2026-11-12]
            addView(
                View(this@ConversationsActivity).apply {
                    setBackgroundColor(Identities.colourOf(colours, t.by))
                    layoutParams = LinearLayout.LayoutParams(px(6), px(26)).apply { setMargins(0, 0, px(10), 0) }
                },
            )
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
            setTextColor(Palette.now.sub)
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
                        .putExtra("peer", t.peerKey)
                        .putExtra("by", t.by),
                )
            }
        }
    }
}
