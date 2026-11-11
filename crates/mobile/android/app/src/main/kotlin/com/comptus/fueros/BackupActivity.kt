package com.comptus.fueros

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **The door to a backup** (design §13.7.1, `light-client-requirements.md`
 * §2; PRD-07, PRD-10).
 *
 * The kernel has had `export_backup` and `restore_backup` throughout and
 * the shell had no way in, so what a device loss takes away was
 * recoverable in principle and by nobody in practice. This is the way in.
 *
 * **What it writes is already safe to put anywhere.** The blob is
 * enveloped under a key derived from the identity (design §13.7.1), so a
 * cloud folder holds ciphertext; the identity is not in it, and the seed
 * phrase is what opens it again. The screen says so, because a person who
 * keeps the backup beside a written-down phrase has one file and not two.
 *
 * **Where it went is kept, and that is the only state.** PRD-10 wants the
 * prompt to return if the backup is deleted, which no flag could do — a
 * boolean saying *backed up* can never come back down. So the document's
 * URI is held with a lasting permission and [Backup.prompting] asks the
 * file, not a memory.
 */
class BackupActivity : Activity() {

    private companion object {
        /** The one request code this screen writes under. */
        const val WRITE = 7448

        /** Where the URI of the last backup is kept. */
        const val PREFS = "backup"
        const val KEY = "where"
    }

    private lateinit var narration: TextView
    private lateinit var body: LinearLayout

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val g = Screen.gutter(this)
        narration = TextView(this).apply {
            textSize = 15f
            setTextColor(Palette.now.ink)
        }
        body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(g, g, g, g)
            addView(narration)
        }
        val root = ScrollView(this).apply { addView(body) }
        Screen.insetting(root)
        setContentView(root)
        draw()
    }

    private fun draw() {
        body.removeAllViews()
        body.addView(narration)
        // **the author's words, from the table** (PRD-07): what losing the
        // device costs, fixed by the kind of thing this is
        narration.text = Backup.words().joinToString("\n\n")
        held()?.let { where ->
            body.addView(
                TextView(this).apply {
                    textSize = 13f
                    setTextColor(Palette.now.sub)
                    text = Backup.written(where)
                },
            )
        }
        body.addView(
            Button(this).apply {
                text = if (held() == null) "Save a backup" else "Save a newer backup"
                setOnClickListener { write() }
            },
        )
        // **the destroying door sits under the one that makes destruction
        // survivable** (PRD-03): a person reaching for a wipe passes the
        // backup on the way, and a device with no backup has been told
        // what that costs twice over by the time they get here
        body.addView(
            TextView(this).apply {
                textSize = 13f
                setTextColor(Palette.now.sub)
                text = if (held() == null) {
                    "No backup has been written from this device."
                } else {
                    "A backup exists, which is what makes the next door survivable."
                }
            },
        )
        body.addView(
            Button(this).apply {
                text = "Wipe this device"
                setOnClickListener {
                    startActivity(Intent(this@BackupActivity, WipeActivity::class.java))
                }
            },
        )
    }

    /** Where the last backup went, if this device wrote one and can still reach it. */
    private fun held(): String? {
        val kept = getSharedPreferences(PREFS, MODE_PRIVATE).getString(KEY, null) ?: return null
        return if (reaches(Uri.parse(kept))) kept else null
    }

    /**
     * **Whether the document is still there**, asked of the file.
     *
     * A person who deleted their backup, or revoked this app's hold on the
     * folder, is one PRD-10 says must be prompted again — and neither
     * event tells this app anything, so the only way to know is to look.
     */
    private fun reaches(uri: Uri): Boolean = runCatching {
        contentResolver.openInputStream(uri)?.use { true } ?: false
    }.getOrDefault(false)

    private fun write() {
        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("application/octet-stream")
            .putExtra(Intent.EXTRA_TITLE, Backup.fileName(System.currentTimeMillis()))
        runCatching {
            @Suppress("DEPRECATION")
            startActivityForResult(intent, WRITE)
        }.onFailure {
            narration.text = "This device offers nowhere to write it.\n\n${narration.text}"
        }
    }

    @Deprecated("the shell is on startActivityForResult throughout")
    override fun onActivityResult(request: Int, result: Int, data: Intent?) {
        @Suppress("DEPRECATION")
        super.onActivityResult(request, result, data)
        if (request != WRITE) return
        val uri = data?.data ?: return
        // **the blob is asked for only once there is somewhere to put it**:
        // exporting and then finding no destination would leave the whole
        // archive in this process's memory for no reason
        val blob = Kernel.exportBackup()
        if (blob == null) {
            narration.text = "This device's kernel did not answer, so nothing was written."
            return
        }
        val ok = runCatching {
            contentResolver.openOutputStream(uri)?.use { it.write(blob) } != null
        }.getOrDefault(false)
        if (!ok) {
            narration.text = "The file could not be written, so nothing was kept."
            return
        }
        // **a lasting hold on the document**, which is what lets this
        // device answer later whether the backup is still there (PRD-10)
        runCatching {
            contentResolver.takePersistableUriPermission(
                uri,
                Intent.FLAG_GRANT_READ_URI_PERMISSION,
            )
        }
        getSharedPreferences(PREFS, MODE_PRIVATE).edit()
            .putString(KEY, uri.toString()).apply()
        Diag.event("backup.written", "bytes" to blob.size)
        draw()
    }
}
