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
    fun a_resources_origin_is_its_own_and_resolves_nowhere() {
        val a = "a".repeat(64)
        val b = "b".repeat(64)
        assertEquals("https://${"a".repeat(32)}.${"a".repeat(32)}.rhtn.invalid", Frames.origin(a))
        assertNotEquals(Frames.origin(a), Frames.origin(b))
        assertEquals(Frames.Going.INSIDE, Frames.resourceGoing(Frames.origin(a), Frames.origin(a) + "/x?y"))
        assertEquals(Frames.Going.REFUSE, Frames.resourceGoing(Frames.origin(a), Frames.origin(b) + "/x"))
        assertEquals(Frames.Going.REFUSE, Frames.resourceGoing(Frames.origin(a), "https://example.invalid/"))
        assertEquals(Frames.Going.REFUSE, Frames.resourceGoing(Frames.origin(a), "file:///etc/hosts"))
        assertEquals(
            "a prefix of the host is another host",
            Frames.Going.REFUSE,
            Frames.resourceGoing(Frames.origin(a), Frames.origin(a) + ".example.invalid/"),
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
        // **the settings moved to [Frames] and the screen calls it**
        // [2026-11-08], so the instrumentation run can apply the same code
        // to a real `WebView` instead of a copy of it. Both halves are
        // asserted: the definition refuses, and the screen reaches for the
        // definition — either alone would pass while the other drifted.
        val src = source("Frames.kt")
        assertTrue(
            "ResourceActivity no longer frames through Frames.resource",
            source("ResourceActivity.kt").contains("Frames.resource(this, script)"),
        )
        for (setting in listOf(
            "javaScriptEnabled = script",
            "domStorageEnabled = script",
            "databaseEnabled = script",
            "allowFileAccess = false",
            "allowContentAccess = false",
            "allowFileAccessFromFileURLs = false",
            "allowUniversalAccessFromFileURLs = false",
            "setGeolocationEnabled(false)",
        )) {
            assertTrue("the frame does not set $setting", src.contains(setting))
        }
        // **the origin is the resource's own, and the page loads at it**
        // [author, 2026-11-15]: served from the kernel's answer by the
        // screen's client, never handed to the engine with no origin
        val screen = source("ResourceActivity.kt")
        assertTrue("the page loads at the resource's own origin", screen.contains("Frames.origin(resource)"))
        assertFalse("no document is given no origin any more", screen.contains("loadDataWithBaseURL("))
        assertTrue("the first load is served from the answer in hand", screen.contains("loadUrl(\"\$origin\$path\")"))
        // and the one object a running page is given is the bridge, in
        // the screen and only behind the person's answer
        assertFalse("Frames publishes nothing into a page", src.contains("addJavascriptInterface"))
        assertTrue(
            "the bridge is given only to a page that runs",
            screen.contains("if (script) {\n            // **the one object**"),
        )
        val bridge = source("ResourceBridge.kt")
        assertTrue("the bridge carries a request and nothing else", bridge.contains("@JavascriptInterface"))
        assertEquals("one method on the one object", 1, Regex("@JavascriptInterface").findAll(bridge).count())
        assertTrue("a path under /.rhtn/ is the node's and refused", bridge.contains("/.rhtn/"))
    }

    /** No screen framing somebody else's content may be written without
     *  this policy: a second frame is how the two come apart. */
    @Test
    fun only_the_two_known_frames_hold_a_web_view() {
        val holders = File(module(), "app/src/main/kotlin").walkTopDown()
            .filter { it.extension == "kt" }
            .filter { it.readText().contains("WebView(this)") }
            .map { it.name }
            .sorted()
            .toList()
        assertEquals(
            "a web view in a third place is a third frame policy",
            listOf("NodePageActivity.kt", "ResourceActivity.kt"),
            holders,
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
            Regex("javaScriptEnabled = script").findAll(frames).count(),
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
            "ResourceActivity does not frame through Frames.resource",
            source("ResourceActivity.kt").contains("Frames.resource(this, script)"),
        )
        for (name in listOf("NodePageActivity.kt", "ResourceActivity.kt")) {
            assertFalse(
                "$name sets a web setting of its own, outside the frame it shares",
                source(name).contains("settings.apply"),
            )
        }
    }
}
