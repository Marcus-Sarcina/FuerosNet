package com.comptus.fueros

import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.common.BitMatrix
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel

/**
 * The optical channel, as this shell carries it (`wire-format.md` §14.3.1).
 *
 * **The kernel fixes the bytes and this file fixes nothing else.** §14.3.2
 * gives `OpticalContribution` and `TranscriptConfirm` their encodings; what
 * a QR does to carry those bytes across is the shell's own business, and
 * the protocol neither names nor depends on it — a counterparty running a
 * different shell could carry the same bytes any way the two of them
 * manage, which is what §14.3.1 means by leaving the carriage to whatever
 * means the two have.
 *
 * **Why base64url and not QR byte mode.** A QR can carry raw bytes, but
 * every layer between a camera and this code is a place where binary gets
 * mangled by a charset nobody chose — ZXing's byte mode decodes through
 * ISO-8859-1 by convention and a scanner on the other side may not. The
 * payload is therefore text, and the price is a third more characters on
 * objects of 34 and 53 bytes: 46 and 71 characters, which is nothing for a
 * QR. §14.3.1's claim is that *"a QR of this size resolves at arm's length
 * on a modest selfie camera"*, and the sizes below are what keep that
 * true. **The contribution is not of this size**: it carries a full key,
 * and what keeps *it* readable is being cut into parts
 * (`OpticalExchange.CHUNK`), one symbol per code, rather than anything
 * here.
 */
object Optical {
    /**
     * **Base45 (RFC 9285)**, whose alphabet is exactly QR's alphanumeric
     * set, so the symbol encodes in alphanumeric mode: 2 bytes become 3
     * characters of 5.5 bits each, 1.5× the bytes, where base64 in byte
     * mode would be 1.33× the bytes at 8 bits each — a third fewer modules
     * for the same payload. The first optical code carries a full key,
     * about 2 KB, which in base64 would not fit a QR at error correction M
     * at all and does here with room.
     */
    private const val A = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ \$%*+-./:"

    /**
     * **Error correction M, 15%.** L would make a smaller symbol and a
     * less forgiving one; Q and H cost modules that make the symbol denser
     * at the same physical size, which is the wrong trade for a phone held
     * at arm's length — the limit there is the camera's resolution of fine
     * modules, not the number of smudges.
     */
    /** **The quiet zone, in modules each side.** Named because the
     *  tracking rows are laid out against the drawn matrix and have to
     *  know where the symbol inside it starts (`Tracking.cells`). */
    const val MARGIN = 2

    private val HINTS = mapOf<EncodeHintType, Any>(
        EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
        EncodeHintType.MARGIN to MARGIN,
        EncodeHintType.CHARACTER_SET to "US-ASCII", // alphanumeric mode is what the alphabet earns
    )

    /** `bytes` in base45 (RFC 9285), which is the alphabet a QR's
     *  alphanumeric mode encodes two characters to eleven bits. */
    fun payload(bytes: ByteArray): String {
        val out = StringBuilder((bytes.size + 1) / 2 * 3)
        var i = 0
        while (i + 1 < bytes.size) {
            // two bytes, little-endian base 45: c + 45 d + 45² e
            var n = ((bytes[i].toInt() and 0xff) shl 8) or (bytes[i + 1].toInt() and 0xff)
            out.append(A[n % 45]); n /= 45
            out.append(A[n % 45]); n /= 45
            out.append(A[n])
            i += 2
        }
        if (i < bytes.size) {
            // one byte left: two characters
            var n = bytes[i].toInt() and 0xff
            out.append(A[n % 45]); n /= 45
            out.append(A[n])
        }
        return out.toString()
    }

    /** The bytes back, or null: a QR that is not one of ours reads as one. */
    fun bytes(payload: String): ByteArray? {
        if (payload.isEmpty()) return ByteArray(0)
        // one leftover character cannot be a byte: base45's lengths are 3k
        // and 3k+2, never 3k+1
        val n = payload.length
        if (n % 3 == 1) return null
        val out = ByteArray(n / 3 * 2 + if (n % 3 == 2) 1 else 0)
        var o = 0
        var i = 0
        while (i < n) {
            val take = minOf(3, n - i)
            var acc = 0
            var w = 1
            for (k in 0 until take) {
                val c = A.indexOf(payload[i + k])
                if (c < 0) return null
                acc += c * w
                w *= 45
            }
            if (take == 3) {
                if (acc > 0xffff) return null
                out[o++] = (acc ushr 8).toByte()
                out[o++] = (acc and 0xff).toByte()
            } else {
                if (acc > 0xff) return null
                out[o++] = acc.toByte()
            }
            i += take
        }
        return out
    }

    /**
     * The symbol for `bytes`, as a module matrix. `size` is the requested
     * side in modules-or-more; ZXing returns at least the version the data
     * needs, so a small request yields the smallest symbol that fits.
     */
    fun matrix(bytes: ByteArray, size: Int = 0): BitMatrix {
        val text = payload(bytes)
        return QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, size, size, HINTS)
    }
}
