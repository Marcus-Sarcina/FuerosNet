package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **What the frame's one proxy reads and answers** ([Carrier]), on the
 * JVM: what is read of a request, what is refused before it is read, and
 * what of an answer reaches the engine.
 */
class CarrierTest {
    private val host = "${"a".repeat(32)}.${"a".repeat(32)}.localhost"

    private fun text(message: ByteArray) = String(message, Charsets.ISO_8859_1)

    @Test
    fun a_request_is_read_only_as_a_plain_one_to_a_named_host() {
        val got = Carrier.read(
            "POST http://$host/submit?x=1 HTTP/1.1\r\nHost: $host\r\nContent-Type: application/json\r\nContent-Length: 12",
        )!!
        assertEquals("POST", got.method)
        assertEquals(host, got.host)
        assertEquals("/submit?x=1", got.path)
        assertEquals("application/json", got.type)
        assertEquals(12, got.length)
        // the origin's own port is its default and no other
        assertEquals(host, Carrier.read("GET http://$host:80/ HTTP/1.1")!!.host)
        assertEquals("/", Carrier.read("GET http://$host HTTP/1.1")!!.path)
        // a tunnel, another scheme, another port, a user, a body of no
        // stated length, a length past one message: none is read
        for (head in listOf(
            "CONNECT example.invalid:443 HTTP/1.1",
            "GET https://$host/ HTTP/1.1",
            "GET /only-a-path HTTP/1.1",
            "GET http://$host:8080/ HTTP/1.1",
            "GET http://user@$host/ HTTP/1.1",
            "POST http://$host/ HTTP/1.1\r\nTransfer-Encoding: chunked",
            "POST http://$host/ HTTP/1.1\r\nContent-Length: ${(16 shl 20) + 1}",
            "POST http://$host/ HTTP/1.1\r\nContent-Length: -1",
            "GET http://$host/",
        )) {
            assertNull(head, Carrier.read(head))
        }
    }

    @Test
    fun the_secret_is_this_processs_own_and_is_what_a_request_must_present() {
        assertEquals(48, Carrier.secret.length)
        assertTrue(Carrier.credential().startsWith("Basic "))
        val head = "GET http://$host/ HTTP/1.1\r\nProxy-Authorization: ${Carrier.credential()}"
        assertEquals(Carrier.credential(), Carrier.presented(head))
        assertNull(Carrier.presented("GET http://$host/ HTTP/1.1\r\nAuthorization: ${Carrier.credential()}"))
        assertNotEquals(Carrier.credential(), Carrier.presented("GET http://$host/ HTTP/1.1\r\nProxy-Authorization: Basic a2VybmVsOg=="))
        assertTrue(text(Carrier.challenge()).startsWith("HTTP/1.1 407 "))
        assertTrue(text(Carrier.challenge()).contains("proxy-authenticate: Basic realm="))
        assertTrue(text(Carrier.elsewhere()).startsWith("HTTP/1.1 403 "))
    }

    /** One exchange with the proxy as it listens: what was sent, and its answer's first line. */
    private fun exchange(request: String): String {
        java.net.Socket("127.0.0.1", Carrier.port()).use { socket ->
            socket.soTimeout = 5_000
            socket.getOutputStream().write(request.toByteArray(Charsets.ISO_8859_1))
            return String(socket.getInputStream().readBytes(), Charsets.ISO_8859_1)
        }
    }

    /**
     * **The proxy as the engine meets it**: nothing without the secret,
     * nothing for a host that is no framed resource's, no tunnel, and a
     * framed resource's own page with it.
     */
    // acceptance: RSC-58
    // functional: CAT-031
    @Test
    fun the_proxy_answers_a_framed_resource_to_the_holder_of_the_secret_and_nobody_else() {
        val resource = "c".repeat(64)
        val origin = Frames.host(resource)
        val page = Gateway.Page(200, "text/html", "utf-8", Gateway.Shape.PAGE, "<p>framed</p>".toByteArray())
        Carrier.serve(Carrier.Serving(resource, "nobody", "node", "/" to page))
        val secret = "Proxy-Authorization: ${Carrier.credential()}\r\n"
        try {
            // another app on the device: no secret, and the page stays in hand
            assertTrue(exchange("GET http://$origin/ HTTP/1.1\r\nHost: $origin\r\n\r\n").startsWith("HTTP/1.1 407 "))
            assertTrue(exchange("GET http://$origin/ HTTP/1.1\r\nProxy-Authorization: Basic a2VybmVsOng=\r\n\r\n").startsWith("HTTP/1.1 407 "))
            // the engine, asked by a page for anywhere else
            assertTrue(exchange("GET http://example.invalid/ HTTP/1.1\r\n$secret\r\n").startsWith("HTTP/1.1 403 "))
            assertTrue(exchange("GET http://${Frames.host("d".repeat(64))}/ HTTP/1.1\r\n$secret\r\n").startsWith("HTTP/1.1 403 "))
            assertTrue(exchange("GET http://127.0.0.1:8080/ HTTP/1.1\r\n$secret\r\n").startsWith("HTTP/1.1 403 "))
            assertTrue(exchange("CONNECT example.invalid:443 HTTP/1.1\r\n$secret\r\n").startsWith("HTTP/1.1 403 "))
            // a path that is the node's, and a method no frame carries
            assertTrue(exchange("GET http://$origin/.rhtn/relationships HTTP/1.1\r\n$secret\r\n").startsWith("HTTP/1.1 502 "))
            assertTrue(exchange("TRACE http://$origin/ HTTP/1.1\r\n$secret\r\n").startsWith("HTTP/1.1 502 "))
            // and the framed resource's own page, once, to the engine
            val served = exchange("GET http://$origin/ HTTP/1.1\r\n$secret\r\n")
            assertTrue(served, served.startsWith("HTTP/1.1 200 OK\r\n"))
            assertTrue(served.endsWith("<p>framed</p>"))
        } finally {
            Carrier.stop(resource)
        }
        // once its screen is gone, its origin is nobody's
        assertTrue(exchange("GET http://$origin/ HTTP/1.1\r\n$secret\r\n").startsWith("HTTP/1.1 403 "))
    }

    @Test
    fun what_a_frame_carries_is_five_methods_on_a_rooted_path_not_the_nodes() {
        for (m in listOf("GET", "POST", "PUT", "PATCH", "DELETE")) assertNull(Carrier.refused(m, "/a"))
        for (m in listOf("HEAD", "OPTIONS", "TRACE", "CONNECT")) assertTrue(Carrier.refused(m, "/a")!!.contains("method"))
        assertTrue(Carrier.refused("GET", "/.rhtn/relationships")!!.contains("/.rhtn/"))
        assertTrue(Carrier.refused("GET", "relative")!!.contains("rooted"))
    }

    @Test
    fun of_an_answer_the_engine_is_given_its_status_type_policy_and_body() {
        val page = Gateway.Page(
            httpStatus = 200,
            media = "text/html",
            charset = "utf-8",
            shape = Gateway.Shape.PAGE,
            body = "<p>hi</p>".toByteArray(),
            policy = "script-src 'self'",
            runsScript = true,
        )
        val said = text(Carrier.page(page))
        assertTrue(said.startsWith("HTTP/1.1 200 OK\r\n"))
        assertTrue(said.contains("content-type: text/html; charset=utf-8\r\n"))
        assertTrue(said.contains("content-security-policy: script-src 'self'\r\n"))
        assertTrue(said.contains("x-content-type-options: nosniff\r\n"))
        assertTrue(said.contains("content-length: 9\r\n"))
        assertTrue(said.endsWith("\r\n\r\n<p>hi</p>"))
        // no cookie, no redirect, nothing else of a browser's
        assertEquals(
            setOf("content-type", "x-content-type-options", "content-security-policy", "content-length", "cache-control", "connection"),
            said.substringBefore("\r\n\r\n").lines().drop(1).map { it.substringBefore(":") }.toSet(),
        )
        // text that declares no charset is said to be UTF-8, and nothing else is
        assertTrue(text(Carrier.page(page.copy(charset = null))).contains("content-type: text/html; charset=utf-8\r\n"))
        assertTrue(text(Carrier.page(page.copy(media = "video/mp4", charset = null))).contains("content-type: video/mp4\r\n"))
        // an undeclared type is bytes, marked, with nothing guessed
        val bare = text(Carrier.page(page.copy(media = null, charset = null, policy = null)))
        assertTrue(bare.contains("content-type: application/octet-stream\r\n"))
        assertTrue(bare.contains("${Carrier.UNDECLARED}: 1\r\n"))
        // a redirect is handed over as the answer it carried
        assertTrue(text(Carrier.page(page.copy(httpStatus = 302))).startsWith("HTTP/1.1 200 "))
        // a policy cannot carry a second header in with it
        assertTrue(text(Carrier.page(page.copy(policy = "a\r\nset-cookie: x=1"))).contains("content-security-policy: a  set-cookie: x=1\r\n"))
    }

    @Test
    fun an_answer_that_carried_nothing_says_why_where_the_pages_object_reads_it() {
        val said = text(Carrier.answered(null))
        assertTrue(said.startsWith("HTTP/1.1 502 "))
        assertTrue(said.contains("${Carrier.CARRIED}: the kernel did not carry the request\r\n"))
        assertEquals("a b:c%2Fd%C3%A9", Carrier.escape("a b:c/dé"))
    }
}
