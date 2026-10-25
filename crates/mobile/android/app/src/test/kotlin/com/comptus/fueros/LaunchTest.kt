package com.comptus.fueros

import java.net.ServerSocket
import kotlin.concurrent.thread
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **The launch flow's half that is this shell's** ([Launch]): the token,
 * the configuration and the script it composes, and the DigitalOcean
 * exchange against an ordinary socket standing in for the provider.
 */
class LaunchTest {

    /** A provider that answers one request and says what it was asked. */
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
    fun the_configuration_and_the_script_carry_what_the_instance_needs_and_no_seed() {
        val token = "ab".repeat(32)
        val conf = Launch.nodeConfig(token)
        assertTrue(conf.contains("token = \"$token\""))
        assertTrue(conf.contains("listen = \"0.0.0.0:7431\""))
        assertTrue(conf.contains("listen = \"0.0.0.0:7449\""))
        assertTrue(conf.contains("operator = \"/var/lib/rhtn/operator.material\""))
        val script = Launch.userData("82a301", token, registry = "reg.example/x", tag = "abc123")
        assertTrue(script.startsWith("#cloud-config\n"))
        assertTrue(script.contains("  - podman\n"))
        assertTrue(script.contains("      token = \"$token\""))
        assertTrue(script.contains("      82a301\n"))
        assertTrue(script.contains("chown -R 10001:10001 /var/lib/rhtn"))
        assertTrue(script.contains("--network=host -v /var/lib/rhtn:/var/lib/rhtn reg.example/x/rhtnd:abc123"))
        // what a seed would look like is nowhere in it: only the public material
        assertTrue(!script.contains("seed"))
    }

    @Test
    fun a_launch_posts_the_droplet_and_reads_its_id() {
        val api = Api("""{"droplet":{"id":123456,"name":"rhtn-1","status":"new"}}""", "202 Accepted")
        api.serve()
        val got = Launch.DigitalOcean(api.base).launch("tok", Launch.Plan("nyc3", "s-1vcpu-1gb"), "rhtn-1", "#cloud-config\n")
        assertEquals("123456", got.getOrThrow().id)
        assertTrue(api.asked.startsWith("POST /v2/droplets "))
        assertEquals("Bearer tok", api.auth)
        val sent = Json.obj(Json.parse(api.body))
        assertEquals("nyc3", Json.string(sent["region"]))
        assertEquals("s-1vcpu-1gb", Json.string(sent["size"]))
        assertEquals("debian-12-x64", Json.string(sent["image"]))
        assertEquals("#cloud-config\n", Json.string(sent["user_data"]))
    }

    @Test
    fun the_standing_reads_the_public_address_once_active() {
        val api = Api(
            """{"droplet":{"id":1,"status":"active","networks":{"v4":[
            {"ip_address":"10.0.0.2","type":"private"},{"ip_address":"203.0.113.9","type":"public"}],"v6":[]}}}""",
        )
        api.serve()
        val s = Launch.DigitalOcean(api.base).standing("tok", "1").getOrThrow()
        assertEquals("active", s.status)
        assertEquals("203.0.113.9", s.ipv4)
        assertTrue(api.asked.startsWith("GET /v2/droplets/1 "))
    }

    @Test
    fun regions_and_sizes_parse_and_a_refusal_carries_the_providers_words() {
        val r = Api("""{"regions":[{"slug":"nyc3","name":"New York 3","available":true},{"slug":"x","name":"gone","available":false}]}""")
        r.serve()
        val regions = Launch.DigitalOcean(r.base).regions("tok").getOrThrow()
        assertEquals(listOf("nyc3", "x"), regions.map { it.slug })
        val s = Api("""{"sizes":[{"slug":"s-1vcpu-1gb","memory":1024,"vcpus":1,"price_monthly":6.0,"regions":["nyc3"],"available":true}]}""")
        s.serve()
        val sizes = Launch.DigitalOcean(s.base).sizes("tok").getOrThrow()
        assertEquals(1, sizes.size)
        assertEquals(1024, sizes[0].memoryMb)
        val no = Api("""{"id":"unauthorized","message":"Unable to authenticate you"}""", "401 Unauthorized")
        no.serve()
        val e = Launch.DigitalOcean(no.base).regions("bad").exceptionOrNull()
        assertTrue(e != null && e.message!!.contains("401") && e.message!!.contains("Unable to authenticate you"))
    }
}
