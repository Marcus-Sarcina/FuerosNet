package com.comptus.fueros

/**
 * **How this device gives what a resource asked for** — the pure half
 * (`light-client-requirements.md` §8.1; `resource-requirements.md` §3.4).
 *
 * The kernel reads the ask, puts it to the person and composes the
 * answer. What is left is the device's own: which capability each utility
 * needs, whether this device has it, what the person is told at the
 * moment of giving, and the encoding of the one result that has a format
 * of its own — a place, as a geohash.
 *
 * **It is pure so that it can be held to.** The camera, the picker, the
 * radio, the unlock and the location service are the Activity's; every
 * decision about *what* to ask the person for, and *what this device
 * cannot give at all*, is here, where a unit test can read it. A utility
 * this shell cannot give is declined **with a reason the resource sees**,
 * never left silent: that is §3.4's rule, and it is the whole reason the
 * [Way.NONE] plans carry words.
 */
object Giving {

    /** What the shell does to get the result. */
    enum class Way {
        /** A photograph, taken here, as JPEG ([Still]). */
        PHOTOGRAPH,

        /** A file the person picks, handed over as its own type. */
        PICK,

        /** A place, as a geohash at the precision asked. */
        PLACE,

        /** A code read with the camera. */
        TAG_QR,

        /** A tag read with the near-field radio. */
        TAG_NFC,

        /** The platform's own unlock, authorising a signature. */
        UNLOCK,

        /** Standing permission to be shown a resource's notifications. */
        NOTIFY,

        /** Agreement to be handed a file, which the next answer carries. */
        RECEIVE,

        /** **Not on this device**, with [Plan.says] as the reason. */
        NONE,
    }

    /** What this device has, as the Activity finds it. */
    data class Facts(
        /** A camera the shell can open. */
        val camera: Boolean,
        /** A near-field radio, enabled. */
        val nfc: Boolean,
        /** A biometric or a device credential enrolled. */
        val unlock: Boolean,
        /** Whether notifications need the runtime permission here (API 33
         *  and up); below it the manifest's declaration is enough. */
        val notifyNeedsPermission: Boolean,
    )

    /** What the shell will do, what it must hold to do it, and what the
     *  person is told while it happens — or why it cannot. */
    data class Plan(
        val way: Way,
        val permissions: List<String>,
        val says: String,
    )

    /** The most a result may be, leaving the kernel's own headroom under
     *  `wire-format.md` §11's message bound: a capture larger than this is
     *  shrunk or declined here rather than refused after the act. */
    const val RESULT_CAP: Int = (16 shl 20) - 8192

    private const val FINE = "android.permission.ACCESS_FINE_LOCATION"
    private const val COARSE = "android.permission.ACCESS_COARSE_LOCATION"
    private const val CAMERA = "android.permission.CAMERA"
    private const val NOTIFICATIONS = "android.permission.POST_NOTIFICATIONS"

    /**
     * **The plan for one ask.** Everything that decides it is here: the
     * utility, the channel and media the resource asked for, and what
     * this device has.
     */
    fun plan(ask: Kernel.Ask, has: Facts): Plan {
        val photograph = Plan(
            Way.PHOTOGRAPH,
            listOf(CAMERA),
            "Point the camera and take the photograph. It is sent only when you send it.",
        )
        return when (ask.utility) {
            // **a scan is a photograph here, and says so.** Deskewing,
            // reading the text and assembling a PDF want a local model
            // this shell does not carry; a photograph of the document is
            // what it can give, and a resource that will take only a PDF
            // is told so rather than handed an image it cannot use.
            "scan" -> when {
                !has.camera -> Plan(Way.NONE, listOf(), "this device has no camera the app can open")
                !takes(ask, "image/jpeg") -> Plan(
                    Way.NONE,
                    listOf(),
                    "this app can photograph the document but not read it into " +
                        "${ask.media.joinToString(" or ")}",
                )
                else -> photograph.copy(
                    says = "Photograph the document. This app sends the photograph itself: " +
                        "it does not straighten it, read the text or make a PDF.",
                )
            }
            // a photograph, not a video: the provenance is the kernel's
            // claim over what is sent, whatever it is
            "capture" -> when {
                !has.camera -> Plan(Way.NONE, listOf(), "this device has no camera the app can open")
                !takes(ask, "image/jpeg") -> Plan(
                    Way.NONE,
                    listOf(),
                    "it asks for ${ask.media.joinToString(" or ")} and this app can give a photograph",
                )
                else -> photograph.copy(
                    says = "Take the photograph. This app sends a still photograph, not video, " +
                        "with the time it was taken and this device's claim that it took it.",
                )
            }
            "location" -> Plan(
                Way.PLACE,
                // **the coarser permission wherever it will do**: a cell
                // of a kilometre or more does not need the fine one, and
                // asking for less is the whole of what this can do about
                // the most sensitive field there is (design §8.1.1)
                if ((ask.precision ?: 0) >= 7) listOf(FINE, COARSE) else listOf(COARSE),
                "This device finds where it is, and you see exactly what would be sent " +
                    "before anything is.",
            )
            "check-in", "tag" -> when (ask.channel) {
                "nfc" -> if (has.nfc) {
                    Plan(Way.TAG_NFC, listOf(), "Hold the phone against the tag.")
                } else {
                    Plan(Way.NONE, listOf(), "this device has no near-field radio turned on")
                }
                "uwb" -> Plan(
                    Way.NONE,
                    listOf(),
                    "this app does not range over ultra-wideband; a code or a near-field tag it can read",
                )
                // the resource does not mind, so the radio where there is
                // one and the camera otherwise
                null -> when {
                    has.nfc -> Plan(Way.TAG_NFC, listOf(), "Hold the phone against the tag.")
                    has.camera -> Plan(Way.TAG_QR, listOf(CAMERA), "Point the camera at the code.")
                    else -> Plan(Way.NONE, listOf(), "this device has neither a camera nor a near-field radio")
                }
                else -> if (has.camera) {
                    Plan(Way.TAG_QR, listOf(CAMERA), "Point the camera at the code.")
                } else {
                    Plan(Way.NONE, listOf(), "this device has no camera the app can open")
                }
            }
            "sign" -> if (has.unlock) {
                Plan(
                    Way.UNLOCK,
                    listOf(),
                    "Read what you are signing, then unlock to sign it. The signature is " +
                        "this device's key for this resource, and not your network identity.",
                )
            } else {
                Plan(
                    Way.NONE,
                    listOf(),
                    "this device has no screen lock or biometric enrolled, and a signature " +
                        "here is authorised by one",
                )
            }
            "live" -> Plan(
                Way.NONE,
                listOf(),
                "this app carries no live audio or video; it is not built yet",
            )
            "notify" -> Plan(
                Way.NOTIFY,
                if (has.notifyNeedsPermission) listOf(NOTIFICATIONS) else listOf(),
                "This resource may show you notifications from now on, when this app " +
                    "begins collecting them. You can take it back here.",
            )
            "export" -> Plan(
                Way.RECEIVE,
                listOf(),
                "The resource hands this app the file next; you choose where it goes.",
            )
            "import" -> Plan(
                Way.PICK,
                listOf(),
                "Pick the file to hand over. Nothing else of the device is read.",
            )
            // a utility the kernel knows and this table does not: declined
            // with that said, which is the honest answer and not a silence
            else -> Plan(
                Way.NONE,
                listOf(),
                "this app has nothing built for \"${ask.utility}\"",
            )
        }
    }

    /** Whether the ask will take `media`: an ask naming none takes what
     *  it is given. */
    fun takes(ask: Kernel.Ask, media: String): Boolean =
        ask.media.isEmpty() || ask.media.any { it == media || it == "*/*" || it == media.substringBefore('/') + "/*" }

    /** The types a picker offers for an ask: what it named, or anything. */
    fun types(ask: Kernel.Ask): Array<String> =
        if (ask.media.isEmpty()) arrayOf("*/*") else ask.media.toTypedArray()

    // ---- a place ------------------------------------------------------

    private const val BASE32 = "0123456789bcdefghjkmnpqrstuvwxyz"

    /**
     * **A geohash of `chars` characters** — the wire format's own
     * vocabulary for a place (`wire-format.md` §3.3's precision 3–4 is
     * the model), so what is sent is the cell and not a point.
     *
     * The standard interleave: the first bit halves longitude, the next
     * latitude, five bits to a character.  **A coordinate exactly on a
     * boundary takes the upper half**, which is what every reference
     * implementation does and what makes the null island `s0000000`
     * rather than `7zzzzzzz`.
     */
    fun geohash(lat: Double, lon: Double, chars: Int): String {
        val want = chars.coerceIn(1, 12)
        var latMin = -90.0
        var latMax = 90.0
        var lonMin = -180.0
        var lonMax = 180.0
        val out = StringBuilder(want)
        var even = true
        var bit = 0
        var ch = 0
        while (out.length < want) {
            if (even) {
                val mid = (lonMin + lonMax) / 2
                if (lon >= mid) {
                    ch = ch or (1 shl (4 - bit))
                    lonMin = mid
                } else {
                    lonMax = mid
                }
            } else {
                val mid = (latMin + latMax) / 2
                if (lat >= mid) {
                    ch = ch or (1 shl (4 - bit))
                    latMin = mid
                } else {
                    latMax = mid
                }
            }
            even = !even
            if (bit < 4) {
                bit++
            } else {
                out.append(BASE32[ch])
                bit = 0
                ch = 0
            }
        }
        return out.toString()
    }

    /** How wide a cell of `chars` characters is, in words a person reads
     *  before sending one. */
    fun cell(chars: Int): String = when (chars.coerceIn(1, 12)) {
        1 -> "about 5,000 km across"
        2 -> "about 1,250 km across"
        3 -> "about 156 km across"
        4 -> "about 39 km across"
        5 -> "about 4.9 km across"
        6 -> "about 1.2 km across"
        7 -> "about 153 m across"
        8 -> "about 38 m across"
        9 -> "about 4.8 m across"
        10 -> "about 1.2 m across"
        11 -> "about 15 cm across"
        else -> "about 4 cm across"
    }

    /**
     * **What the person is shown before a place is sent**: the cell, how
     * wide it is, and whether this device's own fix is even that good —
     * a nine-character cell off a fix accurate to a hundred metres claims
     * a precision nobody has, and saying so is the honest alternative to
     * quietly sending it.
     */
    fun placeSays(hash: String, accuracyM: Float?): String {
        val says = StringBuilder("This would send ")
        says.append(hash).append(", a cell ").append(cell(hash.length)).append(".")
        if (accuracyM != null) {
            says.append(" This device puts itself within about ")
                .append(accuracyM.toInt()).append(" m")
            if (coarserThanCell(hash.length, accuracyM)) {
                says.append(", which is wider than the cell — the cell is as precise as the ")
                    .append("resource asked, not as precise as this device knows")
            }
            says.append(".")
        }
        return says.toString()
    }

    /** Whether the fix is coarser than the cell it would be reported in. */
    fun coarserThanCell(chars: Int, accuracyM: Float): Boolean =
        accuracyM > cellMetres(chars)

    /** A cell's width in metres, for the comparison above. */
    fun cellMetres(chars: Int): Double = when (chars.coerceIn(1, 12)) {
        1 -> 5_000_000.0
        2 -> 1_250_000.0
        3 -> 156_000.0
        4 -> 39_000.0
        5 -> 4_900.0
        6 -> 1_200.0
        7 -> 153.0
        8 -> 38.0
        9 -> 4.8
        10 -> 1.2
        11 -> 0.15
        else -> 0.04
    }

    // ---- a signature --------------------------------------------------

    /**
     * **The exact payload, shown.** A person asked to sign is shown what
     * they are signing: the text where it is text, and its size and
     * nothing else where it is not — a body rendered as something it is
     * not declared to be is the sniffing §8 forbids, one step along.
     */
    fun payloadSays(payload: ByteArray?): String {
        if (payload == null || payload.isEmpty()) return "There is nothing to sign."
        val text = runCatching { String(payload, Charsets.UTF_8) }.getOrNull()
        val printable = text != null &&
            text.toByteArray(Charsets.UTF_8).contentEquals(payload) &&
            text.none { it.isISOControl() && it != '\n' && it != '\t' && it != '\r' }
        return if (printable && payload.size <= 4096) {
            "You would be signing these ${payload.size} bytes, exactly as they are:\n\n$text"
        } else {
            "You would be signing ${payload.size} bytes, which are not text this app can " +
                "show you. Sign it only if you know what the resource sent."
        }
    }

    // ---- a tag ---------------------------------------------------------

    /**
     * **What a near-field tag said**, out of one NDEF record: the text of
     * a text record, the address of a URI record, and nothing for any
     * other kind — a payload of some other shape is not text, and
     * reading it as text is the sniffing §8 forbids.
     *
     * A text record carries a status byte whose low six bits are the
     * length of the language code that follows it; the text is what is
     * left (NFCForum-TS-RTD_Text, and `android.nfc.NdefRecord`).
     */
    fun ndefText(tnf: Short, type: ByteArray, payload: ByteArray): String? {
        if (tnf != TNF_WELL_KNOWN) return null
        if (type.contentEquals(RTD_TEXT)) {
            if (payload.isEmpty()) return null
            val language = payload[0].toInt() and 0x3F
            if (payload.size < 1 + language) return null
            return String(payload, 1 + language, payload.size - 1 - language, Charsets.UTF_8)
        }
        if (type.contentEquals(RTD_URI)) {
            if (payload.isEmpty()) return null
            val prefix = URI_PREFIXES.getOrElse(payload[0].toInt() and 0xFF) { "" }
            return prefix + String(payload, 1, payload.size - 1, Charsets.UTF_8)
        }
        return null
    }

    /** A tag's own identifier, as hex: what a tag carrying no record is. */
    fun tagId(id: ByteArray): String = id.joinToString("") { "%02x".format(it) }

    private const val TNF_WELL_KNOWN: Short = 1
    private val RTD_TEXT = byteArrayOf('T'.code.toByte())
    private val RTD_URI = byteArrayOf('U'.code.toByte())

    /** The NDEF URI record's abbreviation table, as far as it is used by
     *  anything a person taps: the index is the payload's first byte. */
    private val URI_PREFIXES = listOf(
        "", "http://www.", "https://www.", "http://", "https://", "tel:", "mailto:",
    )

    /** Where a resource's standing notification permission is kept: the
     *  shell's own record, per resource, and nothing of the kernel's. */
    const val NOTIFY_STORE: String = "utility-notify"
}
