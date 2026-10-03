package com.comptus.fueros

import android.nfc.cardemulation.HostApduService
import android.os.Bundle

/**
 * The tap's card side (D3): host-card emulation answering [NfcApdu]'s two
 * commands with this device's ceremony-id.
 *
 * The system instantiates this when a reader selects the AID, so it
 * reaches the ceremony through [ProximityChannels] and holds nothing of
 * its own. A phone with no ceremony open answers unknown — the service
 * exists whenever the app does, and its answers must not say more than a
 * ceremony's existence allows.
 *
 * **NOT RUN ON HARDWARE**, like every radio half in this shell.
 */
class NfcCeremonyService : HostApduService() {

    private var apdus = 0

    override fun processCommandApdu(command: ByteArray, extras: Bundle?): ByteArray {
        val (response, passed) = NfcApdu.respond(command, ProximityChannels.current())
        apdus += 1
        Diag.event(
            "nfc",
            "op" to "hce",
            "command" to NfcApdu.kind(command),
            "status" to NfcApdu.status(response),
            "passed" to passed,
            "apdus" to apdus,
        )
        if (passed) ProximityChannels.tapServed()
        return response
    }

    override fun onDeactivated(reason: Int) {
        Diag.event("nfc", "op" to "hce", "state" to "deactivated", "reason" to reason, "apdus" to apdus)
        apdus = 0
    }
}
