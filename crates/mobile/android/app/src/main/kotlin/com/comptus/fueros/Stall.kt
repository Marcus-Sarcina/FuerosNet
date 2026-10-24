package com.comptus.fueros

import android.os.Handler
import android.os.Looper
import java.util.concurrent.atomic.AtomicLong

/**
 * **A watchdog on the main thread**, for the field-test build: a tick is
 * posted to the main looper twice a second, and a tick that has not run
 * within a second is a stall — said once with the main thread's stack at
 * that moment, and again when it ends with how long it held. The screen
 * freezing while the code stopped cycling at the optical exchange was
 * invisible to every other line of the diagnostics (run 1bd1c15-direct-1,
 * 2026-10-24); this is the line that names the frame that held it.
 *
 * Started only where [Diag] has a sink, so a releasable build runs no such
 * thread.
 */
object Stall {
    private const val TICK_MS = 500L
    private const val STALL_MS = 1_000L
    private const val GIVE_UP_MS = 120_000L
    @Volatile private var started = false

    fun start() {
        synchronized(this) {
            if (started) return
            started = true
        }
        val main = Handler(Looper.getMainLooper())
        val ran = AtomicLong(0L)
        val ack = Runnable { ran.set(System.currentTimeMillis()) }
        Thread({
            while (true) {
                val posted = System.currentTimeMillis()
                ran.set(0L)
                main.post(ack)
                Thread.sleep(TICK_MS)
                var said = false
                var waited = TICK_MS
                while (ran.get() == 0L && waited < GIVE_UP_MS) {
                    if (!said && waited >= STALL_MS) {
                        said = true
                        val stack = Looper.getMainLooper().thread.stackTrace
                            .take(12)
                            .joinToString(" < ") { "${it.className.substringAfterLast('.')}.${it.methodName}:${it.lineNumber}" }
                        Diag.warn("shell.stall", "waited_ms" to waited, "stack" to stack)
                    }
                    Thread.sleep(100)
                    waited += 100
                }
                if (said) {
                    val held = ran.get().let { if (it == 0L) waited else it - posted }
                    Diag.event("shell.stall", "state" to "over", "held_ms" to held)
                }
            }
        }, "fueros-stall").apply { isDaemon = true }.start()
    }
}
