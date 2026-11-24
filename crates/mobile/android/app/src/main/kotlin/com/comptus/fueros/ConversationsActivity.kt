package com.comptus.fueros

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.style.StyleSpan
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import uniffi.rhtn_model.Contact
import uniffi.rhtn_model.How

/**
 * Conversations (C1): the threads this device holds, one card per peer.
 *
 * **Each card is the width of the screen** [author, 2026-11-23]: the
 * person's orb in their colour or their picture, which a press-and-hold
 * opens their options on ([People.menu]); their name in bold, the subnet
 * the two share and where they stand from here in it, in bold where they
 * are in this person's direct line; what arrived unseen; and the last line
 * said, with a **direct-path chip that is status only** — the path order
 * is fixed and nobody chooses it (design §12.6.3). While unprovisioned the
 * list is empty and the kernel's provisioning guidance stands in its place.
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
            typeface = Type.bold
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
        Gateway.bind(sink)
    }

    override fun onStop() {
        Gateway.unbind(sink)
        super.onStop()
    }

    private fun redraw() {
        list.removeAllViews()
        val threads = Gateway.front.threads()
        // **what each identity knows of each party**, read once a drawing
        val known = Gateway.contactsHeld().associateBy { it.by to it.row.keyhash.hex() }
        if (threads.isEmpty()) {
            // the empty state is the provisioning guidance, not a blank
            list.addView(
                TextView(this).apply {
                    text = "No conversations yet."
                    textSize = 18f
                    setPadding(0, 0, 0, 24)
                },
            )
            Gateway.front.notices().forEach { line ->
                list.addView(
                    TextView(this).apply {
                        text = line
                        textSize = 13f
                        typeface = Type.mono
                        setTextColor(Palette.now.sub)
                        setPadding(0, 4, 0, 4)
                    },
                )
            }
            contacts(threads, known)
            return
        }
        // **a blocked party's card goes last**, where it is unblocked from
        val (open, blocked) = threads.partition { !Gateway.front.person(it.by, it.peerKey).blocked }
        (open + blocked).forEach { t ->
            val name = Gateway.aliasOfHeld(t.by, t.peerKey) ?: t.peerName
            list.addView(card(t.by, t.peerKey, name, known[t.by to t.peerKey]?.row, t))
        }
        contacts(threads, known)
    }

    /**
     * **Everyone an identity held here has met, without a thread yet** —
     * each placed as the identity that knows them [author, 2026-11-12:
     * *"it should be placed as the identity which knows that recipient"*],
     * striped in its colour and opened as it. Met in person only: a party
     * handed over as somebody's prior counterparty, a nominee, a verifier,
     * infrastructure or the bench's provisioned witnesses are known of, not
     * known. The device's own identities are not offered to each other.
     */
    private fun contacts(threads: List<Front.Thread>, known: Map<Pair<String, String>, Identities.Owned<Contact>>) {
        val own = Identities.identities(this).map { it.id }.toSet()
        val open = threads.map { it.by to it.peerKey }.toSet()
        val seen = HashSet<Pair<String, String>>()
        val fresh = Gateway.contactsHeld()
            .filter { it.row.how == How.COUNTERPARTY }
            .map { it.by to it.row.keyhash.hex() }
            .filter { it.second !in own && it !in open && seen.add(it) }
        if (fresh.isEmpty()) return
        list.addView(
            TextView(this).apply {
                text = "Contacts"
                textSize = 13f
                setTextColor(Palette.now.sub)
                setPadding(0, 36, 0, 8)
            },
        )
        for ((by, key) in fresh) {
            val name = Gateway.aliasOfHeld(by, key) ?: "${key.take(8)}…"
            list.addView(card(by, key, name, known[by to key]?.row, null))
        }
    }

    /** Which identity holds a thread, said on the row only where the
     *  device holds more than one — a label, not an explanation. */
    private fun holderLabel(by: String): String {
        val held = Identities.identities(this)
        if (held.size <= 1) return ""
        val h = held.firstOrNull { it.id == by } ?: return ""
        return " · " + Identities.label(h)
    }

    private val colours: Map<String, Int> by lazy { Identities.identityColours(this) }

    private fun px(v: Int): Int = (v * resources.displayMetrics.density).toInt()

    /**
     * **The first row after the name** [author, 2026-11-23]: the subnet the
     * two share, in its own name, and where they stand from here in it —
     * in bold where they are in this person's direct line.
     */
    private fun place(by: String, c: Contact?): CharSequence? {
        val subnet = c?.subnet?.hex() ?: return null
        val s = Model.subnets.firstOrNull { it.by == by && it.row.id.hex() == subnet }?.row
        val out = SpannableStringBuilder(s?.alias ?: "subnet ${subnet.take(8)}…")
        Chrome.standing(c.offset)?.let { at ->
            out.append(" ")
            val from = out.length
            out.append(at)
            if (c.inLine) out.setSpan(StyleSpan(Typeface.BOLD), from, out.length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
        }
        return out
    }

    /** One card: a thread's where `t` is one, else a contact's with nothing said yet. */
    private fun card(by: String, peer: String, name: String, contact: Contact?, t: Front.Thread?): View {
        val person = Gateway.front.person(by, peer)
        val title = TextView(this).apply {
            text = name + holderLabel(by)
            textSize = 18f
            typeface = Type.bold
            maxLines = 1
        }
        val head = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(title, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
            Chrome.badge(t?.unread ?: 0)?.let { n ->
                addView(
                    TextView(this@ConversationsActivity).apply {
                        text = n
                        textSize = 11f
                        typeface = Type.bold
                        setTextColor(Palette.RAIL_INK)
                        gravity = Gravity.CENTER
                        // **a muted thread counts on its card alone**, in the
                        // grey of what asks for nothing
                        background = Crowns.disc(if (person.muted) Palette.now.muted else Palette.COUNT)
                        layoutParams = LinearLayout.LayoutParams(px(22), px(22))
                    },
                )
            }
        }
        val text = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(head)
            place(by, contact)?.let { line ->
                addView(
                    TextView(this@ConversationsActivity).apply {
                        this.text = line
                        textSize = 14f
                        setTextColor(Palette.now.muted)
                        maxLines = 1
                    },
                )
            }
            val foot = LinearLayout(this@ConversationsActivity).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
            }
            foot.addView(
                TextView(this@ConversationsActivity).apply {
                    val last = t?.messages?.lastOrNull()
                    this.text = when {
                        person.blocked -> "blocked"
                        t == null -> "nothing said yet"
                        last == null -> "no messages"
                        last.mine -> "me: ${last.text}"
                        else -> last.text
                    } + quiet(person)
                    textSize = 14f
                    setTextColor(Palette.now.muted)
                    maxLines = 1
                },
                LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f),
            )
            if (t != null) {
                foot.addView(
                    TextView(this@ConversationsActivity).apply {
                        this.text = if (t.direct) " · direct" else " · relayed"
                        textSize = 12f
                        setTextColor(Palette.now.muted)
                    },
                )
            }
            addView(foot)
        }
        val orb = People.orb(this, by, peer, name, contact, px(48))
        (orb.layoutParams as LinearLayout.LayoutParams).setMargins(0, 0, px(14), 0)
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(px(12), px(12), px(14), px(12))
            // **the identity this thread is held as, by its colour**
            // [author, 2026-11-12]: the card's edge
            background = GradientDrawable().apply {
                cornerRadius = px(12).toFloat()
                setColor(Palette.now.sunken)
                setStroke(px(2), Identities.colourOf(colours, by))
            }
            alpha = if (person.blocked) 0.6f else 1f
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { setMargins(0, 0, 0, px(10)) }
            isClickable = true
            addView(orb)
            addView(text, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
            setOnClickListener {
                startActivity(
                    Intent(this@ConversationsActivity, ConversationActivity::class.java)
                        .putExtra("peer", peer)
                        .putExtra("by", by),
                )
            }
        }
    }

    /** What was muted about them, said after the last line. */
    private fun quiet(p: Front.Person): String = when {
        p.blocked -> ""
        p.muted -> " · muted"
        p.silent -> " · notifications muted"
        else -> ""
    }
}
