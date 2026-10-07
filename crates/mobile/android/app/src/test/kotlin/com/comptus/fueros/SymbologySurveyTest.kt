package com.comptus.fueros

import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.aztec.AztecWriter
import com.google.zxing.datamatrix.DataMatrixWriter
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import org.junit.Test

/**
 * **A survey, not an assertion**: what each 2D symbology carries at each
 * module count, from the real encoders over the real contribution. The
 * question is duration, not range [author, 2026-10-07] — more bits in a
 * frame at an unchanged module means fewer parts and a shorter exchange.
 *
 * No capacity figure is taken from recollection: that habit produced the
 * `450 / modules` rule and cost two field cycles.
 *
 * ---
 *
 * **AZTEC WAS TRIED ON HARDWARE AND REJECTED. The decode floor below is
 * measured on a perfect render and it does not predict what a camera
 * does.** Both symbologies decode from two pixels a module here, and in
 * front of a phone:
 *
 * | | QR, 102 parts | Aztec bare, 39 | Aztec +margin, 64 |
 * |---|---|---|---|
 * | optical pass | **48.4 s** | 72.6 s | 98.1 s |
 * | a part | **0.47 s** | 1.86 s | 1.53 s |
 * | decode p90 | **466 ms** | 2,561 ms | 3,029 ms |
 * | range | **~42 in** | ~22–27 in | ~22 in |
 *
 * The quiet zone was the suspected cause and made no difference at an
 * identical drawn module, so the loss is the symbol's own: one central
 * bullseye gives a detector far less to localise and perspective-correct
 * from than three corner finders, which is what a hand-held camera at
 * distance needs and what a synthetic render never asks for.
 *
 * **And the capacity never mattered anyway.** 2.6 times fewer parts gave
 * a 50% *longer* pass. The per-part cost is a lockstep round-trip, not a
 * decode — 0.47 s against a 293 ms median — so payload a frame does not
 * touch it. Anything revisiting the symbology should measure *on
 * hardware*, early, and should expect capacity to buy little.
 */
class SymbologySurveyTest {

    private val contribution = ByteArray(2022) { (it * 31 % 251).toByte() }

    /** The header a part carries either way ([OpticalExchange.HEADER]). */
    private val header = 5

    private fun qrBase45(payload: ByteArray): Int {
        val hints = mapOf<EncodeHintType, Any>(
            EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
            EncodeHintType.MARGIN to 2,
            EncodeHintType.CHARACTER_SET to "US-ASCII",
        )
        return QRCodeWriter().encode(Optical.payload(payload), BarcodeFormat.QR_CODE, 0, 0, hints).width
    }

    /** Raw bytes through ISO-8859-1, which is lossless for 0..255 and what
     *  ZXing's own byte handling assumes. */
    private fun latin(b: ByteArray) = String(b, Charsets.ISO_8859_1)

    private fun aztecBytes(payload: ByteArray): Int {
        // 23% is Aztec's own default correction; MARGIN is not honoured and
        // Aztec requires no quiet zone, which is part of what it saves
        val hints = mapOf<EncodeHintType, Any>(EncodeHintType.ERROR_CORRECTION to 23)
        return AztecWriter().encode(latin(payload), BarcodeFormat.AZTEC, 0, 0, hints).width
    }

    private fun dataMatrixBytes(payload: ByteArray): Int {
        val hints = mapOf<EncodeHintType, Any>(EncodeHintType.MARGIN to 2)
        return DataMatrixWriter().encode(latin(payload), BarcodeFormat.DATA_MATRIX, 0, 0, hints).width
    }

    private fun survey(name: String, width: (ByteArray) -> Int) {
        println("== $name")
        println("chunk\tparts\tmodules")
        var last = -1
        val rows = mutableListOf<Triple<Int, Int, Int>>()
        for (chunk in 4..120) {
            val parts = (contribution.size + chunk - 1) / chunk
            if (parts > 255) continue
            // **real bytes, not zeros.** An all-zero part base45s to a
            // string of '0' and QR then picks *numeric* mode, three digits
            // to ten bits, which flatters every symbology surveyed with it.
            // The first pass of this survey did exactly that and reported
            // 55 parts at 29 modules where the real figure is 102.
            val part = ByteArray(header + chunk) { i ->
                if (i < header) (i * 37 + 11).toByte() else contribution[(i - header) % contribution.size]
            }
            val m = try {
                width(part)
            } catch (e: Exception) {
                continue
            }
            rows.add(Triple(chunk, parts, m))
            if (m != last) {
                last = m
            }
        }
        // the cheapest chunk at each module count: the last before it grows
        val byModules = rows.groupBy { it.third }
        byModules.keys.sorted().forEach { m ->
            val best = byModules.getValue(m).maxByOrNull { it.first }!!
            println("${best.first}\t${best.second}\t$m")
        }
    }

    /**
     * **The minimum pixels a module needs to decode**, which is the half
     * of this that capacity does not answer. A symbology that packs more
     * into the same module is no gain if its detector needs a sharper
     * image: that trade is what put decode p90 at 1979 ms and three
     * watchdog restarts at 41 QR modules.
     *
     * Renders the matrix at a whole number of pixels a module, as the
     * screen does, and asks the real reader to find it.
     */
    private fun readsAt(m: com.google.zxing.common.BitMatrix, px: Int, reader: com.google.zxing.Reader): Boolean {
        val w = m.width * px
        val h = m.height * px
        // white where the matrix is clear, as the screen draws it
        val lum = ByteArray(w * h) { -1 }
        for (y in 0 until m.height) {
            for (x in 0 until m.width) {
                if (!m.get(x, y)) continue
                for (dy in 0 until px) {
                    val row = (y * px + dy) * w + x * px
                    for (dx in 0 until px) lum[row + dx] = 0
                }
            }
        }
        val src = com.google.zxing.PlanarYUVLuminanceSource(lum, w, h, 0, 0, w, h, false)
        val hints = mapOf<com.google.zxing.DecodeHintType, Any>(
            com.google.zxing.DecodeHintType.TRY_HARDER to true,
        )
        return try {
            reader.decode(
                com.google.zxing.BinaryBitmap(com.google.zxing.common.HybridBinarizer(src)),
                hints,
            )
            true
        } catch (e: Exception) {
            false
        }
    }

    private fun floorPx(m: com.google.zxing.common.BitMatrix, reader: com.google.zxing.Reader): Int {
        for (px in 1..8) if (readsAt(m, px, reader)) return px
        return -1
    }

    @Test
    fun the_decode_floor_at_each_candidate() {
        val hdr = ByteArray(header) { (it * 37 + 11).toByte() }
        fun part(chunk: Int) = hdr + ByteArray(chunk) { contribution[it % contribution.size] }
        val qrHints = mapOf<EncodeHintType, Any>(
            EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
            EncodeHintType.MARGIN to 2,
            EncodeHintType.CHARACTER_SET to "US-ASCII",
        )
        println("== decode floor, pixels a module (lower is better)")
        println("symbology	chunk	parts	drawn	floor_px")
        fun row(name: String, chunk: Int, m: com.google.zxing.common.BitMatrix, reader: com.google.zxing.Reader) {
            val parts = (contribution.size + chunk - 1) / chunk
            println("$name	$chunk	$parts	${m.width}	${floorPx(m, reader)}")
        }
        // today's point
        row(
            "QR base45", 20,
            QRCodeWriter().encode(Optical.payload(part(20)), BarcodeFormat.QR_CODE, 0, 0, qrHints),
            com.google.zxing.qrcode.QRCodeReader(),
        )
        // Aztec, bare and with a margin drawn round it
        for (chunk in listOf(32, 52)) {
            val bare = AztecWriter().encode(
                latin(part(chunk)), BarcodeFormat.AZTEC, 0, 0,
                mapOf(EncodeHintType.ERROR_CORRECTION to 23),
            )
            row("Aztec bare", chunk, bare, com.google.zxing.aztec.AztecReader())
            row("Aztec +2", chunk, bordered(bare, 2), com.google.zxing.aztec.AztecReader())
        }
        for (chunk in listOf(28, 36)) {
            val dm = DataMatrixWriter().encode(
                latin(part(chunk)), BarcodeFormat.DATA_MATRIX, 0, 0,
                mapOf(EncodeHintType.MARGIN to 2),
            )
            row("DataMatrix", chunk, dm, com.google.zxing.datamatrix.DataMatrixReader())
        }
    }

    /** The matrix with `n` clear modules round it, as a quiet zone. */
    private fun bordered(m: com.google.zxing.common.BitMatrix, n: Int): com.google.zxing.common.BitMatrix {
        val out = com.google.zxing.common.BitMatrix(m.width + 2 * n, m.height + 2 * n)
        for (y in 0 until m.height) for (x in 0 until m.width) if (m.get(x, y)) out.set(x + n, y + n)
        return out
    }

    /** The boundaries through the real exchange, which is what ships:
     *  `OpticalExchange.frame()` builds the part and `Optical.matrix`
     *  encodes it, so this is the number `qr.shown` will report. */
    @Test
    fun the_shipping_path_s_own_boundaries() {
        println("== Aztec through OpticalExchange + Optical.matrix")
        println("chunk\tparts\tmodules")
        var last = -1
        var prev = ""
        for (chunk in 20..90) {
            val x = OpticalExchange(OpticalExchange.CONTRIBUTION, contribution, chunk)
            if (x.count > 255) continue
            val m = Optical.matrix(x.frame()).width
            if (m != last && prev.isNotEmpty()) println(prev)
            prev = "$chunk\t${x.count}\t$m"
            last = m
        }
        println(prev)
    }

    @Test
    fun survey_every_symbology() {
        survey("QR, base45 in alphanumeric mode (today)", ::qrBase45)
        survey("Aztec, raw bytes", ::aztecBytes)
        survey("Data Matrix, raw bytes", ::dataMatrixBytes)
    }
}
