package com.comptus.fueros

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.OpenableColumns
import java.io.File
import java.io.FileNotFoundException

/**
 * Serves the one directory `Report` writes to, read-only, to the
 * application the tester shares the bundle with. The same job as
 * androidx's `FileProvider`, without the dependency: one directory, one
 * kind of file, no writes. Declared in the fieldtest manifest alone, not
 * exported, with per-URI grants.
 */
class ReportProvider : ContentProvider() {

    companion object {
        /** This provider's authority, which the manifest declares too. */
        fun authority(context: Context) = "${context.packageName}.report"

        /** The content URI for `file`, to grant another app read of. */
        fun uriFor(context: Context, file: File): Uri =
            Uri.Builder().scheme("content").authority(authority(context)).path(file.name).build()
    }

    override fun onCreate(): Boolean = true

    private fun resolve(uri: Uri): File {
        val ctx = context ?: throw FileNotFoundException(uri.toString())
        val dir = File(File(ctx.filesDir, "diag"), Report.DIR)
        val name = uri.lastPathSegment ?: throw FileNotFoundException(uri.toString())
        val f = File(dir, name)
        // the name is a path segment, never a path: anything that resolves
        // outside the one directory is not served
        if (f.parentFile?.canonicalFile != dir.canonicalFile || !f.isFile) {
            throw FileNotFoundException(uri.toString())
        }
        return f
    }

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        if (mode != "r") throw FileNotFoundException("read-only")
        return ParcelFileDescriptor.open(resolve(uri), ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun query(
        uri: Uri,
        projection: Array<out String>?,
        selection: String?,
        selectionArgs: Array<out String>?,
        sortOrder: String?,
    ): Cursor {
        val f = resolve(uri)
        val cols = projection ?: arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE)
        val cursor = MatrixCursor(cols, 1)
        cursor.addRow(
            cols.map {
                when (it) {
                    OpenableColumns.DISPLAY_NAME -> f.name
                    OpenableColumns.SIZE -> f.length()
                    else -> null
                }
            },
        )
        return cursor
    }

    override fun getType(uri: Uri): String = "application/zip"

    override fun insert(uri: Uri, values: ContentValues?): Uri? = null

    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0

    override fun update(
        uri: Uri,
        values: ContentValues?,
        selection: String?,
        selectionArgs: Array<out String>?,
    ): Int = 0
}
