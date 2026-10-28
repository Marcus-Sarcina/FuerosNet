package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
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
        shape: Kernel.Shape,
        body: String,
        media: String? = "text/html",
    ) = Kernel.Page(
        httpStatus = 200,
        media = media,
        charset = "utf-8",
        shape = shape,
        body = body.toByteArray(),
    )

    private fun answered(
        status: ULong = 0UL,
        page: Kernel.Page? = null,
        unreadable: String? = null,
    ) = Kernel.Answered(status, page, unreadable)

    // ---- what is shown, and what is not -------------------------------

    @Test
    fun html_is_framed_text_is_not_and_everything_else_is_named() {
        val html = ResourcePage.showing(answered(page = page(Kernel.Shape.PAGE, "<p>hi</p>")))
        assertTrue(html.toString(), html is ResourcePage.Showing.Page)

        val text = ResourcePage.showing(
            answered(page = page(Kernel.Shape.TEXT, "the yard tap", "text/plain")),
        )
        assertEquals(
            "text is not a page and takes no frame",
            ResourcePage.Showing.Text("the yard tap"),
            text,
        )

        val other = ResourcePage.showing(
            answered(page = page(Kernel.Shape.OTHER, "%PDF", "application/pdf")),
        )
        val said = (other as ResourcePage.Showing.Said).why
        assertTrue(said, said.contains("application/pdf") && said.contains("4 bytes"))
        assertTrue("it says it is not guessing: $said", said.contains("not guessed at"))
    }

    @Test
    fun an_undeclared_body_is_named_as_declaring_nothing_rather_than_sniffed() {
        val other = ResourcePage.showing(
            answered(page = page(Kernel.Shape.OTHER, "<p>hi</p>", media = null)),
        )
        val said = (other as ResourcePage.Showing.Said).why
        assertTrue("the absence is named: $said", said.contains("with nothing,"))
        assertFalse(
            "markup that declared nothing is not framed as a page: $said",
            said.contains("<p>"),
        )
    }

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
        assertTrue(ResourcePage.FRAME_SAYS, ResourcePage.FRAME_SAYS.contains("no script"))
        assertTrue(ResourcePage.FRAME_SAYS, ResourcePage.FRAME_SAYS.contains("no network"))
        assertTrue(ResourcePage.FRAME_SAYS, ResourcePage.FRAME_SAYS.contains("no storage"))
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
        val src = source("ResourceActivity.kt")
        for (setting in listOf(
            "javaScriptEnabled = false",
            "domStorageEnabled = false",
            "databaseEnabled = false",
            "allowFileAccess = false",
            "allowContentAccess = false",
            "allowFileAccessFromFileURLs = false",
            "allowUniversalAccessFromFileURLs = false",
            "blockNetworkLoads = true",
            "blockNetworkImage = true",
            "setGeolocationEnabled(false)",
        )) {
            assertTrue("the frame does not set $setting", src.contains(setting))
        }
        // the null base is the isolation, so it is asserted as text: a
        // base URL here would give the document an origin to share
        assertTrue(
            "the page must load with a null base, which is its opaque origin",
            src.contains("loadDataWithBaseURL(null,"),
        )
        // and nothing is published into the page
        assertFalse("a bridge is the one way in", src.contains("addJavascriptInterface"))
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

    /** And neither of them enables script, whatever else they differ on. */
    @Test
    fun neither_frame_runs_somebody_elses_script() {
        for (name in listOf("NodePageActivity.kt", "ResourceActivity.kt")) {
            val src = source(name)
            assertTrue("$name does not refuse script", src.contains("javaScriptEnabled = false"))
            assertFalse("$name enables script", src.contains("javaScriptEnabled = true"))
        }
    }
}
