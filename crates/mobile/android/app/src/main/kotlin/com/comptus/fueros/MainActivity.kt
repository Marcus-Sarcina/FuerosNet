package com.comptus.fueros

import android.app.Activity
import android.os.Bundle
import android.widget.TextView
import java.security.SecureRandom
import uniffi.rhtn_ffi.Participant
import uniffi.rhtn_ffi.Refused

/**
 * The skeleton's one screen: start a participant over the binding and show
 * what it presents.  The seeds are minted on first launch and kept in the
 * shell's storage until the ceremony exists to replace this arrangement;
 * a phone that is a delegated device of another identity starts
 * differently, and none of that is this slice.
 */
class MainActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val view = TextView(this).apply {
            textSize = 16f
            setPadding(48, 96, 48, 48)
            text = "starting the kernel…"
        }
        setContentView(view)

        Thread {
            val line = try {
                val p = Participant.start(seeds(), listOf(), platformOf(this))
                val key = p.presentedKey().joinToString("") { "%02x".format(it) }
                val status = p.status()
                "kernel up\n\nstatus: $status\npresents: ${key.take(16)}…"
            } catch (e: Refused.Reason) {
                "kernel refused to start:\n${e.reason}"
            }
            runOnUiThread { view.text = line }
        }.start()
    }

    private fun seeds(): ByteArray {
        val f = java.io.File(java.io.File(filesDir, "kernel"), "seeds")
        if (f.isFile) return f.readBytes()
        val s = ByteArray(64).also { SecureRandom().nextBytes(it) }
        f.parentFile?.mkdirs()
        f.writeBytes(s)
        return s
    }
}
