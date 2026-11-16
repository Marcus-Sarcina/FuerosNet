package com.comptus.fueros

import android.content.Context
import android.webkit.WebSettings
import android.webkit.WebStorage
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
 * matters: a node's own pages are *served* by the instance and load from
 * its origin with no script ever, while a resource's page lives in an
 * origin of its own that this shell mints for the resource, reaches that
 * one resource through the kernel, keeps storage of its own, and runs
 * script only where the page declared it and the person allowed it
 * (`light-client-requirements.md` §8) [author, 2026-11-15]. Folding them
 * together would mean either giving a node's pages a bridge or taking a
 * resource's own storage away.
 *
 * What neither permits, in both: a bridge into this app, file or content
 * access, a load from anywhere but its own origin.
 */
object Frames {

    /**
     * **A resource's origin, minted here** (`light-client-requirements.md`
     * §8): the resource's keyhash under a name no network resolves, so the
     * page is same-origin with its own resource and nothing else, its
     * storage is its own, and a load to it reaches the kernel and not a
     * socket. Two labels, since one may not exceed 63 characters.
     */
    fun origin(resource: String): String {
        val hex = resource.lowercase()
        require(hex.length == 64 && hex.all { it in '0'..'9' || it in 'a'..'f' }) { "a resource is 64 hex digits" }
        return "https://${hex.substring(0, 32)}.${hex.substring(32)}.rhtn.invalid"
    }

    /** The per-resource store of what the person allowed a page to run. */
    const val SCRIPT_STORE: String = "resource-script"

    /**
     * **Whether the person allowed this resource's pages to run their
     * code**: `true` or `false` once they decided, `null` before — and
     * null is what puts the question, once per resource.
     */
    fun scriptAllowed(context: Context, resource: String): Boolean? =
        runCatching {
            val prefs = context.getSharedPreferences(SCRIPT_STORE, Context.MODE_PRIVATE)
            if (prefs.contains(resource)) prefs.getBoolean(resource, false) else null
        }.getOrNull()

    /** The person's answer, kept for the resource. */
    fun allowScript(context: Context, resource: String, allowed: Boolean) {
        runCatching {
            context.getSharedPreferences(SCRIPT_STORE, Context.MODE_PRIVATE)
                .edit()
                .putBoolean(resource, allowed)
                .apply()
        }
    }

    /**
     * **Forget a resource on this device**: what its pages stored under
     * their origin, and what the person allowed them. Nothing of another
     * resource is touched, which is what the origin per resource is for.
     */
    fun forget(context: Context, resource: String) {
        runCatching {
            context.getSharedPreferences(SCRIPT_STORE, Context.MODE_PRIVATE)
                .edit()
                .remove(resource)
                .apply()
        }
        runCatching { WebStorage.getInstance().deleteOrigin(origin(resource)) }
    }

    /**
     * **A resource's page** (RSC-58; `light-client-requirements.md` §8)
     * [author, 2026-11-15]: a page in the origin [origin] mints for its
     * resource, reaching that resource and nothing else — every load and
     * navigation inside the origin is carried by the screen's client
     * through the kernel, and every other one is answered with nothing —
     * keeping storage under that origin, and running script only as
     * `script` says: the page's own declaration and the person's answer,
     * both decided by the screen before the frame is made. **No bridge
     * into this app**: the one object a page that runs script is given
     * ([ResourceBridge]) carries a request to the same resource and
     * nothing else.
     *
     * The caller loads the origin's URL and serves it from the kernel's
     * answer: the engine then enforces the page's own policy, and no
     * document is ever handed to it with no origin.
     */
    fun resource(web: WebView, script: Boolean = false) {
        web.settings.apply {
            javaScriptEnabled = script
            javaScriptCanOpenWindowsAutomatically = false
            // storage of the resource's own, under its origin, where its
            // pages run; nothing to keep where they do not
            domStorageEnabled = script
            databaseEnabled = script
            allowFileAccess = false
            allowContentAccess = false
            allowFileAccessFromFileURLs = false
            allowUniversalAccessFromFileURLs = false
            // loads are the screen's client's to carry or refuse, every
            // one, so the engine is not told to block them before it asks
            blockNetworkLoads = false
            blockNetworkImage = false
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
     * **Where a resource's frame will go** (RSC-58): inside the origin
     * minted for its resource, and nowhere else — the same shape as
     * [nodeGoing] without the bounce, since a resource has no acts of the
     * client's to ask for.
     */
    fun resourceGoing(origin: String, going: String): Going = when {
        going == origin || going.startsWith("$origin/") -> Going.INSIDE
        else -> Going.REFUSE
    }

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
        "javaScriptCanOpenWindowsAutomatically",
        "allowFileAccess",
        "allowContentAccess",
        "allowFileAccessFromFileURLs",
        "allowUniversalAccessFromFileURLs",
    )

    /**
     * **What a node's page refuses always, and a resource's page refuses
     * unless its author declared it and the person allowed it**: script,
     * and the storage that goes with a page that runs.
     */
    val SHUT_UNLESS_ALLOWED: List<String> = listOf(
        "javaScriptEnabled",
        "domStorageEnabled",
        "databaseEnabled",
    )
}
