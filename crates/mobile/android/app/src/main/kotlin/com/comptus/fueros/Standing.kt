package com.comptus.fueros

/**
 * **The standing a platform gives a process that must keep a connection**
 * (`light-client-requirements.md` §4.1, SES-020) [author, 2026-11-14]: on
 * Android a foreground service and the notification it costs, so sessions
 * hold while the person is not looking. **And the client works without
 * it**: the platform, a vendor's build or a utility the person installed
 * may refuse that standing — *"it seems like the kind of thing a mobile OS
 * vendor or performance utility might choose to block in the future"* —
 * and a client so refused runs the same, its sessions re-attached by the
 * kernel's own retry chains the moment it is visible again. Both states
 * are tested here, not the first alone: the seam is [take], where what
 * the platform does is a function handed in.
 */
object Standing {
    /** What asking for the standing came to. */
    sealed class Outcome {
        /** The platform granted it. */
        object Granted : Outcome()

        /** The platform refused it, in its own words; nothing else changes. */
        data class Refused(val why: String) : Outcome()
    }

    @Volatile private var last: Outcome? = null

    /** The last outcome, or null where the standing was never asked for. */
    fun state(): Outcome? = last

    /**
     * Ask for the standing by running `start`, which is the platform's own
     * act (starting the service, declaring it foreground) and may throw
     * for a refusal: `ForegroundServiceStartNotAllowedException`, a
     * `SecurityException` for a permission withheld, an
     * `IllegalStateException` for a start from the background. **A refusal
     * is an outcome, never a failure**: it is recorded, said in the
     * diagnostics, and the caller carries on exactly as it would have.
     */
    fun take(start: () -> Unit): Outcome {
        val out = try {
            start()
            Outcome.Granted
        } catch (e: Exception) {
            Outcome.Refused(e.javaClass.simpleName + (e.message?.let { ": $it" } ?: ""))
        }
        last = out
        Diag.event(
            "standing",
            "state" to if (out is Outcome.Granted) "granted" else "refused",
            "why" to (out as? Outcome.Refused)?.why,
        )
        return out
    }

    /** Forget the last outcome, for a test that asks again. */
    fun reset() {
        last = null
    }
}
