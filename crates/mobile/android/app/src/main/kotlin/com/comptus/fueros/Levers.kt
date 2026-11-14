package com.comptus.fueros

import android.content.Context
import android.content.Intent

/**
 * **The bench's levers** [restructure step 4, 2026-11-14]: what the
 * field-test build reads off an intent's extras and nothing else does.
 * Every lever names what it acts on or acts for the identity in focus,
 * which is the bench's own choice of actor; the releasable build has
 * none of this, [BuildConfig.FIELD_TEST] being false there.
 *
 * `--es diag_stream <host:port>` with `--es diag_serial <serial>` sets
 * the live diagnostic stream; `--es active_identity <hex>` is read before
 * the kernel starts, since the bring-up is what applies it;
 * `--es forget_contact <keyhash>` takes one contact out of the focused
 * identity's dataset and `--ez forget_contacts true` every one;
 * `--es agree_to <node hex>@<host:port>` re-points an agreement;
 * `--es export_material 1` says the focused identity's public material
 * as an event; `--es forget_captures <keyhash>` drops every sealed
 * capture held of that party; `--ez contact_peers true` reaches for the
 * peers now.
 */
object Levers {
    /** The levers read before the kernel starts. */
    fun beforeStart(context: Context, intent: Intent) {
        if (!BuildConfig.FIELD_TEST) return
        intent.getStringExtra("diag_stream")?.let {
            FuerosApp.streamTo(context, it, intent.getStringExtra("diag_serial"))
        }
        intent.getStringExtra("active_identity")?.let { Identities.requestActive(it) }
    }

    /** The levers that need a kernel already up, read at every start and every new intent. */
    fun apply(intent: Intent) {
        if (!BuildConfig.FIELD_TEST) return
        val by = Identities.focused()
        intent.getStringExtra("forget_contact")?.let { Kernel.forgetContact(by, it) }
        intent.getStringExtra("agree_to")?.let { Attachments.agreeToNow(by, it) }
        intent.getStringExtra("export_material")?.let {
            Diag.event(
                "identity.material",
                "who" to by.take(8),
                "hex" to Kernel.materialHex(by),
            )
        }
        if (intent.getBooleanExtra("forget_contacts", false)) Kernel.forgetContacts(by)
        intent.getStringExtra("forget_captures")?.let { Attachments.forgetCaptures(by, it) }
        if (intent.getBooleanExtra("contact_peers", false)) Attachments.contactPeersNow(by)
    }
}
