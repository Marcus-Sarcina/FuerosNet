package com.comptus.fueros

import java.net.ServerSocket
import kotlin.concurrent.thread
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **The operator's side of the enrolment exchange** ([Provision]), against
 * an ordinary socket standing in for an instance's surface.
 *
 * What is checked here is the half that is this shell's: the request it
 * makes, the answer it parses, and that a refusal arrives as the surface's
 * own words rather than a code. The proof itself is the kernel's and is
 * pinned there (`rhtn_crypto::enrolment`), with its vector computed by an
 * implementation that is not ours.
 */
class ProvisionTest {

    /** A surface that answers one request and says what it was asked. */
    private class Surface(val answer: String, val status: String = "200 OK") {
        val socket = ServerSocket(0)
        var asked: String = ""
        var body = ByteArray(0)
        val at get() = "127.0.0.1:${socket.localPort}"

        fun serve() = thread {
            socket.accept().use { c ->
                // **the head is text and the body is not**: a signed
                // object carries every byte value, and a reader that
                // decoded it would replace the ones that are not
                // characters
                val input = c.getInputStream()
                asked = line(input)
                var length = 0
                while (true) {
                    val header = line(input)
                    if (header.isEmpty()) break
                    if (header.lowercase().startsWith("content-length:")) {
                        length = header.substringAfter(':').trim().toInt()
                    }
                }
                if (length > 0) {
                    body = ByteArray(length)
                    var got = 0
                    while (got < length) {
                        val n = input.read(body, got, length - got)
                        if (n <= 0) break
                        got += n
                    }
                }
                c.getOutputStream().write(
                    (
                        "HTTP/1.1 $status\r\ncontent-type: text/plain\r\n" +
                            "content-length: ${answer.toByteArray().size}\r\n" +
                            "connection: close\r\n\r\n$answer"
                        ).toByteArray(),
                )
            }
        }
    }

    @Test
    fun `a fetch carries this device's nonce and reads what the instance answered`() {
        val s = Surface(
            "transport ${"ab".repeat(32)}\nproof ${"cd".repeat(32)}\n" +
                "phase enrolling\ncredentials 0\nendpoint-record wanted\nanchor-entry unconfigured\n",
        )
        s.serve()
        val nonce = ByteArray(16) { it.toByte() }
        val got = Provision.fetch(s.at, nonce).getOrThrow()

        // **the nonce is this caller's and goes on the wire**, which is
        // what makes the answer unreplayable
        assertTrue(
            "asked: ${s.asked}",
            s.asked.startsWith("GET /node?nonce=${Provision.hex(nonce)} "),
        )
        assertEquals(32, got.transportKey.size)
        assertEquals("ab".repeat(32), Provision.hex(got.transportKey))
        assertEquals("cd".repeat(32), Provision.hex(got.proof))
        assertEquals("enrolling", got.phase)
        assertEquals(0, got.credentials)
        assertEquals("wanted", got.endpointRecord)
        assertEquals("unconfigured", got.anchorEntry)
        assertTrue("the nonce it chose comes back with the answer", nonce.contentEquals(got.nonce))
    }

    @Test
    fun `a refusal arrives as the surface's own words`() {
        val s = Surface("the run is not this operator's\n", status = "400 Bad Request")
        s.serve()
        val failed = Provision.fetch(s.at).exceptionOrNull()
        assertTrue("a refusal is not an answer", failed != null)
        assertTrue(
            "and it carries what the surface said: ${failed!!.message}",
            failed.message!!.contains("not this operator's"),
        )
    }

    @Test
    fun `a put sends the signed bytes whole and reads the first line back`() {
        val s = Surface("taken: credential 1 of 7\nmore detail\n")
        s.serve()
        val signed = ByteArray(64) { (it * 3).toByte() }
        val said = Provision.put(s.at, "/node/run", signed).getOrThrow()
        assertEquals("taken: credential 1 of 7", said)
        assertTrue("asked: ${s.asked}", s.asked.startsWith("PUT /node/run "))
        assertEquals(
            "every byte of what was signed, unchanged",
            Provision.hex(signed),
            Provision.hex(s.body),
        )
    }

    @Test
    fun `an answer missing the key or the proof is not an answer`() {
        for (answer in listOf("proof ${"cd".repeat(32)}\n", "transport ${"ab".repeat(32)}\n", "")) {
            val s = Surface(answer)
            s.serve()
            assertTrue(
                "a fetch needs both the key and the proof: $answer",
                Provision.fetch(s.at).isFailure,
            )
        }
    }

    @Test
    fun `hex refuses what is not hex, because a token mistyped is not a token`() {
        assertEquals("00ff10", Provision.hex(byteArrayOf(0, -1, 16)))
        for (bad in listOf("", "abc", "AB", "zz", "00 11")) {
            assertFalse(
                "`$bad` is not lower-case hex of an even length",
                runCatching { Provision.unhex(bad) }.isSuccess,
            )
        }
        assertEquals(32, Provision.unhex("ab".repeat(32)).size)
    }

    @Test
    fun `a nonce is sixteen bytes of this device's own choosing`() {
        val a = Provision.nonce()
        val b = Provision.nonce()
        assertEquals(16, a.size)
        assertFalse("two fetches do not share a nonce", a.contentEquals(b))
    }
}

/** One CRLF-terminated line, read a byte at a time. */
private fun line(input: java.io.InputStream): String {
    val out = StringBuilder()
    while (true) {
        val b = input.read()
        if (b < 0 || b == '\n'.code) break
        if (b != '\r'.code) out.append(b.toChar())
    }
    return out.toString()
}
