package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **The device half of a resource's typed ask** (`light-client-requirements.md`
 * §8.1; `resource-requirements.md` §3.4).
 *
 * What a camera, a radio, an unlock or a location service does cannot be
 * exercised on the JVM. What can: the decision of **which** capability
 * each utility needs and **whether this device can give it at all**, the
 * words the person is shown, and the two encodings that have a format of
 * their own — a geohash, and what a near-field tag said.
 *
 * The defect this is written against: a utility the shell has nothing
 * built for answering with silence. §3.4 has every ask answered, so every
 * plan this cannot carry out carries the reason instead, and the test
 * below holds that for every name the kernel can send.
 */
class GivingTest {

    private fun ask(
        utility: String,
        precision: Int? = null,
        media: List<String> = listOf(),
        channel: String? = null,
        payload: ByteArray? = null,
        seconds: ULong? = null,
    ) = Kernel.Ask(
        id = 1UL,
        utility = utility,
        purpose = "the reading",
        retention = "ninety days",
        resource = "02".repeat(32),
        node = "01".repeat(32),
        payload = payload,
        precision = precision,
        media = media,
        channel = channel,
        seconds = seconds,
    )

    private val phone = Giving.Facts(camera = true, nfc = true, unlock = true, notifyNeedsPermission = true)
    private val bare = Giving.Facts(camera = false, nfc = false, unlock = false, notifyNeedsPermission = false)

    // ---- every ask is answerable --------------------------------------

    /** The names the kernel's own table carries, read out of it, so a
     *  utility added there and not here is caught by this test rather
     *  than by a resource meeting a silence. */
    private fun utilitiesTheKernelSends(): List<String> {
        var at: File? = File(".").canonicalFile
        while (at != null && !File(at, "crates/client/src/utility.rs").isFile) at = at.parentFile
        val source = at?.let { File(it, "crates/client/src/utility.rs") }
            ?: throw AssertionError(
                "no crates/client/src/utility.rs at or above ${File(".").canonicalPath}: " +
                    "this test reads the kernel's own table, and a runner started elsewhere " +
                    "has to be told where",
            )
        val names = Regex("""Utility::\w+ => "([^"]+)"""")
            .findAll(source.readText())
            .map { it.groupValues[1] }
            .toList()
        assertEquals("the kernel's table reads as ten utilities", 10, names.size)
        return names
    }

    @Test
    fun every_utility_the_kernel_can_send_has_a_plan_or_a_reason() {
        for (name in utilitiesTheKernelSends()) {
            for (facts in listOf(phone, bare)) {
                val plan = Giving.plan(ask(name, precision = 7, payload = byteArrayOf(1)), facts)
                assertTrue(
                    "$name on ${if (facts == phone) "a full phone" else "a bare device"} says nothing",
                    plan.says.isNotBlank(),
                )
                if (plan.way == Giving.Way.NONE) {
                    // the words become the refusal the resource is given,
                    // so they have to read as a reason
                    assertFalse("$name: the reason is a sentence", plan.says.endsWith("."))
                    assertTrue("$name: the reason is said", plan.says.length > 12)
                }
            }
        }
    }

    @Test
    fun a_utility_this_app_has_nothing_for_is_declined_by_name() {
        val plan = Giving.plan(ask("teleport"), phone)
        assertEquals(Giving.Way.NONE, plan.way)
        assertTrue(plan.says, plan.says.contains("teleport"))
    }

    // ---- which capability, and what it needs held ---------------------

    @Test
    fun each_utility_takes_the_capability_it_needs_and_asks_for_no_more() {
        val camera = "android.permission.CAMERA"
        val coarse = "android.permission.ACCESS_COARSE_LOCATION"
        val fine = "android.permission.ACCESS_FINE_LOCATION"

        val scan = Giving.plan(ask("scan"), phone)
        assertEquals(Giving.Way.PHOTOGRAPH, scan.way)
        assertEquals(listOf(camera), scan.permissions)
        assertTrue("the scan says it is a photograph and not a PDF", scan.says.contains("not"))

        // **a coarse cell asks for the coarse permission alone**: the
        // fine one is asked for only where the precision needs it
        val village = Giving.plan(ask("location", precision = 5), phone)
        assertEquals(Giving.Way.PLACE, village.way)
        assertEquals(listOf(coarse), village.permissions)
        val doorway = Giving.plan(ask("location", precision = 9), phone)
        assertEquals(listOf(fine, coarse), doorway.permissions)

        // a signature needs no permission at all: the unlock is a dialogue
        assertEquals(listOf<String>(), Giving.plan(ask("sign", payload = byteArrayOf(1)), phone).permissions)
        // and a file either way needs none
        assertEquals(Giving.Way.PICK, Giving.plan(ask("import"), bare).way)
        assertEquals(listOf<String>(), Giving.plan(ask("import"), bare).permissions)
        assertEquals(Giving.Way.RECEIVE, Giving.plan(ask("export"), bare).way)

        // notifications, where the platform wants asking
        assertEquals(
            listOf("android.permission.POST_NOTIFICATIONS"),
            Giving.plan(ask("notify"), phone).permissions,
        )
        assertEquals(listOf<String>(), Giving.plan(ask("notify"), bare).permissions)
    }

    @Test
    fun a_tag_is_read_by_the_channel_the_resource_asked_for_and_the_device_has() {
        assertEquals(Giving.Way.TAG_NFC, Giving.plan(ask("tag", channel = "nfc"), phone).way)
        assertEquals(Giving.Way.TAG_QR, Giving.plan(ask("tag", channel = "qr"), phone).way)
        // the resource does not mind: the radio where there is one
        assertEquals(Giving.Way.TAG_NFC, Giving.plan(ask("check-in"), phone).way)
        val noRadio = phone.copy(nfc = false)
        assertEquals(Giving.Way.TAG_QR, Giving.plan(ask("check-in"), noRadio).way)
        // it asked for the radio and there is none: that is the reason
        val asked = Giving.plan(ask("check-in", channel = "nfc"), noRadio)
        assertEquals(Giving.Way.NONE, asked.way)
        assertTrue(asked.says, asked.says.contains("near-field"))
        // and ranging is not built
        val uwb = Giving.plan(ask("tag", channel = "uwb"), phone)
        assertEquals(Giving.Way.NONE, uwb.way)
        assertTrue(uwb.says, uwb.says.contains("ultra-wideband"))
    }

    @Test
    fun what_this_device_lacks_is_the_reason_the_resource_is_given() {
        for (utility in listOf("scan", "capture")) {
            val plan = Giving.plan(ask(utility), bare)
            assertEquals(Giving.Way.NONE, plan.way)
            assertTrue(plan.says, plan.says.contains("camera"))
        }
        val unsigned = Giving.plan(ask("sign", payload = byteArrayOf(1)), phone.copy(unlock = false))
        assertEquals(Giving.Way.NONE, unsigned.way)
        assertTrue(unsigned.says, unsigned.says.contains("lock"))
        // live is not built at all, on any device
        val live = Giving.plan(ask("live", seconds = 60UL), phone)
        assertEquals(Giving.Way.NONE, live.way)
        assertTrue(live.says, live.says.contains("not built"))
    }

    @Test
    fun a_resource_that_will_take_only_what_this_app_cannot_make_is_told_so() {
        val pdfOnly = Giving.plan(ask("scan", media = listOf("application/pdf")), phone)
        assertEquals(Giving.Way.NONE, pdfOnly.way)
        assertTrue(pdfOnly.says, pdfOnly.says.contains("application/pdf"))
        // an image is a photograph, and a wildcard takes one
        assertEquals(
            Giving.Way.PHOTOGRAPH,
            Giving.plan(ask("capture", media = listOf("image/jpeg")), phone).way,
        )
        assertEquals(Giving.Way.PHOTOGRAPH, Giving.plan(ask("capture", media = listOf("image/*")), phone).way)
        assertEquals(Giving.Way.PHOTOGRAPH, Giving.plan(ask("capture", media = listOf("*/*")), phone).way)
        // and the picker offers what was named, or anything
        assertArrayEquals(arrayOf("*/*"), Giving.types(ask("import")))
        assertArrayEquals(
            arrayOf("application/pdf", "image/png"),
            Giving.types(ask("import", media = listOf("application/pdf", "image/png"))),
        )
    }

    private fun assertArrayEquals(want: Array<String>, got: Array<String>) =
        assertEquals(want.toList(), got.toList())

    // ---- a place ------------------------------------------------------

    /**
     * The geohash of the reference point every implementation is checked
     * against: 57.64911 N, 10.40744 E is `u4pruydqqvj`, and a shorter
     * precision is a prefix of it, which is the property that makes
     * truncation to the precision asked honest.
     */
    @Test
    fun a_place_encodes_to_the_geohash_of_the_precision_asked() {
        val whole = Giving.geohash(57.64911, 10.40744, 11)
        assertEquals("u4pruydqqvj", whole)
        for (chars in 1..11) {
            assertEquals(whole.take(chars), Giving.geohash(57.64911, 10.40744, chars))
        }
        // the corners and the middle, which is where an interleave goes wrong
        assertEquals("s0000000", Giving.geohash(0.0, 0.0, 8))
        assertEquals("ezs42", Giving.geohash(42.6, -5.6, 5))
        assertEquals("000000", Giving.geohash(-90.0, -180.0, 6))
        // out of range characters are never produced: no a, i, l or o
        for (c in Giving.geohash(51.5007, -0.1246, 12)) {
            assertFalse("the alphabet has no $c", c in "ailo")
        }
        // a precision outside the vocabulary is brought inside it
        assertEquals(1, Giving.geohash(51.5, -0.12, 0).length)
        assertEquals(12, Giving.geohash(51.5, -0.12, 99).length)
    }

    @Test
    fun the_person_is_shown_the_cell_and_told_when_the_fix_is_coarser_than_it() {
        val tight = Giving.placeSays("u4pruyd", 40f)
        assertTrue(tight, tight.contains("u4pruyd"))
        assertTrue(tight, tight.contains("153 m"))
        assertFalse("40 m inside a 153 m cell is not flagged", tight.contains("wider than the cell"))

        val loose = Giving.placeSays("u4pruydqq", 90f)
        assertTrue(loose, loose.contains("wider than the cell"))
        assertTrue("the claim is the resource's precision, not the device's", loose.contains("asked"))

        // with no accuracy at all, the cell is still said and nothing is claimed
        val quiet = Giving.placeSays("u4pru", null)
        assertTrue(quiet, quiet.contains("4.9 km"))
        assertFalse(quiet, quiet.contains("within about"))

        // the cells get smaller as the precision rises, every step
        val widths = (1..12).map { Giving.cellMetres(it) }
        assertEquals(widths.sortedDescending(), widths)
        for (chars in 1..12) assertTrue(Giving.cell(chars).isNotBlank())
    }

    // ---- a signature, and a tag ---------------------------------------

    @Test
    fun the_exact_payload_is_shown_where_it_is_text_and_never_guessed_at() {
        val text = Giving.payloadSays("I accept the terms of the loan".toByteArray())
        assertTrue(text, text.contains("I accept the terms of the loan"))
        assertTrue(text, text.contains("30 bytes"))

        // bytes that are not text are not rendered as text
        val binary = Giving.payloadSays(byteArrayOf(0, 1, 2, 3, -1, -2))
        assertTrue(binary, binary.contains("6 bytes"))
        assertTrue(binary, binary.contains("not text"))

        // nor is a megabyte of text, which no consent screen can show
        val long = Giving.payloadSays(ByteArray(5000) { 'x'.code.toByte() })
        assertTrue(long, long.contains("not text this app can"))

        assertTrue(Giving.payloadSays(null).contains("nothing to sign"))
        assertTrue(Giving.payloadSays(byteArrayOf()).contains("nothing to sign"))
    }

    @Test
    fun a_tag_says_what_its_record_carries_and_its_identifier_otherwise() {
        // a text record: a status byte whose low bits count the language
        // code, then the text
        val language = "en"
        val payload = byteArrayOf(language.length.toByte()) +
            language.toByteArray() + "bay 14".toByteArray()
        assertEquals("bay 14", Giving.ndefText(1, byteArrayOf('T'.code.toByte()), payload))

        // a URI record: the first byte abbreviates the scheme
        val uri = byteArrayOf(0x04) + "example.org/asset/9".toByteArray()
        assertEquals(
            "https://example.org/asset/9",
            Giving.ndefText(1, byteArrayOf('U'.code.toByte()), uri),
        )
        assertEquals("mailto:a@b.c", Giving.ndefText(1, byteArrayOf('U'.code.toByte()), byteArrayOf(0x06) + "a@b.c".toByteArray()))

        // anything else is not text, and is not read as text
        assertNull(Giving.ndefText(2, "application/json".toByteArray(), "{}".toByteArray()))
        assertNull(Giving.ndefText(1, byteArrayOf('S'.code.toByte()), byteArrayOf(1, 2)))
        assertNull(Giving.ndefText(1, byteArrayOf('T'.code.toByte()), byteArrayOf()))
        // a malformed text record, whose language runs past its payload
        assertNull(Giving.ndefText(1, byteArrayOf('T'.code.toByte()), byteArrayOf(9, 'e'.code.toByte())))

        assertEquals("04a21b7c", Giving.tagId(byteArrayOf(4, -94, 27, 124)))
    }

    /** The cap is the kernel's bound less its own headroom, so a capture
     *  is turned away before the act rather than after it. */
    @Test
    fun a_result_is_capped_under_the_messages_own_bound() {
        assertTrue(Giving.RESULT_CAP < 16 shl 20)
        assertTrue(Giving.RESULT_CAP > 8 shl 20)
    }
}
