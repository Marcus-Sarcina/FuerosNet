package com.comptus.fueros

import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.security.SecureRandom
import java.util.concurrent.ConcurrentHashMap

/**
 * **The one place a resource frame's engine can send anything**
 * (`light-client-requirements.md` §8) [author, 2026-11-25].
 *
 * The engine ([Engine]) is given one proxy for everything it would fetch,
 * and this is it: a listener on this device's own loopback that takes a
 * request for a framed resource's origin, carries it to that resource
 * through the kernel and the node that serves it, and answers every other
 * request with a refusal. So a page reaches its own resource and nothing
 * else by construction, whichever way its markup or its code asks — there
 * is no second route for the engine to take.
 *
 * **Only this app's engine may ask.** A port on the loopback is open to
 * every app on the device, and a request here is answered as the member.
 * So each carries a secret minted when this process starts, which the
 * engine is told when the proxy challenges it ([Engine]) and nothing else
 * on the device holds: a request without it is refused before it is read
 * further.
 *
 * **What reaches the engine of an answer** is what the desktop's frame is
 * given (`rhtn_desktop::frame::served`): the status, the declared type and
 * charset, the page's own policy and the body. Every other header stops
 * here.
 */
object Carrier {

    /** Whom a framed resource is asked as, and through which node. */
    class Serving(
        val resource: String,
        val by: String?,
        val node: String,
        /** The page already in hand, served to the first load of its path. */
        @Volatile var first: Pair<String, Gateway.Page>?,
        /** **Where an ask goes**: an answer to a page's own request that
         *  is a question for the person is the screen's to put, and never
         *  the page's to read (`light-client-requirements.md` §8.1). */
        val asked: ((Gateway.Answered) -> Unit)? = null,
    )

    /** The methods a frame carries, as the desktop's does. */
    val METHODS: List<String> = listOf("GET", "POST", "PUT", "PATCH", "DELETE")

    /** The header naming why the kernel carried nothing, as `rhtn.request` reads it. */
    const val CARRIED: String = "rhtn-carried"

    /** The header marking an answer whose type was not declared. */
    const val UNDECLARED: String = "rhtn-undeclared"

    /** The name the engine presents the secret under. */
    const val USER: String = "kernel"

    /** The most one request's head and body may be, the message bound of
     *  `wire-format.md` §11. */
    private const val MOST: Int = 16 shl 20

    private val serving = ConcurrentHashMap<String, Serving>()

    /** Minted once a process, and told to nothing but this app's engine. */
    val secret: String by lazy {
        ByteArray(24).also { SecureRandom().nextBytes(it) }.joinToString("") { "%02x".format(it) }
    }

    @Volatile
    private var bound: Int = 0

    /** Where the proxy listens, started at first need. */
    @Synchronized
    fun port(): Int {
        if (bound != 0) return bound
        val server = ServerSocket(0, 64, InetAddress.getByName("127.0.0.1"))
        bound = server.localPort
        Thread({
            while (true) {
                val socket = runCatching { server.accept() }.getOrNull() ?: return@Thread
                Thread({ runCatching { one(socket) }; runCatching { socket.close() } }, "fueros-carried").start()
            }
        }, "fueros-carrier").start()
        return bound
    }

    /** Begin answering a resource's origin, as the identity and node its screen named. */
    fun serve(s: Serving) {
        serving[Frames.host(s.resource)] = s
    }

    /** Stop answering it: the screen that framed it is gone. */
    fun stop(resource: String) {
        serving.remove(Frames.host(resource))
    }

    /** One request as this proxy reads it. */
    data class Asked(
        val method: String,
        /** The host named, lower-cased, with no port. */
        val host: String,
        val path: String,
        val type: String?,
        val length: Int,
    )

    /** What the engine presents when it holds the secret. */
    fun credential(): String =
        "Basic " + java.util.Base64.getEncoder().encodeToString("$USER:$secret".toByteArray())

    /**
     * **Read a request's head**, or nothing where it is not one this proxy
     * takes: a method, an absolute `http://host/path` target, and the
     * headers that say how long its body is, what type it declares and
     * what it presents. A tunnel (`CONNECT`) and any other scheme are not
     * read at all.
     */
    fun read(head: String): Asked? {
        val lines = head.split("\r\n")
        val first = lines.firstOrNull()?.split(" ") ?: return null
        if (first.size != 3) return null
        val target = first[1]
        if (!target.startsWith("http://")) return null
        val rest = target.removePrefix("http://")
        val slash = rest.indexOf('/')
        val authority = (if (slash < 0) rest else rest.substring(0, slash)).lowercase()
        val path = if (slash < 0) "/" else rest.substring(slash)
        val host = authority.removeSuffix(":80")
        if (host.isEmpty() || host.contains(':') || host.contains('@')) return null
        var type: String? = null
        var length = 0
        for (line in lines.drop(1)) {
            val colon = line.indexOf(':')
            if (colon <= 0) continue
            val value = line.substring(colon + 1).trim()
            when (line.substring(0, colon).trim().lowercase()) {
                "content-type" -> type = value
                "content-length" -> length = value.toIntOrNull()?.takeIf { it in 0..MOST } ?: return null
                // a body whose length is not said is not one this reads
                "transfer-encoding" -> return null
            }
        }
        return Asked(first[0].uppercase(), host, path, type, length)
    }

    /** What a request's head presents to the proxy, whatever else it is. */
    fun presented(head: String): String? = head.split("\r\n").drop(1).firstNotNullOfOrNull { line ->
        val colon = line.indexOf(':')
        if (colon > 0 && line.substring(0, colon).trim().equals("proxy-authorization", ignoreCase = true)) {
            line.substring(colon + 1).trim()
        } else {
            null
        }
    }

    /** Why a request is not carried, or nothing where it is. */
    fun refused(method: String, path: String): String? = when {
        method !in METHODS -> "a method this frame does not carry: $method"
        !path.startsWith("/") || path.startsWith("/.rhtn/") ->
            "a path is rooted on this resource and not under /.rhtn/"
        else -> null
    }

    /** Percent-encoding for a header value, as `decodeURIComponent` reads it. */
    fun escape(s: String): String = s.toByteArray(Charsets.UTF_8).joinToString("") { b ->
        val c = b.toInt().toChar()
        if (c in 'A'..'Z' || c in 'a'..'z' || c in '0'..'9' || c in "-_.~ :") c.toString() else "%%%02X".format(b)
    }

    private fun message(status: Int, headers: List<Pair<String, String>>, body: ByteArray): ByteArray {
        val head = StringBuilder("HTTP/1.1 $status ${if (status in 200..299) "OK" else "Answered"}\r\n")
        for ((name, value) in headers) head.append(name).append(": ").append(value).append("\r\n")
        head.append("content-length: ${body.size}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n")
        return head.toString().toByteArray(Charsets.ISO_8859_1) + body
    }

    /** **An answer that carried nothing**, said as text and named in [CARRIED]. */
    fun notCarried(why: String): ByteArray = message(
        502,
        listOf(
            "content-type" to "text/plain; charset=utf-8",
            "x-content-type-options" to "nosniff",
            CARRIED to escape(why),
        ),
        "$why\n".toByteArray(Charsets.UTF_8),
    )

    /** **The proxy's own refusals**: no secret, or nothing this proxy answers for. */
    fun challenge(): ByteArray = message(
        407,
        listOf("proxy-authenticate" to "Basic realm=\"kernel\""),
        ByteArray(0),
    )

    /** What a request for anything but a framed resource gets. */
    fun elsewhere(): ByteArray = message(403, emptyList(), ByteArray(0))

    /** **One page as the engine is given it**: its declared type and
     *  nothing guessed, its own policy, and its body. */
    fun page(p: Gateway.Page): ByteArray {
        val headers = mutableListOf<Pair<String, String>>()
        if (p.media != null) {
            // text with no charset declared is read as UTF-8, as this app
            // has always handed it to an engine: left unsaid, the engine
            // would guess an older encoding [found on GXV, 2026-11-25]
            val charset = p.charset ?: "utf-8".takeIf { p.media.startsWith("text/") }
            headers += "content-type" to (charset?.let { "${p.media}; charset=$it" } ?: p.media)
        } else {
            headers += "content-type" to "application/octet-stream"
            headers += UNDECLARED to "1"
        }
        headers += "x-content-type-options" to "nosniff"
        p.policy?.let { headers += "content-security-policy" to it.replace("\r", " ").replace("\n", " ") }
        return message(Frames.engineStatus(p.httpStatus), headers, p.body)
    }

    /** **Whether an answer is an ask**: delivered, read, and a question
     *  for the person in place of a page. */
    fun asks(a: Gateway.Answered?): Boolean =
        a != null && a.status.toInt() == 0 && a.unreadable == null && a.ask != null

    /** **What a page gets of a request answered with an ask**: nothing.
     *  The question is the person's, and what follows it is the screen's
     *  to show. */
    fun asking(): ByteArray = message(204, emptyList(), ByteArray(0))

    /** What the kernel's answer comes to, for the engine. */
    fun answered(a: Gateway.Answered?): ByteArray {
        if (a == null) return notCarried("the kernel did not carry the request")
        if (a.status.toInt() != 0) return notCarried("not delivered: ${ResourcePage.delivery(a.status)}")
        a.unreadable?.let { return notCarried("the answer could not be read: $it") }
        val p = a.page ?: return notCarried("the resource answered with nothing")
        return page(p)
    }

    private fun one(socket: Socket) {
        socket.soTimeout = 30_000
        val input = socket.getInputStream()
        val out = socket.getOutputStream()
        val head = head(input) ?: return
        // **the secret first**: without it nothing else of the request is
        // acted on, whoever on this device sent it
        if (presented(head) != credential()) {
            out.write(challenge())
            return
        }
        // a tunnel, another scheme, a body of no stated length: not read
        val asked = read(head)
        if (asked == null) {
            out.write(elsewhere())
            return
        }
        val s = serving[asked.host]
        if (s == null) {
            out.write(elsewhere())
            return
        }
        val body = ByteArray(asked.length)
        var got = 0
        while (got < body.size) {
            val n = input.read(body, got, body.size - got)
            if (n < 0) return
            got += n
        }
        refused(asked.method, asked.path)?.let {
            out.write(notCarried(it))
            return
        }
        if (asked.method == "GET") {
            val first = s.first
            if (first != null && first.first == asked.path) {
                s.first = null
                out.write(page(first.second))
                return
            }
        }
        val who = s.by ?: Identities.activeId
        if (who == null) {
            out.write(notCarried("nobody is acting"))
            return
        }
        val carried = Gateway.requestAs(
            who, s.node, s.resource, asked.method, asked.path,
            asked.type.takeIf { asked.length > 0 },
            body.takeIf { asked.length > 0 },
        )
        // **an ask is put by the screen that framed the page**, whichever
        // request of the page's it answered; with no screen to put it,
        // the page is told nothing came, as before
        val put = s.asked
        if (asks(carried) && put != null) {
            out.write(asking())
            carried?.let(put)
            return
        }
        out.write(answered(carried))
    }

    /** A request's head, up to and without its blank line, or nothing. */
    private fun head(input: InputStream): String? {
        val bytes = ByteArrayOutputStream()
        var run = 0
        while (bytes.size() < 65_536) {
            val c = input.read()
            if (c < 0) return null
            bytes.write(c)
            run = if (c == '\r'.code || c == '\n'.code) run + 1 else 0
            if (run == 4) return bytes.toString("ISO-8859-1").removeSuffix("\r\n\r\n")
        }
        return null
    }
}
