package com.comptus.fueros

import android.app.Activity
import android.os.Bundle

/**
 * **An invitation to serve from a node, taken in and never shown**
 * (`rhtn_client::invitation`, `rhtn_client::relocation`).
 *
 * [author, 2026-11-04] *"Attachment is not a separate act. The operator
 * should attach upon launch and install, other horizon members should
 * agree to attach once when invited and then all horizon members should
 * re-attach every time they become detached."*
 *
 * **There is no invitation screen** [author, 2026-11-17]: *"the invite
 * should manifest in the UI only in the notification counter on the
 * subnet shield. When the subnet shield is launched, that is when the user
 * sees the invite in the form of the popup."* So this window is clear and
 * goes at once. The link is read behind it and, where it places a node,
 * held for that node — a first invitation as a move from no address — and
 * proved; the subnet it names carries the mark once it has, and the
 * question ([Relocate]) is the person's one tap.
 *
 * **What secures the agreement is not text.** The link carries no key: an
 * address and an index — which patron, which subnet, which identity — and
 * the key that must answer comes from this device's own archive, so an
 * invitation cannot introduce a patron. What the person weighs is the
 * adoption it follows: *"what secures it is physical proximity to the
 * Patron"*, and an invitation that arrives soon after, on that subnet's
 * orb, from that patron's or their patron's server.
 *
 * **An invitation that places nothing says so to the log alone**: one
 * naming a patron this device is not under, an identity not running here,
 * or none. Every invitation an operator's client writes names its patron
 * and its subnet.
 */
class InviteActivity : Activity() {
    private companion object {
        /** How long to wait for the kernel a deep link did not start. */
        const val KERNEL_WAIT_MS = 8000
        const val STEP_MS = 250
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // the identity named by whoever opened it, else the one in focus
        val by = intent.getStringExtra("by") ?: Identities.focused()
        // the link as it arrived: a tap on a message, or opened with one in
        // hand by the bench
        val link = intent.data?.toString() ?: intent.getStringExtra("invitation") ?: ""
        val app = applicationContext
        finish()
        if (link.isEmpty()) return
        if (!Identities.hasIdentity(app)) {
            Diag.warn("invitation.taken", "why" to "no identity on this device")
            return
        }
        // **a deep link opens this cold** [2026-11-04]: nothing but the
        // launcher brings the kernel up, so this waits for it rather than
        // reading once and calling the absence a bad link
        Attachments.start(app)
        Thread({
            var read = Gateway.invitation(by, link)
            var waited = 0
            while (read == null && waited < KERNEL_WAIT_MS) {
                Thread.sleep(STEP_MS.toLong())
                waited += STEP_MS
                read = Gateway.invitation(by, link)
            }
            val node = read?.node
            if (read == null || node == null || read.why.isNotEmpty()) {
                val why = read?.why?.ifEmpty { "it names no patron" } ?: Gateway.invitationSay(link).ifEmpty { "the kernel did not start" }
                Diag.warn("invitation.taken", "why" to why)
                return@Thread
            }
            hold(by, node, read.at, read.asLabel, read.subnet)
        }, "fueros-invite").start()
    }

    /**
     * **Held for the node and proved at once**: the kernel dials the
     * address and binds what answers there to the patron before the person
     * is asked anything; one cut short is proved at the next start.
     */
    private fun hold(by: String, nodeHex: String, at: String, label: String?, subnet: String?) {
        val p = Gateway.loaded[by] ?: return
        val node = Gateway.unhex(nodeHex)
        val held = Gateway.call("offerRelocation", { e -> Diag.warn("relocation", "why" to e.reason); false }) {
            p.offerRelocation(node, at, subnet?.let { Gateway.unhex(it) }, label)
        }
        val r = if (held) {
            Gateway.call("proveRelocation", { e -> Diag.warn("relocation", "why" to e.reason); null }) { p.proveRelocation(node) }
        } else {
            null
        }
        Diag.event("invitation.taken", "node" to nodeHex.take(8), "held" to held, "standing" to r?.standing)
        Model.refreshAll()
    }
}
