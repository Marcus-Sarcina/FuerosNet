package com.comptus.fueros

import android.annotation.SuppressLint
import android.app.Activity
import android.graphics.Color
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

        val bar = TextView(this).apply {
            textSize = 12f
            typeface = Typeface.MONOSPACE
            setTextColor(Color.GRAY)
            setPadding(24, 24, 24, 8)
            text = "$at — the node's own page, in a frame with no reach into this device"
        }
        setContentView(
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                fitsSystemWindows = true
                addView(bar)
                addView(frame(origin))
            },
        )
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
                if (going.startsWith("$origin/") || going == origin) return false
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
        loadUrl("$origin/")
    }

    override fun onDestroy() {
        // **nothing the page left behind outlives the screen**
        WebStorage.getInstance().deleteAllData()
        android.webkit.CookieManager.getInstance().removeAllCookies(null)
        super.onDestroy()
    }
}
