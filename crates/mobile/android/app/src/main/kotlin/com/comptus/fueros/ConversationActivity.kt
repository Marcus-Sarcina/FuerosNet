package com.comptus.fueros

import android.app.Activity
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.view.Gravity
import android.view.View
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
 *
 * **As a chat is laid out** [author, 2026-11-23]: the box at the foot and
 * the conversation stacked up from it, both lifted above the keyboard
 * while it is up ([Screen.insetting]); this person's bubbles to the right,
 * grey and outlined in the colour of the identity they speak as; the other
 * party's to the left, on a low-saturation version of that party's colour
 * ([Chrome.BUBBLE_SHARE]).
 */
class ConversationActivity : Activity() {

    private lateinit var messages: LinearLayout
    private lateinit var scroll: ScrollView
    /** The box and its button: not offered on a thread whose sender is
     *  unverified, since a reply goes only to a contact. */
    private lateinit var compose: LinearLayout
    /** Who this is with, redrawn with the thread: their colour may change under it. */
    private lateinit var heading: LinearLayout
    private var peerKey: String? = null

    /** The identity this conversation belongs to: the one a thread was
     *  opened under, or the one in focus for a thread started here — a
     *  new thread is the focused identity's, as a new tab is the focused
     *  window's [author, 2026-11-11]. */
    private var by: String? = null

    private val sink = object : Front.Ui {
        override fun render() = runOnUiThread { redraw() }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // the peer named by the row that opened this, or the one party this
        // device has met or been provisioned to, where a row did not name one
        // **a conversation is one identity's with one party, named by the
        // row that opened it** [review, 2026-11-13]: opened without either
        // is a defect, not a default
        peerKey = intent.getStringExtra("peer")
        by = intent.getStringExtra("by")
        if (peerKey == null || by == null) {
            Diag.warn("conversation.opened", "why" to "no row named the identity and the party")
            finish()
            return
        }

        // **stacked up from the box**: a short conversation sits at the
        // foot, as a long one ends there
        heading = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        messages = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.BOTTOM
        }
        scroll = ScrollView(this).apply {
            isFillViewport = true
            addView(messages, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.MATCH_PARENT))
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
            val holder = by ?: return@setOnClickListener
            Attachments.send(holder, to, text)
        }
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(box)
            addView(send)
        }
        compose = row
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(Screen.gutter(context), 24, Screen.gutter(context), 16)
            addView(heading)
            addView(scroll)
            addView(row)
        }
        Screen.insetting(root)
        setContentView(root)
    }

    override fun onStart() {
        super.onStart()
        Gateway.bind(sink)
        // the thread is read when it is opened, its newest page first
        val (b, p) = Pair(by, peerKey)
        if (b != null && p != null) Thread { Gateway.front.open(b, p) }.start()
    }

    override fun onStop() {
        Gateway.unbind(sink)
        super.onStop()
    }

    // **on screen, what arrives is seen** as it comes, and what had
    // arrived is seen now
    override fun onResume() {
        super.onResume()
        val (b, p) = Pair(by, peerKey)
        if (b != null && p != null) Gateway.front.showing(b, p)
    }

    override fun onPause() {
        val (b, p) = Pair(by, peerKey)
        if (b != null && p != null) Gateway.front.hidden(b, p)
        super.onPause()
    }

    /** Who this is with: their orb, whose press-and-hold opens their options, and their name. */
    private fun header(): View {
        val (b, p) = Pair(by!!, peerKey!!)
        val t = Gateway.front.thread(b, p)
        val name = Gateway.aliasOfHeld(b, p) ?: t?.peerName ?: "${p.take(8)}…"
        val contact = Gateway.contactsHeld().firstOrNull { it.by == b && it.row.keyhash.hex() == p }?.row
        val orb = People.orb(this, b, p, name, contact, px(40))
        (orb.layoutParams as LinearLayout.LayoutParams).setMargins(0, 0, px(12), 0)
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, 0, px(12))
            addView(orb)
            addView(
                TextView(this@ConversationActivity).apply {
                    text = name
                    textSize = 22f
                    typeface = Type.bold
                    maxLines = 1
                },
            )
        }
    }

    private fun px(v: Int): Int = (v * resources.displayMetrics.density).toInt()

    private fun redraw() {
        heading.removeAllViews()
        heading.addView(header())
        messages.removeAllViews()
        val t = by?.let { b -> peerKey?.let { Gateway.front.thread(b, it) } }
        // **not answered**: an unattributed sender, or a party blocked
        val blocked = by?.let { b -> peerKey?.let { Gateway.front.person(b, it).blocked } } == true
        compose.visibility = if (t?.unverified == true || blocked) View.GONE else View.VISIBLE
        if (blocked) {
            messages.addView(
                TextView(this).apply {
                    text = "Blocked: what they send is dropped as it comes. Press and hold their orb to unblock."
                    textSize = 14f
                    setTextColor(Palette.now.sub)
                    setPadding(0, 0, 0, px(12))
                },
            )
        }
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
        if (t.more) {
            messages.addView(
                Button(this).apply {
                    text = "earlier messages"
                    setOnClickListener {
                        isEnabled = false
                        Thread { Gateway.front.earlier(t.by, t.peerKey) }.start()
                    }
                },
            )
        }
        t.messages.forEach { m -> messages.addView(bubble(m)) }
        scroll.post { scroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }

    /** One message: this person's to the right, the other party's to the left. */
    private fun bubble(m: Front.Message): View {
        val (b, p) = Pair(by!!, peerKey!!)
        val c = Palette.now
        val inner = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(px(12), px(8), px(12), px(8))
            background = GradientDrawable().apply {
                cornerRadius = px(14).toFloat()
                if (m.mine) {
                    setColor(c.control)
                    setStroke(px(2), Identities.colourOf(colours, b))
                } else {
                    setColor(Palette.mix(People.colour(b, p), c.ground, Chrome.BUBBLE_SHARE))
                }
            }
            addView(
                TextView(this@ConversationActivity).apply {
                    text = m.text
                    textSize = 16f
                    setTextColor(c.ink)
                },
            )
            if (m.mine) {
                addView(
                    TextView(this@ConversationActivity).apply {
                        text = mark(m)
                        textSize = 12f
                        setTextColor(c.muted)
                    },
                )
            }
        }
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = if (m.mine) Gravity.END else Gravity.START
            setPadding(if (m.mine) px(48) else 0, px(4), if (m.mine) 0 else px(48), px(4))
            addView(inner, LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT))
        }
    }

    private val colours: Map<String, Int> by lazy { Identities.identityColours(this) }

    /** The honest tail on an outgoing bubble: what the kernel told us, and
     *  nothing it did not — *sent* is accepted for carriage, never read. */
    private fun mark(m: Front.Message): String = when (m.delivery) {
        Front.Delivery.SENDING -> "sending…"
        Front.Delivery.SENT -> "sent"
        Front.Delivery.UNSENT -> "unsent: ${m.reason ?: "refused"}"
    }
}
