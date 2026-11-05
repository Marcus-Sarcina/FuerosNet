package com.comptus.fueros

import android.app.Activity
import android.graphics.Typeface
import android.os.Bundle
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **An invitation to serve from a node, agreed to once**
 * (`rhtn_client::invitation`).
 *
 * [author, 2026-11-04] *"Attachment is not a separate act. The operator
 * should attach upon launch and install, other horizon members should
 * agree to attach once when invited and then all horizon members should
 * re-attach every time they become detached."*
 *
 * The operator's half needs no screen: their client launched the machine
 * and the node speaks as them, so [OperatorActivity] attaches on launch.
 * A member has neither the address nor a reason to trust one, and nodeless
 * no endpoint record has reached them — so their patron sends a link, and
 * this is the once.
 *
 * ## One action, and what it is protecting
 *
 * **There is nothing here to fill in, and the link carries no key.** It
 * carries an address and an *index* — which patron, which subnet, which
 * identity on the device [author, 2026-11-04] — and the key that must
 * answer comes from this device's own archive. So an invitation cannot
 * introduce a patron and cannot point at a party this device has never
 * met: the author's reason for the tap [2026-10-29], *"to prevent stealth
 * insertion of a node the user did not choose"*, is answered by the format
 * and not only by this screen.
 *
 * **Which leaves three honest answers.** One membership, and the only
 * control is agreeing to it. Several that the invitation did not index,
 * and the person picks which — a real choice, not a guess. Or nothing to
 * agree to, because the invitation named a patron this device is not
 * under, an identity that is not the one running here, or there is no
 * patron at all; each says so in words rather than offering a button that
 * would fail.
 *
 * No request goes out to place it: an invitation that had to be checked
 * with somebody would be one whose sender chose who to ask.
 *
 * **Agreeing is remembered.** [Kernel.serveFrom] writes the node into the
 * provision, which is what every later start attaches from, so a device
 * that becomes detached comes back without being asked again.
 */
class InviteActivity : Activity() {

    private companion object {
        /** How long to wait for the kernel a deep link did not start. */
        const val KERNEL_WAIT_MS = 8000
        const val STEP_MS = 250
    }

    private lateinit var narration: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val g = Screen.gutter(this)
        narration = TextView(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.sub)
        }
        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(g, g, g, g)
            addView(narration)
        }
        val root = ScrollView(this).apply { addView(body) }
        Screen.insetting(root)
        setContentView(root)

        // the link as it arrived: a tap on a message, or this screen opened
        // with one in hand by the bench
        val link = intent.data?.toString() ?: intent.getStringExtra("invitation") ?: ""
        if (link.isEmpty()) {
            narration.text = "Nothing was opened here. An invitation begins `fueros://attach`."
            return
        }
        // **a deep link opens this screen cold** [2026-11-04]: nothing but
        // the launcher brings the kernel up, so a tap on an invitation
        // arrived here with no participant and the screen said only "this
        // could not be read" — about a link it had read perfectly well.
        if (!Kernel.hasIdentity(this)) {
            narration.text =
                "This device has no identity yet. Open Fueros and make one, then open this " +
                    "invitation again."
            return
        }
        narration.text = "Reading the invitation…"
        Kernel.start(this)
        // the bring-up runs on its own thread, so this waits for it rather
        // than reading once and calling the absence a bad link
        Thread({
            var read = Kernel.invitation(link)
            var waited = 0
            while (read == null && waited < KERNEL_WAIT_MS) {
                Thread.sleep(STEP_MS.toLong())
                waited += STEP_MS
                read = Kernel.invitation(link)
            }
            val r = read
            runOnUiThread {
                if (r == null) {
                    val why = Kernel.invitationSay(link)
                    narration.text = if (why.isEmpty()) {
                        "The invitation reads correctly, but this device's kernel did not start, " +
                            "so there is nothing to agree with yet."
                    } else {
                        "This could not be read.\n\n$why"
                    }
                } else {
                    render(r, body)
                }
            }
        }, "fueros-invite").start()
    }

    private fun render(read: uniffi.rhtn_ffi.Invited, body: LinearLayout) {
        val said = StringBuilder()
        said.append("An invitation to serve from a node.\n\n")
        read.asLabel?.let {
            said.append("It calls itself \"$it\" — which is a claim, not a fact.\n\n")
        }
        said.append("At  ${read.at}\n\n")

        // **the link named no key and this device placed it** from its own
        // records, so there are only three answers: one membership, a
        // choice between several, or nothing to agree to
        if (read.why.isNotEmpty()) {
            said.append("${read.why}\n")
            narration.text = said.toString()
            return
        }
        said.append(
            "Serving from a node means this device publishes its payload material there, " +
                "collects what is queued for it, and reaches verifiers and a catalog through " +
                "it. Detached, it still meets whoever is in front of it over the bearer " +
                "alone.\n",
        )
        narration.text = said.toString()

        val offer = { node: String, label: String ->
            body.addView(
                Button(this).apply {
                    text = label
                    setOnClickListener {
                        isEnabled = false
                        narration.text = "${narration.text}\nattaching to ${read.at}…"
                        Kernel.serveFrom(this@InviteActivity, node, read.at)
                    }
                },
            )
        }
        val one = read.node
        if (one != null) {
            offer(one, "Serve from ${one.take(8)}…'s node")
            return
        }
        // **several memberships could be meant, so the person picks which**
        // [author, 2026-11-04]: a user belongs to many subnets and a device
        // may hold several, and an invitation that indexed none of them
        // leaves a real choice rather than a guess
        narration.text = "${narration.text}\nWhich of your patrons is this node for?"
        for (k in read.choose) {
            offer(k, "${k.take(8)}…")
        }
        return
    }
}
