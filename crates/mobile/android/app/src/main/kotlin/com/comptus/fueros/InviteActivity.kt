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
 * **Agreeing is remembered.** [Gateway.serveFrom] writes the node into the
 * provision, which is what every later start attaches from, so a device
 * that becomes detached comes back without being asked again.
 */
class InviteActivity : Activity() {
    /** The identity this screen acts for: the one named by whoever opened it, else the one in focus — a link tapped, or a screen nothing opens yet. */
    private val by: String by lazy { intent.getStringExtra("by") ?: Identities.focused() }

    private companion object {
        /** How long to wait for the kernel a deep link did not start. */
        const val KERNEL_WAIT_MS = 8000
        const val STEP_MS = 250
    }

    private lateinit var narration: TextView
    private lateinit var root: ScrollView

    /**
     * **The page, set only where there is one to show** [author,
     * 2026-11-17]: the window is clear until then, and an invitation that
     * is a move never sets it.
     */
    private fun show() {
        if (root.parent != null) return
        root.setBackgroundColor(Palette.now.ground)
        setContentView(root)
    }

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
        root = ScrollView(this).apply { addView(body) }
        Screen.insetting(root)

        // the link as it arrived: a tap on a message, or this screen opened
        // with one in hand by the bench
        val link = intent.data?.toString() ?: intent.getStringExtra("invitation") ?: ""
        if (link.isEmpty()) {
            show()
            narration.text = "Nothing was opened here. An invitation begins `fueros://attach`."
            return
        }
        // **a deep link opens this screen cold** [2026-11-04]: nothing but
        // the launcher brings the kernel up, so a tap on an invitation
        // arrived here with no participant and the screen said only "this
        // could not be read" — about a link it had read perfectly well.
        if (!Identities.hasIdentity(this)) {
            show()
            narration.text =
                "This device has no identity yet. Open Fueros and make one, then open this " +
                    "invitation again."
            return
        }
        Attachments.start(this)
        // the bring-up runs on its own thread, so this waits for it rather
        // than reading once and calling the absence a bad link
        Thread({
            var read = Gateway.invitation(by, link)
            var waited = 0
            while (read == null && waited < KERNEL_WAIT_MS) {
                Thread.sleep(STEP_MS.toLong())
                waited += STEP_MS
                read = Gateway.invitation(by, link)
            }
            val r = read
            runOnUiThread {
                if (r == null) {
                    show()
                    val why = Gateway.invitationSay(link)
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
        // **a node this identity already serves from is a move, not a
        // first invitation** [author, 2026-11-16], and it has no page
        // [author, 2026-11-17]: it is proved behind, and the subnet's orb
        // carries the question once it has
        val one = read.node
        if (read.why.isEmpty() && one != null && agreed(one)) {
            relocate(one, read.at, read.asLabel, read.subnet)
            return
        }
        show()
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
                        if (agreed(node)) {
                            relocate(node, read.at, read.asLabel, read.subnet)
                        } else {
                            narration.text = "${narration.text}\nattaching to ${read.at}…"
                            Attachments.serveFrom(by, this@InviteActivity, node, read.at)
                        }
                    }
                },
            )
        }
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

    /** Whether this identity already agreed to serve from `nodeHex`. */
    private fun agreed(nodeHex: String): Boolean =
        Gateway.loaded[by]?.let { p ->
            runCatching { p.agreed().any { Gateway.hex(it.serving) == nodeHex } }.getOrDefault(false)
        } ?: false

    /**
     * **Held as a relocation and proved at once**: the kernel dials the
     * new address and binds what answers there to the patron before the
     * person is asked anything; the subnet's orb carries the question.
     * **Nothing is shown** [author, 2026-11-17]: *"the invite should
     * manifest in the UI only in the notification counter on the subnet
     * shield"*. This screen goes, the proof carrying on behind it; one cut
     * short is proved at the next start.
     */
    private fun relocate(nodeHex: String, at: String, label: String?, subnet: String?) {
        val p = Gateway.loaded[by] ?: return
        Thread({
            val node = Gateway.unhex(nodeHex)
            val held = Gateway.call("offerRelocation", { e -> Diag.warn("relocation", "why" to e.reason); false }) {
                p.offerRelocation(node, at, subnet?.let { Gateway.unhex(it) }, label)
            }
            val r = if (held) {
                Gateway.call("proveRelocation", { e -> Diag.warn("relocation", "why" to e.reason); null }) { p.proveRelocation(node) }
            } else {
                null
            }
            Diag.event("relocation.offered", "node" to nodeHex.take(8), "held" to held, "standing" to r?.standing)
            Model.refreshAll()
        }, "fueros-relocate").start()
        finish()
    }
}
