package com.comptus.fueros

import android.content.Context
import java.io.File
import org.mozilla.geckoview.ContentBlocking
import org.mozilla.geckoview.GeckoPreferenceController
import org.mozilla.geckoview.GeckoResult
import org.mozilla.geckoview.GeckoRuntime
import org.mozilla.geckoview.GeckoRuntimeSettings
import org.mozilla.geckoview.GeckoSession
import org.mozilla.geckoview.GeckoSessionSettings
import org.mozilla.geckoview.StorageController

/**
 * **The engine a resource's page is framed in** [author, 2026-11-25]:
 * Mozilla's, shipped in this app, in place of the system's.
 *
 * **Why not the system's** [found on GXV, 2026-11-24]. A page whose code
 * the person allowed left the system engine's frame seven ways that are
 * never offered to this app: sockets from its window, from a window it
 * made and from a worker; a link that only asks for a connection ahead of
 * use; a transport over datagrams; and a peer connection from its own
 * window and from one it made. A proxy behind that engine shut the first
 * five. The last has no setting there, and a script cannot take it from a
 * window the page makes for itself before the page reads it.
 *
 * **What this engine is told**, once, before any page:
 *
 *  - **one proxy for everything, and it is this app's own** ([Carrier]):
 *    plain requests go there, where a framed resource's origin is answered
 *    through the kernel and everything else is refused; a tunnel, a
 *    socket and any other stream go to a port of this device that nothing
 *    may listen on; this device's own loopback is proxied with the rest;
 *  - **no peer connection and no datagram transport**, in any window;
 *  - **nothing fetched ahead of being asked for.**
 *
 * It is one engine for the process and these hold for every page in it.
 * A frame is not loaded until they have been read back as set ([with]).
 */
object Engine {

    /** A port of this device that nothing may listen on: below 1024 is the system's to bind. */
    const val NOWHERE: Int = 1

    /** The id the built-in extension that publishes `rhtn.request` goes by. */
    const val FRAME_EXTENSION: String = "frame@fueros.comptus.com"

    /** **What the engine is told**, by name: `proxy` is where [Carrier] listens. */
    fun prefs(proxy: Int): List<Pair<String, Any>> = listOf(
        "media.peerconnection.enabled" to false,
        "network.webtransport.enabled" to false,
        "network.http.http3.enable" to false,
        "network.proxy.type" to 1,
        "network.proxy.http" to "127.0.0.1",
        "network.proxy.http_port" to proxy,
        "network.proxy.ssl" to "127.0.0.1",
        "network.proxy.ssl_port" to NOWHERE,
        "network.proxy.socks" to "127.0.0.1",
        "network.proxy.socks_port" to NOWHERE,
        "network.proxy.socks_remote_dns" to true,
        "network.proxy.no_proxies_on" to "",
        "network.proxy.failover_direct" to false,
        // a resource's origin is a name under `localhost`, which the engine
        // would otherwise reach directly and, proxied, would stop treating
        // as a secure context
        "network.proxy.allow_hijacking_localhost" to true,
        "network.proxy.testing_localhost_is_secure_when_hijacked" to true,
        "network.dns.disablePrefetch" to true,
        "network.dns.disablePrefetchFromHTTPS" to true,
        "network.prefetch-next" to false,
        "network.http.speculative-parallel-limit" to 0,
        "network.predictor.enabled" to false,
        "network.captive-portal-service.enabled" to false,
        "network.connectivity-service.enabled" to false,
    )

    /** The same as the file the engine reads before it starts. */
    fun file(proxy: Int): String = prefs(proxy).joinToString("\n", "prefs:\n", "\n") { (name, value) ->
        "  $name: " + if (value is String) "\"$value\"" else value.toString()
    }

    /** Whether this build carries the engine for this device's processor. */
    fun carried(): Boolean = android.os.Build.SUPPORTED_ABIS.firstOrNull() == "arm64-v8a"

    private var runtime: GeckoRuntime? = null
    private var held = false
    private val waiting = mutableListOf<(GeckoRuntime?) -> Unit>()

    /**
     * **The engine, once it holds what it was told.** Called on the main
     * thread; `then` is run there with the engine, or with nothing where
     * what it was told could not be read back as set — in which case no
     * page is framed.
     */
    fun with(context: Context, then: (GeckoRuntime?) -> Unit) {
        runtime?.let { if (held) return then(it) }
        waiting += then
        if (runtime != null) return
        val app = context.applicationContext
        val proxy = Carrier.port()
        val config = File(app.filesDir, "engine.yaml").apply { writeText(file(proxy)) }
        val made = GeckoRuntime.create(
            app,
            GeckoRuntimeSettings.Builder()
                .configFilePath(config.absolutePath)
                .consoleOutput(BuildConfig.FIELD_TEST)
                .aboutConfigEnabled(false)
                .remoteDebuggingEnabled(false)
                .contentBlocking(
                    ContentBlocking.Settings.Builder()
                        .safeBrowsing(ContentBlocking.SafeBrowsing.NONE)
                        .build(),
                )
                .build(),
        )
        runtime = made
        val done = { ok: Boolean ->
            held = ok
            if (!ok) Diag.warn("engine.held", "why" to "what the engine was told did not read back as set")
            val all = waiting.toList()
            waiting.clear()
            for (w in all) w(if (ok) made else null)
        }
        // told again by name, so nothing rests on the file alone, and then
        // read back: the three a frame's confinement stands on
        var chain: GeckoResult<Void> = GeckoResult.fromValue(null)
        for ((name, value) in prefs(proxy)) {
            chain = chain.then {
                when (value) {
                    is Boolean -> GeckoPreferenceController.setGeckoPref(name, value, GeckoPreferenceController.PREF_BRANCH_USER)
                    is Int -> GeckoPreferenceController.setGeckoPref(name, value, GeckoPreferenceController.PREF_BRANCH_USER)
                    else -> GeckoPreferenceController.setGeckoPref(name, value.toString(), GeckoPreferenceController.PREF_BRANCH_USER)
                }
            }
        }
        chain
            .then {
                GeckoPreferenceController.getGeckoPrefs(
                    listOf("media.peerconnection.enabled", "network.proxy.type", "network.proxy.http_port"),
                )
            }
            .then { read ->
                val got = read.orEmpty().associate { it.pref to it.value }
                val ok = got["media.peerconnection.enabled"] == false &&
                    got["network.proxy.type"] == 1 &&
                    got["network.proxy.http_port"] == proxy
                if (!ok) return@then GeckoResult.fromValue(false)
                // and the page's one object, published by a script of this
                // app's own that the engine runs before the page's
                made.webExtensionController
                    .ensureBuiltIn("resource://android/assets/frame/", FRAME_EXTENSION)
                    .map { true }
            }
            .accept({ ok -> done(ok == true) }, { e ->
                Diag.warn("engine.held", "why" to (e?.message ?: "failed"))
                done(false)
            })
    }

    /**
     * **A session for one resource's page**: script as the page declared
     * and the person allowed, and the proxy's challenge answered with this
     * process's secret and nothing else.
     */
    fun session(script: Boolean): GeckoSession =
        GeckoSession(
            GeckoSessionSettings.Builder()
                .allowJavascript(script)
                .usePrivateMode(false)
                .suspendMediaWhenInactive(true)
                .build(),
        ).apply {
            promptDelegate = object : GeckoSession.PromptDelegate {
                override fun onAuthPrompt(
                    session: GeckoSession,
                    prompt: GeckoSession.PromptDelegate.AuthPrompt,
                ): GeckoResult<GeckoSession.PromptDelegate.PromptResponse> {
                    val proxy = prompt.authOptions.flags and
                        GeckoSession.PromptDelegate.AuthPrompt.AuthOptions.Flags.PROXY != 0
                    return GeckoResult.fromValue(
                        if (proxy) prompt.confirm(Carrier.USER, Carrier.secret) else prompt.dismiss(),
                    )
                }
            }
        }

    /**
     * **Give pages a peer connection, or take it away**, and say whether
     * the engine then reads as told. It is one setting for the engine, so
     * it is on only while one live session runs
     * (`light-client-requirements.md` §8.1) and every other resource's
     * frame is shut first ([only]); a page made while it is on keeps its
     * connection until that page is gone, which is why the session's end
     * loads its page again.
     */
    fun peers(on: Boolean, then: (Boolean) -> Unit) {
        GeckoPreferenceController
            .setGeckoPref("media.peerconnection.enabled", on, GeckoPreferenceController.PREF_BRANCH_USER)
            .then { GeckoPreferenceController.getGeckoPref("media.peerconnection.enabled") }
            .accept({ then(it?.value == on) }, { then(false) })
    }

    private val open = mutableSetOf<GeckoSession>()

    /** A frame's session, kept so a live session can shut the others. */
    fun opened(session: GeckoSession) {
        open += session
    }

    /** That session is gone. */
    fun closed(session: GeckoSession) {
        open -= session
    }

    /** **Shut every frame but this one**: no page but the one a live
     *  session was approved for is running while pages are given a peer
     *  connection. A screen whose frame was shut makes it again when it
     *  is next in front. */
    fun only(session: GeckoSession) {
        for (other in open.filter { it !== session }) {
            runCatching { other.close() }
            open -= other
        }
    }

    /** Wipe what a resource's pages stored under its origin. */
    fun forget(context: Context, resource: String) {
        if (!carried()) return
        val host = Frames.host(resource)
        android.os.Handler(android.os.Looper.getMainLooper()).post {
            with(context) { it?.storageController?.clearDataFromHost(host, StorageController.ClearFlags.ALL) }
        }
    }
}
