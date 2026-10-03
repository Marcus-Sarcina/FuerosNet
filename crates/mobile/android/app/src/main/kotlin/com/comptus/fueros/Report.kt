package com.comptus.fueros

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.nfc.NfcAdapter
import android.os.Build
import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

/**
 * The Report action (`Robot/field-test-diagnostics.md`, section 4, "the
 * bundle"): the run's event files and a header, zipped under app-private
 * storage and offered through the share sheet. Fieldtest flavour only; the
 * screens show the button under `BuildConfig.FIELD_TEST` and the provider
 * that serves the zip is declared in that flavour's manifest alone.
 *
 * The header names what the merge tool and a reader need to place the
 * file: the run id, the commit, the specification pins the build was
 * checked against, the device, the radios it has, the flavour and the app
 * version, and the wall-clock anchor for the `ms` fields. The tester's
 * checklist and the counterparty's run id are M4's.
 */
object Report {

    const val DIR = "report"

    /** Build the zip and return it. */
    fun assemble(context: Context): File {
        Diag.event("report.assemble", "run" to Diag.runId)
        Diag.flush()
        val dir = File(File(context.filesDir, "diag"), DIR).apply { mkdirs() }
        dir.listFiles()?.forEach { it.delete() }
        val zip = File(dir, "fueros-${Diag.runId}.zip")
        ZipOutputStream(zip.outputStream().buffered()).use { z ->
            z.putNextEntry(ZipEntry("header.json"))
            z.write(header(context).toByteArray())
            z.closeEntry()
            for (f in Diag.files()) {
                z.putNextEntry(ZipEntry("events/${f.name}"))
                f.inputStream().use { it.copyTo(z) }
                z.closeEntry()
            }
        }
        return zip
    }

    /** Assemble and hand to the share sheet. */
    fun send(activity: Activity) {
        val zip = assemble(activity)
        val uri = ReportProvider.uriFor(activity, zip)
        val intent = Intent(Intent.ACTION_SEND).apply {
            type = "application/zip"
            putExtra(Intent.EXTRA_STREAM, uri)
            putExtra(Intent.EXTRA_SUBJECT, "fueros run ${Diag.runId}")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        activity.startActivity(Intent.createChooser(intent, "Send the diagnostics bundle"))
        Diag.event("report.sent", "bytes" to zip.length())
    }

    /** `header.json`. */
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
