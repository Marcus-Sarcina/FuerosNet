package com.comptus.fueros

import android.app.Application
import android.content.Context
import java.io.File
import timber.log.Timber

/**
 * The process, before any screen: where the field-test flavour's
 * diagnostics come up (`Robot/field-test-diagnostics.md`, section 4).
 *
 * **The releasable flavour does nothing here.** No tree is planted, so
 * every `Timber` call in the shell is a no-op; `Diag` has no sink, so every
 * shell event and every line the kernel hands to `Diagnostics.event` is
 * dropped at the first branch; no crash handler is installed; no stream
 * target is read. The fieldtest flavour plants a logcat tree, points
 * `Diag` at a file under `filesDir/diag/<run>.jsonl` mirrored to logcat
 * under `fueros.diag` and, where the bench left a target, to a live
 * stream (`DiagStream`), and installs the crash handler.
 * `BuildConfig.FIELD_TEST` is the one switch, read here and where a screen
 * shows or takes something that exists in this flavour alone.
 */
class FuerosApp : Application() {

    companion object {
        /** The live stream, fieldtest only; null when no target is kept. */
        @Volatile private var stream: DiagStream? = null

        private fun diagDir(context: Context) = File(context.filesDir, "diag")

        /**
         * Point the live stream at `address` (`host:port`; blank or `off`
         * forgets it) under `serial`, keep the target for the next launch,
         * and switch to it now. Called by `HomeActivity` for the bench's
         * `diag_stream` extra in the fieldtest flavour alone.
         */
        fun streamTo(context: Context, address: String, serial: String?) {
            if (!BuildConfig.FIELD_TEST) return
            val app = context.applicationContext
            DiagStream.store(diagDir(app), address, serial)
            stream?.close()
            stream = null
            val target = DiagStream.stored(diagDir(app)) ?: run {
                Diag.event("diag.stream", "target" to "off")
                return
            }
            stream = open(app, target.first, target.second)
        }

        private fun open(app: Context, address: String, serial: String?): DiagStream? {
            val (host, port) = DiagStream.parseAddress(address) ?: run {
                Diag.warn("diag.stream", "target" to address, "refused" to "not host:port")
                return null
            }
            val s = DiagStream(host, port, hello = {
                Diag.render(
                    Diag.ms(),
                    "info",
                    "shell",
                    DiagStream.EVENT_HELLO,
                    listOf("serial" to serial, "unix_ms" to System.currentTimeMillis()) + Report.headerFields(app),
                )
            })
            Diag.event("diag.stream", "target" to address, "serial" to serial)
            return s
        }
    }

    override fun onCreate() {
        super.onCreate()
        if (!BuildConfig.FIELD_TEST) return
        Timber.plant(Timber.DebugTree())
        val dir = diagDir(this)
        val file = DiagFile(dir, Diag.runId)
        DiagFile.prune(dir, keepRuns = 3)
        Diag.install(
            sink = { line ->
                file.offer(line)
                // the logcat mirror, under the one tag the bench follows
                Timber.tag("fueros.diag").d(line)
                stream?.offer(line)
            },
            flush = { file.flush() },
            files = { file.files() },
        )
        Crash.install()
        // before the first event, so the stream carries the run from its start
        DiagStream.stored(dir)?.let { (address, serial) -> stream = open(this, address, serial) }
        Diag.event(
            "shell.start",
            "run" to Diag.runId,
            "commit" to BuildConfig.GIT_COMMIT,
            "flavour" to BuildConfig.FLAVOR,
            "wall_ms" to Diag.startedWallMs,
        )
    }
}
