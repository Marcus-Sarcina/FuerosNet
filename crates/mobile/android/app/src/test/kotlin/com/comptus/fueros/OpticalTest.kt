package com.comptus.fueros

import com.google.zxing.BinaryBitmap
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The optical channel's carriage, round-tripped on the JVM: the kernel's
 * bytes to a QR symbol and back to the same bytes.
 *
 * **ZXing is pure Java, so the whole path runs here** — only the camera
 * does not, and what a camera adds is blur and perspective rather than a
 * different encoding. A payload that does not survive this cannot survive
 * a lens either.
 */
class OpticalTest {

    /**
     * Scan a matrix the way a decoder would.
     *
     * **Rendered at eight pixels a module**, because that is what a screen
     * does and what a detector needs: a matrix drawn one pixel per module
     * is 37 pixels across and its finder patterns are three pixels wide,
     * which no binarizer resolves. Scanning the unscaled matrix fails, and
     * it fails for a reason that has nothing to do with the payload.
     */
    private fun scan(bytes: ByteArray, scale: Int = 8): ByteArray? {
        val m = Optical.matrix(bytes)
        val w = m.width * scale
        val h = m.height * scale
        val pixels = IntArray(w * h)
        for (y in 0 until h) {
            for (x in 0 until w) {
                val on = m.get(x / scale, y / scale)
                pixels[y * w + x] = if (on) 0xff000000.toInt() else -1
            }
        }
        val bitmap = BinaryBitmap(HybridBinarizer(RGBLuminanceSource(w, h, pixels)))
        val text = QRCodeReader().decode(bitmap).text
        return Optical.bytes(text)
    }

    @Test
    fun the_two_optical_objects_survive_a_symbol_and_come_back_identical() {
        // the sizes §14.3.2 fixes: an OpticalContribution is 53 bytes
        // (array head, version, keyhash, contribution) and a
        // TranscriptConfirm 36
        val contribution = ByteArray(53) { (it * 7 + 1).toByte() }
        val transcript = ByteArray(36) { (it * 11 + 3).toByte() }
        assertArrayEquals(contribution, scan(contribution))
        assertArrayEquals(transcript, scan(transcript))
    }

    @Test
    fun the_symbol_stays_small_enough_to_read_at_arms_length() {
        // §14.3.1 claims a QR of this size resolves on a modest selfie
        // camera. What makes that true is the module count: a version-4
        // symbol is 33 modules and a phone screen renders them large.
        val contribution = ByteArray(53)
        val m = Optical.matrix(contribution)
        assertTrue("53 bytes took ${m.width} modules", m.width <= 45)
        val transcript = ByteArray(36)
        assertTrue(Optical.matrix(transcript).width <= 41)
    }

    @Test
    fun the_payload_round_trips_every_length_and_no_padding_is_written() {
        for (n in 0..200) {
            val bytes = ByteArray(n) { (it * 31 + n).toByte() }
            val text = Optical.payload(bytes)
            assertTrue("alphanumeric only in $text", text.all { it in "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ \$%*+-./:" })
            assertArrayEquals("length $n", bytes, Optical.bytes(text))
        }
    }

    @Test
    fun a_qr_that_is_not_one_of_ours_reads_as_one_rather_than_throwing() {
        assertNull(Optical.bytes("not base45!"))
        assertNull(Optical.bytes("A")) // one character cannot be a byte: lengths are 3k and 3k+2
        assertEquals(0, Optical.bytes("")?.size ?: -1)
    }
}


class OpticalFirstCodeTest {
    /** The first optical code carries a full key, about 2 KB: it must fit a
     *  QR at error correction M, which base45 in alphanumeric mode buys. */
    @Test
    fun a_two_kilobyte_first_code_fits_a_qr_at_correction_m() {
        val bytes = ByteArray(2030) { (it * 31 + 7).toByte() }
        val text = Optical.payload(bytes)
        assertEquals(3045, text.length)
        assertArrayEquals(bytes, Optical.bytes(text))
        val m = Optical.matrix(bytes)
        assertTrue("2030 bytes took ${m.width} modules", m.width <= 177)
    }

    @Test
    fun base45_known_answers_from_rfc_9285() {
        assertEquals("BB8", Optical.payload("AB".toByteArray()))
        assertEquals("%69 VD92EX0", Optical.payload("Hello!!".toByteArray()))
        assertEquals("UJCLQE7W581", Optical.payload("base-45".toByteArray()))
        assertArrayEquals("ietf!".toByteArray(), Optical.bytes("QED8WEX0"))
        // a triple past 0xffff is not two bytes
        assertNull(Optical.bytes("GGW"))
    }
}
