package com.comptus.fueros

import java.net.ServerSocket
import kotlin.concurrent.thread
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.rhtn_ffi.Header

/**
 * **The launch flow's half that is this shell's** ([Launch]): the token it
 * mints, and the HTTPS exchange it makes for the kernel's adapters, against
 * an ordinary socket standing in for a provider. The composition and the
 * adapters are the kernel's and are tested there (`rhtn-launch`).
 */
class LaunchTest {

    /** A server that answers one request and says what it was asked. */
    private class Api(val answer: String, val status: String = "200 OK") {
        val socket = ServerSocket(0)
        var asked = ""
        var auth = ""
        var body = ""
        val base get() = "http://127.0.0.1:${socket.localPort}"

        fun serve() = thread {
            socket.accept().use { c ->
                val input = c.getInputStream()
                asked = line(input)
                var length = 0
                while (true) {
                    val h = line(input)
                    if (h.isEmpty()) break
                    if (h.lowercase().startsWith("content-length:")) length = h.substringAfter(':').trim().toInt()
                    if (h.lowercase().startsWith("authorization:")) auth = h.substringAfter(':').trim()
                }
                if (length > 0) {
                    val b = ByteArray(length)
                    var got = 0
                    while (got < length) {
                        val n = input.read(b, got, length - got)
                        if (n <= 0) break
                        got += n
                    }
                    body = String(b)
                }
                c.getOutputStream().write(
                    (
                        "HTTP/1.1 $status\r\ncontent-type: application/json\r\n" +
                            "content-length: ${answer.toByteArray().size}\r\nconnection: close\r\n\r\n$answer"
                        ).toByteArray(),
                )
            }
        }

        private fun line(s: java.io.InputStream): String {
            val b = StringBuilder()
            while (true) {
                val c = s.read()
                if (c < 0 || c == '\n'.code) break
                if (c != '\r'.code) b.append(c.toChar())
            }
            return b.toString()
        }
    }

    @Test
    fun the_token_is_sixty_four_hex_digits_and_fresh() {
        val a = Launch.token()
        val b = Launch.token()
        assertEquals(64, a.length)
        assertTrue(a.all { it in "0123456789abcdef" })
        assertTrue(a != b)
    }

    @Test
    fun the_transport_carries_the_request_and_answers_with_status_and_body() {
        val api = Api("""{"droplet":{"id":1}}""", "202 Accepted")
        api.serve()
        val t = Launch.HttpTransport(api.base)
        val e = t.exchange(
            "POST",
            "https://api.digitalocean.com/v2/droplets",
            listOf(Header("Authorization", "Bearer tok"), Header("Content-Type", "application/json")),
            """{"name":"x"}""",
        )
        assertEquals(202.toUShort(), e.status)
        assertEquals("""{"droplet":{"id":1}}""", e.body)
        assertTrue(api.asked.startsWith("POST /v2/droplets "))
        assertEquals("Bearer tok", api.auth)
        assertEquals("""{"name":"x"}""", api.body)
    }

    @Test
    fun a_refusal_keeps_the_providers_body_and_a_dead_host_answers_zero() {
        val api = Api("""{"id":"unauthorized","message":"Unable to authenticate you"}""", "401 Unauthorized")
        api.serve()
        val e = Launch.HttpTransport(api.base).exchange("GET", "https://api.digitalocean.com/v2/regions", listOf(), null)
        assertEquals(401.toUShort(), e.status)
        assertTrue(e.body.contains("Unable to authenticate you"))
        val dead = Launch.HttpTransport("http://127.0.0.1:1").exchange("GET", "https://api.digitalocean.com/v2/regions", listOf(), null)
        assertEquals(0.toUShort(), dead.status)
        assertTrue(dead.body.isNotEmpty())
    }
}
