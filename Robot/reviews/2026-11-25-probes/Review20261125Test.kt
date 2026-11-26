package com.comptus.fueros

import org.junit.Assert.assertTrue
import org.junit.Test

/** Proxy-boundary counterexamples; these do not run a GeckoView or a device. */
class Review20261125Test {
    private fun page(body: String) = Gateway.Page(
        200, "text/html", "utf-8", Gateway.Shape.PAGE, body.toByteArray(),
    )

    private fun exchange(resource: String, origin: String): String {
        java.net.Socket("127.0.0.1", Carrier.port()).use { socket ->
            socket.soTimeout = 5_000
            val request = "GET http://${Frames.host(resource)}/ HTTP/1.1\r\n" +
                "Proxy-Authorization: ${Carrier.credential()}\r\n" +
                "Origin: ${Frames.origin(origin)}\r\n" +
                "Sec-Fetch-Site: cross-site\r\n\r\n"
            socket.getOutputStream().write(request.toByteArray())
            return String(socket.getInputStream().readBytes())
        }
    }

    @Test
    fun another_registered_resource_is_not_this_pages_resource() {
        val a = "a".repeat(64)
        val b = "b".repeat(64)
        Carrier.serve(Carrier.Serving(a, "identity-a", "node-a", "/" to page("A")))
        Carrier.serve(Carrier.Serving(b, "identity-b", "node-b", "/" to page("B")))
        try {
            val got = exchange(b, a)
            assertTrue("cross-origin request returned: $got", got.startsWith("HTTP/1.1 403 "))
        } finally {
            Carrier.stop(a)
            Carrier.stop(b)
        }
    }

    @Test
    fun opening_the_same_resource_as_another_identity_does_not_replace_the_first_frame() {
        val resource = "c".repeat(64)
        Carrier.serve(Carrier.Serving(resource, "identity-a", "node-a", "/" to page("A's page")))
        Carrier.serve(Carrier.Serving(resource, "identity-b", "node-b", "/" to page("B's page")))
        try {
            val got = exchange(resource, resource)
            assertTrue("first frame instead received: $got", got.endsWith("A's page"))
        } finally {
            Carrier.stop(resource)
        }
    }
}
