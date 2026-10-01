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
 * on a modest selfie camera"*, and the sizes below are what keep that true.
 */
object Optical {
    /** The alphabet, RFC 4648 §5, and no padding: `=` buys nothing here. */
    private const val A = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"

    /**
     * **Error correction M, 15%.** L would make a smaller symbol and a
     * less forgiving one; Q and H cost modules that make the symbol denser
     * at the same physical size, which is the wrong trade for a phone held
     * at arm's length — the limit there is the camera's resolution of fine
     * modules, not the number of smudges.
     */
    private val HINTS = mapOf<EncodeHintType, Any>(
        EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
        EncodeHintType.MARGIN to 2,
        EncodeHintType.CHARACTER_SET to "US-ASCII",
    )

    fun payload(bytes: ByteArray): String {
        val out = StringBuilder((bytes.size * 4 + 2) / 3)
        var i = 0
        while (i < bytes.size) {
            val b0 = bytes[i].toInt() and 0xff
            val b1 = if (i + 1 < bytes.size) bytes[i + 1].toInt() and 0xff else 0
            val b2 = if (i + 2 < bytes.size) bytes[i + 2].toInt() and 0xff else 0
            val n = (b0 shl 16) or (b1 shl 8) or b2
            out.append(A[n ushr 18 and 63]).append(A[n ushr 12 and 63])
            if (i + 1 < bytes.size) out.append(A[n ushr 6 and 63])
            if (i + 2 < bytes.size) out.append(A[n and 63])
            i += 3
        }
        return out.toString()
    }

    /** The bytes back, or null: a QR that is not one of ours reads as one. */
    fun bytes(payload: String): ByteArray? {
        if (payload.isEmpty()) return ByteArray(0)
        // one leftover character cannot be a byte: base64url's lengths are
        // 4k, 4k+2 and 4k+3, never 4k+1
        if (payload.length % 4 == 1) return null
        val n = payload.length
        val out = ByteArray(n / 4 * 3 + maxOf(0, n % 4 - 1))
        var o = 0
        var i = 0
        while (i < n) {
            val take = minOf(4, n - i)
            var acc = 0
            for (k in 0 until 4) {
                val c = if (k < take) A.indexOf(payload[i + k]) else 0
                if (k < take && c < 0) return null
                acc = (acc shl 6) or c
            }
            out[o++] = (acc ushr 16).toByte()
            if (take > 2) out[o++] = (acc ushr 8 and 0xff).toByte()
            if (take > 3) out[o++] = (acc and 0xff).toByte()
            i += 4
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
