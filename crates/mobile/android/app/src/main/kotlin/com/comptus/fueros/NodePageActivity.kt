package com.comptus.fueros

import android.annotation.SuppressLint
import android.app.Activity
import android.graphics.Typeface
import android.os.Bundle
import android.webkit.WebResourceRequest
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.LinearLayout
import android.widget.TextView

/**
 * **The frame the node's own pages are presented in** (PRD-12,
 * `infra-client-requirements.md` §8.3).
 *
 * §8.3 divides the surface and gives the client this half: "a node
 * develops and serves its own administration pages; a client provides the
 * frame they are presented in". The node serves its own because §8.3
 * expects third-party implementations of both roles, and a surface agreed
 * between them would constrain what either may build.
 *
 * **Why the frame has to be isolated.** A seized node serving a hostile
 * page must reach nothing on the device that still holds the seed (design
 * §18.1, §23.3) — and the node cannot assert that isolation, because it is
 * the party the isolation is against. So it is asserted here:
 *
 *  - **No script.** The node's pages carry none by design: they are text,
 *    tables and plain forms. Refusing script is therefore free, and it is
 *    the single largest thing a hostile page would want.
 *  - **No bridge.** Nothing is published into the page with
 *    `addJavascriptInterface`, which is the one mechanism by which a page
 *    could call into this process at all.
 *  - **No local reach.** File and content access are off, so a page
 *    cannot name `file://` and read what this app holds.
 *  - **No storage.** Nothing the page leaves behind outlives the screen.
 *  - **One origin.** Navigation away from the instance's own address is
 *    refused rather than followed, so a page cannot carry the frame
 *    somewhere else and trade on having been opened from here.
 *
 * **What it is not.** This is containment of a page, not of the operator's
 * judgment: a node they enrolled is a node they chose to trust with their
 * delegation, and §10 is explicit that a compromised node can impersonate
 * its subordinates to every resource they use. The frame limits what a
 * page reaches on *this device*.
 */
class NodePageActivity : Activity() {

    private var at: String = ""
    private var token: String = ""

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val at = intent.getStringExtra("at")
        if (at.isNullOrBlank()) {
            setContentView(
                TextView(this).apply {
                    setPadding(48, 48, 48, 48)
                    text = "No instance was named."
                },
            )
            return
        }
        val origin = "http://$at"
        this.at = at
        // **the token, for the proof every read carries**: the page is
        // this instance's own state, so reading it is an act like any
        // other and proves itself the same way (§8.2)
        this.token = getSharedPreferences("operator", MODE_PRIVATE)
            .getString("token:$at", null) ?: ""

        val bar = TextView(this).apply {
            textSize = 12f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.sub)
            setPadding(24, 24, 24, 8)
            text = "$at — the node's own page, in a frame with no reach into this device"
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(bar)
            addView(frame(origin))
        }
        // the page the node serves scrolls inside the web view, and the
        // bars are this frame's to keep clear of ([`Screen`])
        Screen.insetting(root)
        setContentView(root)
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun frame(origin: String): WebView = WebView(this).apply {
        layoutParams = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            0,
        ).apply { weight = 1f }
        settings.apply {
            // the node's pages carry no script, so refusing it costs
            // nothing and is the strongest single thing this frame does
            javaScriptEnabled = false
            domStorageEnabled = false
            databaseEnabled = false
            allowFileAccess = false
            allowContentAccess = false
            // a page that could not be read at phone width would be one
            // the operator has to fight; §8.3 leaves the presentation here
            useWideViewPort = true
            loadWithOverviewMode = true
            builtInZoomControls = true
            displayZoomControls = false
            // nothing is cached, so nothing of the node's state outlives
            // the screen on this device
            cacheMode = android.webkit.WebSettings.LOAD_NO_CACHE
        }
        webViewClient = object : WebViewClient() {
            /**
             * **One origin.** The instance's own address and nothing else:
             * a page is free to say `href="https://elsewhere"`, and this
             * frame simply does not go.
             */
            override fun shouldOverrideUrlLoading(
                view: WebView,
                request: WebResourceRequest,
            ): Boolean {
                val going = request.url.toString()
                // **an act bounces out of the frame to this client**
                // [author, 2026-11-02]: the page carries no form, because
                // a form cannot compute the MAC an act proves itself
                // with and cannot reach the key an act on the machine
                // needs (`infra-client-requirements.md` §8.3). So each
                // act is a link in this scheme, and **what rides in it is
                // a hint**: `ActActivity` fills a field the person
                // confirms, and the page makes nothing happen.
                val bounced = Acts.bounced(going)
                if (bounced != null) {
                    val (name, query) = bounced
                    startActivity(
                        android.content.Intent(
                            this@NodePageActivity,
                            ActActivity::class.java,
                        )
                            .putExtra("at", at)
                            .putExtra("act", name)
                            .putExtra("query", query),
                    )
                    return true
                }
                // **a read of this instance's own page is proved too**, so
                // the frame re-signs each navigation inside the origin
                // rather than following it bare
                if (going.startsWith("$origin/") || going == origin) {
                    // **a URL this frame already proved is let through**,
                    // which is what stops `go` re-entering itself: a
                    // target carrying a proof is one we minted, and a
                    // page that put one there of its own reaches an
                    // instance that refuses it
                    if (going.contains("&proof=") || going.contains("?proof=")) return false
                    go(view, origin, going)
                    return true
                }
                view.post {
                    android.widget.Toast.makeText(
                        view.context,
                        "that page is not this node's, so the frame did not go",
                        android.widget.Toast.LENGTH_SHORT,
                    ).show()
                }
                return true
            }
        }
        go(this, origin, "$origin/")
    }

    /**
     * **Load one page of the instance's own, proved.**
     *
     * A read of the administration surface is an act like any other off
     * the loopback (§8.2), and a `GET` carries no body — so the proof is
     * over the method and the target alone and **this client can mint it
     * for a URL the frame is about to load**. That is the whole of what
     * reading the page from a phone needed; no session and no cookie.
     *
     * Where there is no token the bare URL is loaded anyway: on a
     * loopback instance arriving is the authorisation, and an operator
     * running one there should see their page rather than a refusal this
     * client invented.
     */
    private fun go(view: WebView, origin: String, url: String) {
        if (token.isEmpty()) {
            view.loadUrl(url)
            return
        }
        val path = url.removePrefix(origin).ifEmpty { "/" }
        val target = Provision.proved(token, "GET", path, ByteArray(0))
        view.loadUrl(target.fold({ "$origin$it" }, { url }))
    }

    override fun onDestroy() {
        // **nothing the page left behind outlives the screen**
        WebStorage.getInstance().deleteAllData()
        android.webkit.CookieManager.getInstance().removeAllCookies(null)
        super.onDestroy()
    }
}
