package com.comptus.fueros

import android.app.Activity
import android.app.AlertDialog
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicBoolean

/**
 * The person's answer to a yes/no the kernel puts to them
 * (`light-client-requirements.md` §1.4: consent is what they said, never
 * what the shell assumed). The kernel asks on its own thread; this posts a
 * dialog to whatever screen is foreground and blocks until the person
 * answers.
 *
 * **A question nobody is shown is answered no.** With no foreground screen
 * to host the dialog, the honest answer is a refusal — the shell must never
 * consent on the person's behalf.
 */
object Consent {

    @Volatile private var host: Activity? = null

    /** Put questions to `a` from now on. */
    fun host(a: Activity) {
        host = a
    }

    /** Stop putting questions to `a`, where it is still the host. */
    fun release(a: Activity) {
        if (host === a) host = null
    }

    /** Put `question` to the person and wait for their answer.  No
     *  foreground screen means no, as nobody was shown it. */
    fun ask(question: String): Boolean {
        val a = host ?: return false
        val latch = CountDownLatch(1)
        val answer = AtomicBoolean(false)
        a.runOnUiThread {
            AlertDialog.Builder(a)
                .setMessage(question)
                .setCancelable(true)
                .setPositiveButton("Yes") { _, _ -> answer.set(true); latch.countDown() }
                .setNegativeButton("No") { _, _ -> answer.set(false); latch.countDown() }
                .setOnCancelListener { latch.countDown() }
                .show()
        }
        latch.await()
        return answer.get()
    }
}
