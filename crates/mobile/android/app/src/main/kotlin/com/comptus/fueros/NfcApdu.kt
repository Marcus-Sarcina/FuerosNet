package com.comptus.fueros

/**
 * The NFC tap's two APDUs, as pure bytes (ISO 7816-4 framing).
 *
 * **What a tap is evidence of, and what it is not.** design §7.6.3 ranks
 * NFC second and says plainly that it is *physical-range friction, not a
 * distance-bounding guarantee*: a relay pair with a fast link defeats it.
 * So the exchange here carries the ceremony-id — a **public** value — and
 * checking it catches a tap against the wrong ceremony, never an adversary
 * quoting the screens (`wire-format.md` §14.3.1's own caveat, applied to
 * this channel). Both sides pass only where the two ids agree.
 *
 * This file is the codec alone, so a JVM test can hold both roles; the
 * radio halves live in [NfcChannel] and the HCE service.
 */
object NfcApdu {
    /**
     * The application this shell selects, in the proprietary range
     * (first nibble F). Like the bearer's UUIDs, it identifies *this
     * shell's* exchange and nothing more — the protocol names no bearer
     * and no AID.
     */
    val AID: ByteArray = byteArrayOf(
        0xF0.toByte(), 0x46, 0x55, 0x45, 0x52, 0x4F, 0x53,
    )

    private val OK = byteArrayOf(0x90.toByte(), 0x00)
    private val REFUSED = byteArrayOf(0x69, 0x85.toByte())
    private val UNKNOWN = byteArrayOf(0x6D, 0x00)

    /** `SELECT by AID`, the first command of every contact. */
    fun select(): ByteArray =
        byteArrayOf(0x00, 0xA4.toByte(), 0x04, 0x00, AID.size.toByte()) + AID

    /** The exchange: the reader's ceremony-id, asking for the card's. */
    fun exchange(ceremonyId: ByteArray): ByteArray {
        require(ceremonyId.size == 32) { "a ceremony-id is 32 bytes" }
        return byteArrayOf(0x80.toByte(), 0xCE.toByte(), 0x00, 0x00, 32) +
            ceremonyId + byteArrayOf(0x20)
    }

    /**
     * The card's side: one command in, one response out, and whether the
     * exchange PASSED for the card. `ours` is this device's ceremony-id,
     * or null where no ceremony is open — a tap against a phone with
     * nothing running answers unknown rather than leaking that a ceremony
     * exists elsewhere in time.
     */
    fun respond(command: ByteArray, ours: ByteArray?): Pair<ByteArray, Boolean> {
        // SELECT: yes, this application lives here
        if (command.size >= 5 + AID.size &&
            command[1] == 0xA4.toByte() &&
            command.copyOfRange(5, 5 + AID.size).contentEquals(AID)
        ) {
            return OK to false
        }
        if (command.size >= 5 + 32 && command[1] == 0xCE.toByte()) {
            val cid = ours ?: return UNKNOWN to false
            val theirs = command.copyOfRange(5, 5 + 32)
            // a tap against another ceremony is refused, and the refusal
            // carries nothing: the reader learns only that this card is
            // not in its ceremony, which the mismatch already told it
            return if (theirs.contentEquals(cid)) (cid + OK) to true else REFUSED to false
        }
        return UNKNOWN to false
    }

    /** The reader's side: whether the card's response is a pass. */
    fun passed(response: ByteArray, ours: ByteArray): Boolean =
        response.size == 34 &&
            response.copyOfRange(32, 34).contentEquals(OK) &&
            response.copyOfRange(0, 32).contentEquals(ours)
}
