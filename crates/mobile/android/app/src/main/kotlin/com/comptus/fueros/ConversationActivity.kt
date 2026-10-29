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
 * A conversation (C2): the thread with one peer, and a box to send from.
 *
 * Each message shows **its real sender** — never a hint styled as an
 * authenticated identity — and an outgoing one shows how far it got:
 * *sending*, *sent* (accepted for carriage, never *delivered* or *read*:
 * the protocol carries no receipt), or *unsent* with the refusal's reason.
 * A submission is never dressed up as a delivery.
 */
class ConversationActivity : Activity() {

    private lateinit var messages: LinearLayout
    private lateinit var scroll: ScrollView
    private var peerKey: String? = null

    private val sink = object : Front.Ui {
        override fun render() = runOnUiThread { redraw() }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // the peer named by the row that opened this, or the one party this
        // device has met or been provisioned to, where a row did not name one
        peerKey = intent.getStringExtra("peer") ?: Kernel.peerKey()

        messages = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        scroll = ScrollView(this).apply {
            addView(messages)
            layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f)
        }
        val box = EditText(this).apply {
            hint = "message"
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }
        val send = Button(this).apply { text = "Send" }
        send.setOnClickListener {
            val text = box.text.toString()
            if (text.isEmpty()) return@setOnClickListener
            // **the party this thread is with, and nobody else**: the
            // kernel used to send to whoever was met last, whichever
            // thread was on screen
            val to = peerKey
            if (to == null) {
                box.setText("")
                return@setOnClickListener
            }
            box.setText("")
            Kernel.send(to, text)
        }
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(box)
            addView(send)
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(Screen.gutter(context), 24, Screen.gutter(context), 16)
            addView(header())
            addView(scroll)
            addView(row)
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

    private fun header(): TextView =
        TextView(this).apply {
            val t = peerKey?.let { Kernel.front.thread(it) }
            text = t?.peerName ?: "conversation"
            textSize = 22f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 0, 0, 16)
        }

    private fun redraw() {
        messages.removeAllViews()
        val t = peerKey?.let { Kernel.front.thread(it) }
        if (t == null) {
            // **a screen with nobody behind it says so**: this opens from
            // the thread list, and a thread exists for every party met,
            // so the empty case is a device that has met nobody
            messages.addView(
                TextView(this).apply {
                    text = "Nobody to talk to yet. Meet someone first, " +
                        "and the conversation opens here."
                    textSize = 16f
                    setPadding(0, 24, 0, 0)
                },
            )
            return
        }
        t.messages.forEach { m -> messages.addView(bubble(m)) }
        scroll.post { scroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }

    private fun bubble(m: Front.Message): TextView =
        TextView(this).apply {
            val body = "${m.who}: ${m.text}"
            text = if (m.mine) "$body   ${mark(m)}" else body
            textSize = 16f
            gravity = if (m.mine) Gravity.END else Gravity.START
            setPadding(0, 10, 0, 10)
        }

    /** The honest tail on an outgoing bubble: what the kernel told us, and
     *  nothing it did not — *sent* is accepted for carriage, never read. */
    private fun mark(m: Front.Message): String = when (m.delivery) {
        Front.Delivery.SENDING -> "sending…"
        Front.Delivery.SENT -> "sent"
        Front.Delivery.UNSENT -> "unsent: ${m.reason ?: "refused"}"
    }
}
