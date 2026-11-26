package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **The resource page's frame policy** (the screens sheet's F;
 * `light-client-requirements.md` §8, settled 2026-10-28).
 *
 * Two halves. The decision — what is rendered, what is named instead,
 * and what the person is told — is [ResourcePage]'s and is asserted
 * directly. The frame's own refusals cannot be exercised on the JVM, so
 * they are read out of the source, as [ManifestTest] and [InsetSweepTest]
 * read theirs: that says nothing about whether a web view honours them,
 * and everything about whether this shell asked.
 */
class ResourcePageTest {

    private fun page(
        shape: Gateway.Shape,
        body: String,
        media: String? = "text/html",
    ) = Gateway.Page(
        httpStatus = 200,
        media = media,
        charset = "utf-8",
        shape = shape,
        body = body.toByteArray(),
    )

    private fun answered(
        status: ULong = 0UL,
        page: Gateway.Page? = null,
        unreadable: String? = null,
    ) = Gateway.Answered(status, page, unreadable)

    // ---- what is shown, and what is not -------------------------------

    @Test
    fun html_is_framed_text_is_not_and_everything_else_is_named() {
        val html = ResourcePage.showing(answered(page = page(Gateway.Shape.PAGE, "<p>hi</p>")))
        assertTrue(html.toString(), html is ResourcePage.Showing.Page)

        val text = ResourcePage.showing(
            answered(page = page(Gateway.Shape.TEXT, "the yard tap", "text/plain")),
        )
        assertEquals(
            "text is not a page and takes no frame",
            ResourcePage.Showing.Text("the yard tap"),
            text,
        )

        val other = ResourcePage.showing(
            answered(page = page(Gateway.Shape.OTHER, "%PDF", "application/pdf")),
        )
        val said = (other as ResourcePage.Showing.Said).why
        assertTrue(said, said.contains("application/pdf") && said.contains("4 bytes"))
        assertTrue("it says it is not guessing: $said", said.contains("not guessed at"))
    }

    @Test
    fun an_undeclared_body_is_named_as_declaring_nothing_rather_than_sniffed() {
        val other = ResourcePage.showing(
            answered(page = page(Gateway.Shape.OTHER, "<p>hi</p>", media = null)),
        )
        val said = (other as ResourcePage.Showing.Said).why
        assertTrue("the absence is named: $said", said.contains("with nothing,"))
        assertFalse(
            "markup that declared nothing is not framed as a page: $said",
            said.contains("<p>"),
        )
    }

    // functional: UX-018
    @Test
    fun an_answer_that_was_not_delivered_says_which_of_the_six_it_was() {
        for (s in 1..5) {
            val said = (ResourcePage.showing(answered(status = s.toULong()))
                as ResourcePage.Showing.Said).why
            assertTrue(said, said.startsWith("Not delivered: "))
            assertEquals(ResourcePage.delivery(s.toULong()) + ".", said.removePrefix("Not delivered: "))
        }
        // and nothing carried at all is its own answer, not a status
        val none = ResourcePage.showing(null) as ResourcePage.Showing.Said
        assertTrue(none.why, none.why.contains("did not carry"))
    }

    // functional: UX-018
    @Test
    fun a_message_the_kernel_could_not_read_shows_none_of_it() {
        val said = (
            ResourcePage.showing(
                answered(unreadable = "content-length twice", page = null),
            ) as ResourcePage.Showing.Said
            ).why
        assertTrue(said, said.contains("content-length twice"))
        assertTrue(
            "none of it rather than some of it: $said",
            said.contains("Nothing of it is shown"),
        )
    }

    // ---- the styling ruling -------------------------------------------

    /** The author's ruling of 2026-10-27: what the resource has not
     *  styled for itself, the application styles to match. */
    @Test
    fun the_apps_sheet_comes_first_so_the_pages_own_rules_win() {
        val out = ResourcePage.styled("<style>p{color:red}</style><p>hi</p>")
        assertTrue(out, out.indexOf(ResourcePage.SHEET) < out.indexOf("p{color:red}"))
        assertTrue("the resource's markup is not rewritten", out.contains("<p>hi</p>"))
        assertTrue(out, out.startsWith("<!doctype html>"))
    }

    /** A sheet of bare element selectors loses to anything the page sets,
     *  which is what "styles what the resource has not" means. */
    @Test
    fun the_sheet_claims_nothing_a_page_cannot_override() {
        assertFalse("!important would win over the page", ResourcePage.SHEET.contains("!important"))
        for (line in ResourcePage.SHEET.lines().filter { it.isNotBlank() }) {
            val selector = line.substringBefore('{')
            assertFalse("a class selector outranks the page's own: $line", selector.contains('.'))
            assertFalse("an id selector outranks the page's own: $line", selector.contains('#'))
        }
    }

    @Test
    fun a_framed_page_is_told_what_the_frame_is() {
        val still = ResourcePage.frameSays(false)
        assertTrue(still, still.contains("runs no code"))
        assertTrue(still, still.contains("this resource and nothing else"))
        val runs = ResourcePage.frameSays(true)
        assertTrue(runs, runs.contains("runs its code"))
        assertTrue(runs, runs.contains("as you allowed"))
        assertTrue(runs, runs.contains("its own storage"))
    }

    // functional: CAT-027
    @Test
    fun a_page_runs_only_where_it_declared_and_the_person_allowed() {
        val declared = Gateway.Page(200, "text/html", null, Gateway.Shape.PAGE, ByteArray(0), "script-src 'self'", true)
        val plain = Gateway.Page(200, "text/html", null, Gateway.Shape.PAGE, ByteArray(0))
        assertEquals("declared, not yet asked: the question is owed", null, ResourcePage.runs(declared, null))
        assertEquals(true, ResourcePage.runs(declared, true))
        assertEquals(false, ResourcePage.runs(declared, false))
        assertEquals("undeclared, never asked, never runs", false, ResourcePage.runs(plain, null))
        assertEquals("an allowance cannot run a page that declared nothing", false, ResourcePage.runs(plain, true))
        assertTrue(ResourcePage.SCRIPT_ASKS.contains("nothing else"))
    }

    @Test
    fun a_redirect_is_handed_to_the_engine_as_the_answer_it_carried() {
        // the platform's frame throws on any status from 300 to 399
        for (http in listOf(300, 301, 302, 303, 307, 308, 399)) {
            assertEquals("$http is not followed and its body is shown", 200, Frames.engineStatus(http))
        }
        for (http in listOf(100, 200, 204, 299, 400, 404, 500, 599)) {
            assertEquals(http, Frames.engineStatus(http))
        }
        for (http in listOf(0, 99, 600, 65536, -1)) {
            assertEquals("no HTTP status", 502, Frames.engineStatus(http))
        }
    }

    @Test
    fun the_wall_behind_a_frame_is_a_port_of_this_device_that_nothing_may_hold() {
        val (host, port) = Frames.NOWHERE.split(":").let { it[0] to it[1].toInt() }
        assertEquals("127.0.0.1", host)
        // below 1024 is the system's to bind, so no other app listens there
        assertTrue(port in 1..1023)
        assertTrue(Engine.NOWHERE in 1..1023)
    }

    @Test
    fun a_resources_engine_is_told_one_proxy_and_no_peer_connection() {
        val told = Engine.prefs(41234).toMap()
        assertEquals(false, told["media.peerconnection.enabled"])
        assertEquals(false, told["network.webtransport.enabled"])
        // plain requests to this app's own proxy, and every other stream nowhere
        assertEquals(1, told["network.proxy.type"])
        assertEquals("127.0.0.1", told["network.proxy.http"])
        assertEquals(41234, told["network.proxy.http_port"])
        assertEquals(Engine.NOWHERE, told["network.proxy.ssl_port"])
        assertEquals(Engine.NOWHERE, told["network.proxy.socks_port"])
        assertEquals(false, told["network.proxy.failover_direct"])
        // nothing is exempt from the proxy, this device's own loopback included
        assertEquals("", told["network.proxy.no_proxies_on"])
        assertEquals(true, told["network.proxy.allow_hijacking_localhost"])
        // and nothing is fetched ahead of being asked for
        assertEquals(0, told["network.http.speculative-parallel-limit"])
        assertEquals(true, told["network.dns.disablePrefetch"])
        // the file the engine reads before it starts says the same
        val file = Engine.file(41234)
        assertTrue(file.startsWith("prefs:\n"))
        assertTrue(file.contains("  media.peerconnection.enabled: false\n"))
        assertTrue(file.contains("  network.proxy.http: \"127.0.0.1\"\n"))
        assertTrue(file.contains("  network.proxy.http_port: 41234\n"))
        assertEquals(told.size + 1, file.trimEnd().lines().size)
    }

    @Test
    fun a_resources_origin_is_its_own_and_resolves_nowhere() {
        val a = "a".repeat(64)
        val b = "b".repeat(64)
        val (me, other) = "1".repeat(64) to "2".repeat(64)
        // one label under a name that is this device: a digest, the same
        // at every start, which says neither who nor what
        val host = Frames.host(me, a)
        assertEquals("http://$host", Frames.origin(me, a))
        assertTrue(host, Regex("[0-9a-f]{62}\\.localhost").matches(host))
        assertEquals(host, Frames.host(me.uppercase(), a.uppercase()))
        assertTrue(!host.contains(a.take(16)) && !host.contains(me.take(16)))
        // per resource, and per identity [author, 2026-11-26]
        assertNotEquals(Frames.origin(me, a), Frames.origin(me, b))
        assertNotEquals(Frames.origin(me, a), Frames.origin(other, a))
        assertNotEquals(Frames.origin(me, a), Frames.origin(a, me))
        for (bad in listOf("ab", "g".repeat(64))) {
            assertTrue(runCatching { Frames.host(bad, a) }.isFailure)
            assertTrue(runCatching { Frames.host(me, bad) }.isFailure)
        }
        assertEquals(Frames.Going.INSIDE, Frames.resourceGoing(Frames.origin(me, a), Frames.origin(me, a) + "/x?y"))
        assertEquals(Frames.Going.REFUSE, Frames.resourceGoing(Frames.origin(me, a), Frames.origin(me, b) + "/x"))
        assertEquals(Frames.Going.REFUSE, Frames.resourceGoing(Frames.origin(me, a), Frames.origin(other, a) + "/x"))
        assertEquals(Frames.Going.REFUSE, Frames.resourceGoing(Frames.origin(me, a), "https://example.invalid/"))
        assertEquals(Frames.Going.REFUSE, Frames.resourceGoing(Frames.origin(me, a), "file:///etc/hosts"))
        assertEquals(
            "a prefix of the host is another host",
            Frames.Going.REFUSE,
            Frames.resourceGoing(Frames.origin(me, a), Frames.origin(me, a) + ".example.invalid/"),
        )
    }

    // ---- the frame, read out of the source ----------------------------

    private fun module(): File {
        var at: File? = File(".").canonicalFile
        while (at != null) {
            if (File(at, "app/src/main/AndroidManifest.xml").isFile) return at
            at = at.parentFile
        }
        throw AssertionError("no app/src/main/AndroidManifest.xml at or above ${File(".").canonicalPath}")
    }

    private fun source(name: String): String {
        val f = File(module(), "app/src/main/kotlin/com/comptus/fueros/$name")
        assertTrue("no $name under ${module()}", f.isFile)
        return f.readText()
    }

    @Test
    fun the_frame_asks_for_every_refusal_the_policy_names() {
        // **a resource's page is framed in this app's own engine**
        // [author, 2026-11-25], held by [Engine] and answered by [Carrier].
        // Both halves are asserted: the definitions refuse, and the screen
        // reaches for the definitions — either alone would pass while the
        // other drifted.
        val screen = source("ResourceActivity.kt")
        assertTrue("the page loads at the resource's own origin", screen.contains("Frames.origin(who, resource)"))
        assertTrue("its origin is answered by the carrier", screen.contains("Carrier.serve("))
        assertTrue("no frame before the engine holds what it was told", screen.contains("Engine.with(this) { engine ->"))
        assertTrue("the session is the engine's own definition", screen.contains("Engine.session(allowed)"))
        assertTrue("the first load is the origin's own URL", screen.contains("made.loadUri(\"\$origin\$path\")"))
        assertFalse("no document is given no origin", screen.contains("loadData"))
        assertFalse("the screen holds no system web view", screen.contains("android.webkit"))
        // nothing of this app is published into a page: the one object is
        // a script of the engine's, carrying a request and nothing else
        for (f in File(module(), "app/src/main/kotlin").walkTopDown().filter { it.extension == "kt" }) {
            assertFalse("${f.name} publishes into a page", f.readText().contains("addJavascriptInterface("))
        }
        val script = File(module(), "app/src/main/assets/frame/request.js").readText()
        assertTrue("a path under /.rhtn/ is the node's and refused", script.contains("p.startsWith('/.rhtn/')"))
        assertEquals("one function on the one object", 1, Regex("exportFunction\\(").findAll(script).count())
        val manifest = File(module(), "app/src/main/assets/frame/manifest.json").readText()
        assertTrue("it runs before the page's own code", manifest.contains("\"run_at\": \"document_start\""))
        assertTrue("and only on a resource's origin", manifest.contains("\"matches\": [\"http://*.localhost/*\"]"))
        // and the engine is not asked for a frame until it read back as told
        val engine = source("Engine.kt")
        assertTrue(engine.contains("got[\"media.peerconnection.enabled\"] == false"))
        assertTrue(engine.contains("got[\"network.proxy.http_port\"] == proxy"))
    }

    /** No screen framing somebody else's content may be written without
     *  this policy: a second frame is how the two come apart. */
    @Test
    fun only_the_two_known_frames_hold_a_web_view() {
        val holders = File(module(), "app/src/main/kotlin").walkTopDown()
            .filter { it.extension == "kt" }
            .filter { Regex("(?<![A-Za-z.])WebView\\(this\\)").containsMatchIn(it.readText()) }
            .map { it.name }
            .sorted()
            .toList()
        assertEquals(
            "the system's web view frames the node's own page and nothing else",
            listOf("NodePageActivity.kt"),
            holders,
        )
        val engines = File(module(), "app/src/main/kotlin").walkTopDown()
            .filter { it.extension == "kt" }
            .filter { it.readText().contains("GeckoView(this)") }
            .map { it.name }
            .sorted()
            .toList()
        assertEquals(
            "a view of this app's engine in a second place is a second frame policy",
            listOf("ResourceActivity.kt"),
            engines,
        )
    }

    /** A node's frame never runs script; a resource's runs it only as the
     *  screen decided from the declaration and the person's answer. */
    @Test
    fun script_runs_only_in_a_resources_frame_and_only_as_decided() {
        val frames = source("Frames.kt")
        assertEquals(
            "the node's frame refuses script, once",
            1,
            Regex("javaScriptEnabled = false").findAll(frames).count(),
        )
        assertEquals(
            "the resource's frame takes the decision and makes none of its own",
            1,
            Regex("allowJavascript\\(script\\)").findAll(source("Engine.kt")).count(),
        )
        assertFalse("no frame enables script on its own", frames.contains("javaScriptEnabled = true"))
        assertTrue(
            "the screen decides from the declaration and the answer",
            source("ResourceActivity.kt").contains("ResourcePage.runs(showing.page, Frames.scriptAllowed(this, resource))"),
        )
        // and each screen reaches for its own frame rather than setting
        // anything of its own
        assertTrue(
            "NodePageActivity does not frame through Frames.node",
            source("NodePageActivity.kt").contains("Frames.node(this)"),
        )
        assertTrue(
            "ResourceActivity does not frame through the engine's own session",
            source("ResourceActivity.kt").contains("Engine.session(allowed)"),
        )
        for (name in listOf("NodePageActivity.kt", "ResourceActivity.kt")) {
            assertFalse(
                "$name sets a web setting of its own, outside the frame it shares",
                source(name).contains("settings.apply"),
            )
        }
    }
}
