package com.comptus.fueros

import android.content.Context
import android.content.pm.PackageManager
import android.nfc.NfcAdapter
import android.os.Build

/**
 * **What a run's events are stamped with**: the run id, the commit, the
 * specification pins the build was checked against, the device, the radios
 * it has, the flavour and the app version, and the wall-clock anchor for
 * the `ms` fields. `DiagStream` opens every live stream with these.
 *
 * **The bundle and its share sheet are gone** [author, 2026-10-07]. They
 * zipped the event files and offered them to `ACTION_SEND`, which on a
 * phone with nothing that accepts a zip degenerates to a save dialogue
 * with nowhere to send it — so the control never worked. It was also
 * redundant: `crates/tools/field-run.sh` streams every event live while a
 * run holds and pulls every file from the device at stop, which is where
 * every measurement in this work has come from.
 */
object Report {

    fun header(context: Context): String = Diag.obj(headerFields(context))

    /** The header's fields, in order: `header.json`'s body, and what the
     *  live stream's hello carries (`DiagStream`). */
    fun headerFields(context: Context): List<Pair<String, Any?>> {
        val pm = context.packageManager
        return listOf(
            "run" to Diag.runId,
            "commit" to BuildConfig.GIT_COMMIT,
            "spec_pins" to pins(BuildConfig.SPEC_PINS),
            "flavour" to BuildConfig.FLAVOR,
            "app_version" to BuildConfig.VERSION_NAME,
            "app_version_code" to BuildConfig.VERSION_CODE,
            "device" to "${Build.MANUFACTURER} ${Build.MODEL}",
            "android" to Build.VERSION.RELEASE,
            "sdk" to Build.VERSION.SDK_INT,
            "radios" to mapOf(
                "nfc" to (NfcAdapter.getDefaultAdapter(context) != null),
                "ble" to pm.hasSystemFeature(PackageManager.FEATURE_BLUETOOTH_LE),
                "uwb" to pm.hasSystemFeature(PackageManager.FEATURE_UWB),
            ),
            "started_wall_ms" to Diag.startedWallMs,
            "reported_wall_ms" to System.currentTimeMillis(),
            "reported_ms" to Diag.ms(),
        )
    }

    /** `BuildConfig.SPEC_PINS`'s `name=prefix,...` back to a map. */
    fun pins(encoded: String): Map<String, String> =
        encoded.split(',').filter { '=' in it }.associate {
            val (name, prefix) = it.split('=', limit = 2)
            name to prefix
        }
}
