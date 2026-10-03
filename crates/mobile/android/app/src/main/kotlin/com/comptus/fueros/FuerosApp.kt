package com.comptus.fueros

import android.app.Application
import java.io.File
import timber.log.Timber

/**
 * The process, before any screen: where the field-test flavour's
 * diagnostics come up (`Robot/field-test-diagnostics.md`, section 4).
 *
 * **The releasable flavour does nothing here.** No tree is planted, so
 * every `Timber` call in the shell is a no-op; `Diag` has no sink, so every
 * shell event and every line the kernel hands to `Diagnostics.event` is
 * dropped at the first branch; no crash handler is installed. The
 * fieldtest flavour plants a logcat tree, points `Diag` at a file under
 * `filesDir/diag/<run>.jsonl` mirrored to logcat under `fueros.diag`, and
 * installs the crash handler. `BuildConfig.FIELD_TEST` is the one switch,
 * read in this file alone.
 */
class FuerosApp : Application() {

    override fun onCreate() {
        super.onCreate()
        if (!BuildConfig.FIELD_TEST) return
        Timber.plant(Timber.DebugTree())
        val dir = File(filesDir, "diag")
        val file = DiagFile(dir, Diag.runId)
        DiagFile.prune(dir, keepRuns = 3)
        Diag.install(
            sink = { line ->
                file.offer(line)
                // the logcat mirror, under the one tag the bench follows
                Timber.tag("fueros.diag").d(line)
            },
            flush = { file.flush() },
            files = { file.files() },
        )
        Crash.install()
        Diag.event(
            "shell.start",
            "run" to Diag.runId,
            "commit" to BuildConfig.GIT_COMMIT,
            "flavour" to BuildConfig.FLAVOR,
            "wall_ms" to Diag.startedWallMs,
        )
    }
}
