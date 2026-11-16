package com.comptus.fueros

import android.util.Log
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.net.InetAddress
import java.net.ServerSocket
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * **Does a resource's page reach past the frame through a media element?**
 * (`light-client-requirements.md` §8; asked 2026-11-16.)
 *
 * The frame answers every load outside the page's own origin with nothing
 * from `shouldInterceptRequest`. A `<video>` or `<audio>` on Android may
 * fetch without that hook being asked, which would be the one load the
 * frame cannot answer. So a listener on this phone's own loopback stands
 * in for an outside host, a page points media and an image at it, and the
 * listener says what reached it.
 *
 * **A control first**, as the rest of the frame tests do: the same page in
 * a web view with no frame must reach the listener, or a silence under the
 * frame proves nothing.
 *
 * Each case logs what it saw under the tag `egress`, so the run is read
 * from logcat as well as from the assertions.
 *
 * **First run** [MRK, Android 15, WebView 157.0.8103.38, 2026-11-16]: the
 * control reached the listener with the video, the audio, the `<source>`
 * and the image; under the frame every one of them was offered to the
 * client's hook first, answered empty, and the listener heard nothing,
 * with network loads blocked or not; and the origin's own document and
 * image were served through the hook with loads blocked.
 */
@RunWith(AndroidJUnit4::class)
class MediaEgressTest {

    private lateinit var server: ServerSocket
    private val hits: MutableList<String> = Collections.synchronizedList(mutableListOf())
    private var accepting: Thread? = null

    private val origin = Frames.origin("d".repeat(64))

    @Before
    fun listen() {
        server = ServerSocket(0, 50, InetAddress.getByName("127.0.0.1"))
        accepting = Thread {
            while (!server.isClosed) {
                val c = runCatching { server.accept() }.getOrNull() ?: break
                runCatching {
                    c.soTimeout = 2_000
                    val line = c.getInputStream().bufferedReader().readLine().orEmpty()
                    hits += line
                    c.getOutputStream().write(
                        "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".toByteArray(),
                    )
                }
                runCatching { c.close() }
            }
        }.apply { isDaemon = true; start() }
    }

    @After
    fun close() {
        runCatching { server.close() }
    }

    private fun page(): String {
        val at = "http://127.0.0.1:${server.localPort}"
        return """
            <html><body>
              <img src="/inside.png">
              <video src="$at/video.mp4" preload="auto" muted></video>
              <audio src="$at/audio.mp3" preload="auto"></audio>
              <video preload="auto" muted><source src="$at/source.webm" type="video/webm"></video>
              <img src="$at/image.png">
            </body></html>
        """.trimIndent()
    }

    /** What the frame's client was asked for, and what reached the listener. */
    private class Asked : WebViewClient() {
        val intercepted: MutableList<String> = Collections.synchronizedList(mutableListOf())
        val finished = CountDownLatch(1)
        var serve: ((String) -> WebResourceResponse?)? = null
        var passThrough = false

        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean = true

        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? {
            val url = request.url.toString()
            intercepted += url
            if (passThrough) return null
            serve?.invoke(url)?.let { return it }
            return WebResourceResponse("text/plain", "utf-8", java.io.ByteArrayInputStream(ByteArray(0)))
        }

        override fun onPageFinished(view: WebView, url: String?) {
            finished.countDown()
        }
    }

    /**
     * Load the page with `frame` applied (or nothing, for the control), by
     * `loadUrl` of the origin served from the client where `served`, else
     * as data under the origin; wait, and return the client and the hits.
     */
    private fun run(
        name: String,
        frame: ((WebView) -> Unit)?,
        block: Boolean? = null,
        served: Boolean = false,
    ): Pair<Asked, List<String>> {
        hits.clear()
        val asked = Asked()
        val html = page()
        if (frame == null) asked.passThrough = true
        if (served) {
            asked.serve = { url ->
                when (url) {
                    "$origin/" -> WebResourceResponse("text/html", "utf-8", java.io.ByteArrayInputStream(html.toByteArray()))
                    else -> null
                }
            }
        }
        val app = InstrumentationRegistry.getInstrumentation().targetContext
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val web = WebView(app)
            frame?.invoke(web)
            block?.let { web.settings.blockNetworkLoads = it }
            web.webViewClient = asked
            if (served) {
                web.loadUrl("$origin/")
            } else {
                web.loadDataWithBaseURL(origin, html, "text/html", "utf-8", null)
            }
        }
        asked.finished.await(20, TimeUnit.SECONDS)
        // media preloads after the document finishes
        Thread.sleep(6_000)
        val seen = hits.toList()
        Log.i("egress", "$name: listener=${seen} intercepted=${asked.intercepted.toList()}")
        return asked to seen
    }

    @Test
    fun the_control_reaches_the_listener_so_a_silence_means_something() {
        val (_, seen) = run("control", null)
        assertTrue("with no frame the page reached nothing, so this run shows nothing: $seen", seen.isNotEmpty())
    }

    // functional: CAT-025
    @Test
    fun under_the_frame_nothing_reaches_an_outside_host_and_the_origin_is_served() {
        // as the screen loads it: the origin's URL, served by the client
        val (asked, seen) = run("frame", { Frames.resource(it) }, served = true)
        assertTrue("the page reached an outside host past the frame: $seen", seen.isEmpty())
        val inside = asked.intercepted.filter { it.startsWith(origin) }
        assertTrue("the origin's own document was not asked for: $inside", "$origin/" in inside)
        assertTrue("an image inside the origin was not asked for: $inside", "$origin/inside.png" in inside)
        // and every outside load the page made was offered to the client first
        val outside = asked.intercepted.filterNot { it.startsWith(origin) }
        for (want in listOf("video.mp4", "audio.mp3", "source.webm", "image.png")) {
            assertTrue("$want was never offered to the client: $outside", outside.any { it.endsWith("/$want") })
        }
    }

    /** **The client's hook alone holds**, without the second wall: what
     *  stands if a future engine changes how blocked loads are reported. */
    @Test
    fun the_clients_answer_alone_keeps_media_off_the_network() {
        val (_, seen) = run("frame, unblocked", { Frames.resource(it) }, block = false, served = true)
        assertTrue("with loads unblocked the page reached an outside host: $seen", seen.isEmpty())
    }
}
