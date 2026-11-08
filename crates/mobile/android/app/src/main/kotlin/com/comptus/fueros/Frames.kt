package com.comptus.fueros

import android.webkit.WebSettings
import android.webkit.WebView

/**
 * **The two frames this app puts somebody else's markup in**, in one place
 * so a test can hold the same code the screens use (PRD-12, RSC-58).
 *
 * **Extracted rather than copied** [2026-11-08]. These settings are the
 * whole of what isolates a resource's page and a node's own pages from the
 * device: a test asserting a *copy* of them would pass while the screens
 * drifted, which is worse than no test. So [resource] and [node] are
 * applied by `ResourceActivity` and `NodePageActivity` and by
 * `FrameIsolationTest` on a real `WebView`, and there is one definition of
 * each.
 *
 * **Two frames and not one**, because the two differ in a way that
 * matters: a resource's page arrives as markup already in hand and fetches
 * nothing ever, while a node's own pages are *served* by the instance and
 * must load from its origin. Folding them together would mean either
 * letting a resource's page fetch or stopping a node's pages working.
 *
 * What neither permits, in both: script, DOM storage, a database, file or
 * content access, a cache that outlives the screen.
 */
object Frames {

    /**
     * **A resource's page** (RSC-58; `light-client-requirements.md` §8):
     * markup handed over by a resource, rendered with no origin to share,
     * nothing published into it, no fetch served, and nothing it stored
     * outliving the screen.
     *
     * The caller loads it with `loadDataWithBaseURL(null, …)`: **the null
     * base is the point** — an opaque origin, same-origin with nothing,
     * not even with itself.
     */
    fun resource(web: WebView) {
        web.settings.apply {
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
            cacheMode = WebSettings.LOAD_NO_CACHE
            useWideViewPort = true
            loadWithOverviewMode = true
            builtInZoomControls = true
            displayZoomControls = false
        }
    }

    /**
     * **A node's own administration pages** (PRD-12;
     * `infra-client-requirements.md` §8.3): served by the instance, so
     * loads from its origin are the point and everything else is shut.
     *
     * The caller confines navigation to that one origin; these are the
     * settings, and the origin rule lives with the screen that knows what
     * the origin is.
     */
    fun node(web: WebView) {
        web.settings.apply {
            // the node's pages carry no script, so refusing it costs
            // nothing and is the strongest single thing this frame does
            javaScriptEnabled = false
            javaScriptCanOpenWindowsAutomatically = false
            domStorageEnabled = false
            databaseEnabled = false
            allowFileAccess = false
            allowContentAccess = false
            allowFileAccessFromFileURLs = false
            allowUniversalAccessFromFileURLs = false
            setGeolocationEnabled(false)
            mediaPlaybackRequiresUserGesture = true
            // a page that could not be read at phone width would be one
            // the operator has to fight; §8.3 leaves the presentation here
            useWideViewPort = true
            loadWithOverviewMode = true
            builtInZoomControls = true
            displayZoomControls = false
            // nothing is cached, so nothing of the node's state outlives
            // the screen on this device
            cacheMode = WebSettings.LOAD_NO_CACHE
        }
    }

    /** What a frame does with a navigation a page asked for. */
    enum class Going {
        /** An act bounced out of the frame for a person to confirm. */
        ACT,

        /** Inside the instance's own origin: the frame loads it, proved. */
        INSIDE,

        /** Anywhere else: the frame does not go. */
        REFUSE,
    }

    /**
     * **Where a node's frame will go, as a decision rather than a
     * side-effect** (PRD-12; `infra-client-requirements.md` §8.3) — so the
     * instrumentation run can ask the same question the screen answers
     * without standing in for the screen's client.
     *
     * `bounced` is whether the target is one of the instance's own acts
     * (`Acts.bounced`), which the screen carries out rather than loading.
     * Everything outside the origin is [Going.REFUSE]: a page is free to
     * say `href="https://elsewhere"`, or `src="file:///etc/hosts"`, and
     * this frame simply does not go.
     */
    fun nodeGoing(origin: String, going: String, bounced: Boolean): Going = when {
        bounced -> Going.ACT
        going == origin || going.startsWith("$origin/") -> Going.INSIDE
        else -> Going.REFUSE
    }

    /**
     * **A resource's page goes nowhere at all** (RSC-58): it has no origin
     * to be inside of, so there is no navigation it could be allowed and
     * the decision takes no arguments.
     */
    fun resourceGoing(): Going = Going.REFUSE

    /**
     * **What both frames must refuse**, named rather than left implicit:
     * the instrumentation run asserts each of these off on a real
     * `WebView` after the frame is applied, so an edit that flips one
     * fails rather than quietly widening what a page reaches.
     *
     * Pure, so a JVM test can hold the list even where it cannot hold a
     * `WebView`.
     */
    val SHUT_IN_BOTH: List<String> = listOf(
        "javaScriptEnabled",
        "javaScriptCanOpenWindowsAutomatically",
        "domStorageEnabled",
        "databaseEnabled",
        "allowFileAccess",
        "allowContentAccess",
        "allowFileAccessFromFileURLs",
        "allowUniversalAccessFromFileURLs",
    )

    /**
     * **What only a resource's page refuses**: it fetches nothing ever,
     * where a node's pages are served and must load from their origin.
     */
    val SHUT_IN_A_RESOURCE_PAGE: List<String> = listOf(
        "blockNetworkLoads",
        "blockNetworkImage",
    )
}
