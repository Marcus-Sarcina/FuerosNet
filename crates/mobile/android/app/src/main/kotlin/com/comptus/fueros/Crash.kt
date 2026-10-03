package com.comptus.fueros

/**
 * The Kotlin side's crash handler (`Robot/field-test-diagnostics.md`,
 * section 3.6, the `crash` row): the exception's class, message and top
 * frames as one event, the file flushed, then the exception handed to
 * whatever handler was there before, which is what kills the process. The
 * Rust side's panic hook is in `crates/ffi/src/diag.rs`.
 *
 * Installed by `FuerosApp` in the fieldtest flavour alone: in the
 * releasable flavour there is no file to flush and nothing to write.
 */
object Crash {

    const val FRAMES = 12

    /** The handler as it would be installed, given the one it chains to.
     *  Separate from [install] so a test can call it without dying. */
    fun handler(previous: Thread.UncaughtExceptionHandler?): Thread.UncaughtExceptionHandler =
        Thread.UncaughtExceptionHandler { thread, e ->
            try {
                Diag.warn(
                    "crash",
                    "thread" to thread.name,
                    "class" to e.javaClass.name,
                    "message" to e.message?.let { Diag.scrub(it) },
                    "frames" to frames(e),
                    "cause" to e.cause?.javaClass?.name,
                )
                Diag.flush()
            } catch (_: Throwable) {
                // a handler that throws loses the crash it was recording
            }
            previous?.uncaughtException(thread, e)
        }

    fun install() {
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler(handler(previous))
    }

    /** The top frames, `class.method(file:line)`. */
    fun frames(e: Throwable): List<String> =
        e.stackTrace.take(FRAMES).map {
            "${it.className}.${it.methodName}(${it.fileName}:${it.lineNumber})"
        }
}
