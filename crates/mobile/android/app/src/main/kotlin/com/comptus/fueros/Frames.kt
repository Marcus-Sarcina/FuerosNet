package com.comptus.fueros

import android.content.Context
import android.webkit.WebSettings
import android.webkit.WebStorage
import android.webkit.WebView
import androidx.webkit.ProxyConfig
import androidx.webkit.ProxyController
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature

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

    /**
     * **Where a frame's engine is sent with everything it would fetch for
     * itself: nowhere** [found on GXV, WebView 157, 2026-11-24]. A load
     * the engine offers the screen's client is carried or refused there,
     * but a page has ways out that are never offered: a socket, from a
     * window or a worker; a transport over datagrams; a link that only
     * asks for a connection to be opened ahead of use. Each reached a
     * listener outside. So the engine is given a proxy for all of it, and
     * the proxy is a port of this device's own that nothing may listen on.
     */
    const val NOWHERE: String = "127.0.0.1:1"

    /**
     * **What is taken from every window of a resource's page before any
     * of its own code runs**: a peer connection sends datagrams of its
     * own to an address the page chooses, by no proxy and by no load, so
     * the only place to refuse one is the constructor.
     */
    const val NO_PEERS: String =
        "(function(){for(const n of ['RTCPeerConnection','webkitRTCPeerConnection'])" +
            "{try{Object.defineProperty(window,n,{value:undefined,writable:false,configurable:false})}" +
            "catch(e){}}})();"

    /**
     * **Whether this device's engine can hold a page that runs code to
     * its frame**: [wall] and [NO_PEERS] both. Where it cannot, a
     * resource's page is shown without its code, whatever was allowed.
     */
    fun confines(): Boolean =
        runCatching {
            WebViewFeature.isFeatureSupported(WebViewFeature.PROXY_OVERRIDE) &&
                WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)
        }.getOrDefault(false)

    /**
     * **Put the wall behind every frame of this app, and then go on**
     * ([NOWHERE]). It is one setting for the whole process, so `through`
     * names the one origin a node's own page is served from while that
     * screen is in front, and nothing while it is not; this device's own
     * loopback is walled with the rest. `then` runs on the main thread
     * once the engine has taken it, which is when a frame may load.
     *
     * **What it leaves** while a node's page is in front: a resource's
     * page still running behind it may open a connection to that one
     * origin, which answers nothing unproved.
     */
    fun wall(through: String? = null, then: () -> Unit) {
        val main = android.os.Handler(android.os.Looper.getMainLooper())
        val supported = runCatching {
            WebViewFeature.isFeatureSupported(WebViewFeature.PROXY_OVERRIDE)
        }.getOrDefault(false)
        if (!supported) {
            main.post(then)
            return
        }
        val config = ProxyConfig.Builder()
            .addProxyRule(NOWHERE)
            .removeImplicitRules()
            .apply { through?.let { addBypassRule(it) } }
            .build()
        ProxyController.getInstance().setProxyOverride(config, { main.post(it) }, then)
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
            // pages run; nothing to keep where they do not. **WebSQL stays
            // off**: the engine has removed it and reads `false` whatever
            // is set [found on MRK, 2026-11-16], and IndexedDB comes with
            // script and DOM storage
            domStorageEnabled = script
            databaseEnabled = false
            allowFileAccess = false
            allowContentAccess = false
            allowFileAccessFromFileURLs = false
            allowUniversalAccessFromFileURLs = false
            // **loads are the screen's client's to carry or refuse, and
            // the network is shut behind it as a second wall**: the engine
            // still offers every load to the client first, the page's own
            // origin is served from there, and nothing the client did not
            // answer can reach the network [MediaEgressTest, MRK, WebView
            // 157, 2026-11-16: media and images to an outside host asked
            // the client, reached nothing, and the origin's own document
            // was served with loads blocked]
            blockNetworkLoads = true
            blockNetworkImage = false
            setGeolocationEnabled(false)
            mediaPlaybackRequiresUserGesture = true
            cacheMode = WebSettings.LOAD_NO_CACHE
            useWideViewPort = true
            loadWithOverviewMode = true
            builtInZoomControls = true
            displayZoomControls = false
        }
        // **and a page that runs has no peer connection to run** ([NO_PEERS]),
        // in its own window and in each one it makes
        if (script) {
            WebViewCompat.addDocumentStartJavaScript(web, NO_PEERS, setOf("*"))
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
     * **The status the engine is handed for a resource's answer.** The
     * platform's frame refuses to be given a status from 300 to 399 at all
     * and throws, and a redirect is not followed here in any case
     * (`resource-requirements.md` §3): the body it carried is what is
     * shown, so it is handed over as an answer. A number that is no HTTP
     * status is a bad answer, as the desktop's frame has it.
     */
    fun engineStatus(http: Int): Int = when (http) {
        in 300..399 -> 200
        in 100..599 -> http
        else -> 502
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
        "databaseEnabled",
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
    )
}
