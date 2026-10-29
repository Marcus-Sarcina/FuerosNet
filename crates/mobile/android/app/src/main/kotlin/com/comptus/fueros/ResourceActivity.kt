package com.comptus.fueros

import android.annotation.SuppressLint
import android.app.Activity
import android.graphics.Typeface
import android.os.Bundle
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **A resource's page, in a frame that reaches nothing** (the screens
 * sheet's F; `light-client-requirements.md` §8, PRD-12's argument one
 * step on).
 *
 * The gap this closes: a resource's answer was read as the first line of
 * its raw HTTP message on a diagnostic screen, so the catalog could ask a
 * resource for something and nobody could look at what came back. What a
 * page may do was written down as owed.
 *
 * **[ResourcePage] holds the policy and the reasons**; this is the frame
 * that enforces it. In one place, because a second screen deciding what a
 * page may do is how the two come apart:
 *
 *  - **A null base URL**, so the document has an opaque origin: same
 *    origin with nothing, not even itself. There is no URL to give it —
 *    §11's exchange is one request and one response through the kernel —
 *    and nothing to be same-origin *with* is the strongest isolation a
 *    web view has.
 *  - **No network, twice over**: the settings block loads, and
 *    `shouldInterceptRequest` answers every request with nothing. A
 *    page's `src` has no base to resolve against in any case, so this
 *    costs an application nothing it could have had.
 *  - **No script**, and with no origin, no network and no bridge there is
 *    nothing script could reach: not the resource, not this process, not
 *    the device. A form is carried by the shell through the kernel
 *    either way.
 *  - **No bridge**: nothing is published into the page.
 *  - **No local reach**: file and content access off, both file-URL
 *    escapes off.
 *  - **No storage**, and what a page leaves behind is deleted with the
 *    screen.
 *  - **No navigation**: there is no origin to stay inside, so every
 *    navigation is refused and the person is told the frame did not go.
 *
 * **Text takes no frame at all.** `text/plain` is not a page, so it is
 * drawn in a `TextView` and no web view is created — the policy's own
 * rule, which is also the cheaper path.
 */
class ResourceActivity : Activity() {

    private var web: WebView? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val node = intent.getStringExtra("node").orEmpty()
        val resource = intent.getStringExtra("resource").orEmpty()
        val name = intent.getStringExtra("name").orEmpty()
        val path = intent.getStringExtra("path") ?: "/"

        val title = TextView(this).apply {
            textSize = 19f
            typeface = Typeface.DEFAULT_BOLD
            text = name.ifBlank { "A resource" }
            setPadding(Screen.gutter(context), 20, Screen.gutter(context), 2)
        }
        val from = TextView(this).apply {
            textSize = 12f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.sub)
            text = ResourcePage.provenance(resource, node)
            setPadding(Screen.gutter(context), 0, Screen.gutter(context), 6)
        }
        val says = TextView(this).apply {
            textSize = 12f
            setTextColor(Palette.now.sub)
            text = ResourcePage.FRAME_SAYS
            setPadding(Screen.gutter(context), 0, Screen.gutter(context), 10)
        }
        val held = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
            ).apply { weight = 1f }
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(title)
            addView(from)
            addView(says)
            addView(held)
        }
        Screen.insetting(root)
        setContentView(root)

        held.addView(note("asking…"))
        // **off the drawing thread**: asking a resource is a round trip
        // through the hosting node
        Thread({
            val answered = Kernel.ask(node, resource, path)
            runOnUiThread {
                if (isFinishing) return@runOnUiThread
                held.removeAllViews()
                when (val showing = ResourcePage.showing(answered)) {
                    is ResourcePage.Showing.Page -> held.addView(frame(showing.html))
                    is ResourcePage.Showing.Text -> held.addView(text(showing.text))
                    is ResourcePage.Showing.Said -> held.addView(note(showing.why))
                }
            }
        }, "fueros-resource").start()
    }

    /** The frame, and the whole of what it permits. */
    @SuppressLint("SetJavaScriptEnabled")
    private fun frame(html: String): WebView = WebView(this).apply {
        web = this
        layoutParams = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.MATCH_PARENT,
        )
        settings.apply {
            javaScriptEnabled = false
            javaScriptCanOpenWindowsAutomatically = false
            domStorageEnabled = false
            databaseEnabled = false
            allowFileAccess = false
            allowContentAccess = false
            allowFileAccessFromFileURLs = false
            allowUniversalAccessFromFileURLs = false
            // nothing in the page can be fetched, and the arithmetic of
            // whether a `src` resolves is not what this rests on
            blockNetworkLoads = true
            blockNetworkImage = true
            setGeolocationEnabled(false)
            mediaPlaybackRequiresUserGesture = true
            cacheMode = android.webkit.WebSettings.LOAD_NO_CACHE
            useWideViewPort = true
            loadWithOverviewMode = true
            builtInZoomControls = true
            displayZoomControls = false
        }
        webViewClient = object : WebViewClient() {
            /** **No origin, so no navigation.** Every one is refused. */
            override fun shouldOverrideUrlLoading(
                view: WebView,
                request: WebResourceRequest,
            ): Boolean {
                view.post {
                    android.widget.Toast.makeText(
                        view.context,
                        "a resource's page goes nowhere: the frame did not follow that",
                        android.widget.Toast.LENGTH_SHORT,
                    ).show()
                }
                return true
            }

            /**
             * **And nothing is fetched.** The settings already block
             * loads; this is the same rule where a request would arrive
             * anyway, answered with an empty body rather than an error
             * the page could time.
             */
            override fun shouldInterceptRequest(
                view: WebView,
                request: WebResourceRequest,
            ): WebResourceResponse = WebResourceResponse(
                "text/plain",
                "utf-8",
                java.io.ByteArrayInputStream(ByteArray(0)),
            )
        }
        // the null base is the point: an opaque origin, same-origin with
        // nothing, not even with itself
        loadDataWithBaseURL(null, html, "text/html", "utf-8", null)
    }

    /** `text/plain` is not a page, so it is not framed as one. */
    private fun text(body: String): ScrollView = ScrollView(this).apply {
        layoutParams = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.MATCH_PARENT,
        )
        addView(
            TextView(this@ResourceActivity).apply {
                textSize = 14f
                typeface = Typeface.MONOSPACE
                text = body
                setPadding(Screen.gutter(context), 8, Screen.gutter(context), 48)
            },
        )
    }

    private fun note(t: String) = TextView(this).apply {
        textSize = 14f
        text = t
        setPadding(Screen.gutter(context), 12, Screen.gutter(context), 24)
    }

    override fun onDestroy() {
        // **nothing the page left behind outlives the screen**
        web?.let {
            it.stopLoading()
            it.loadUrl("about:blank")
            it.destroy()
        }
        web = null
        WebStorage.getInstance().deleteAllData()
        android.webkit.CookieManager.getInstance().removeAllCookies(null)
        super.onDestroy()
    }
}
