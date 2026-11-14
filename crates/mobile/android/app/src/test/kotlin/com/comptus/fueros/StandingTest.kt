package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **Both states, not the first alone** [author, 2026-11-14] (SES-020): the
 * standing granted, and the standing refused by the platform, a vendor's
 * build or a utility — a client so refused runs the same.
 */
// functional: SES-020
class StandingTest {
    @Test
    fun the_standing_granted_is_an_outcome_and_the_standing_refused_is_one_too_never_a_failure() {
        Standing.reset()
        assertNull("nothing asked yet", Standing.state())
        var ran = 0
        assertEquals(Standing.Outcome.Granted, Standing.take { ran += 1 })
        assertEquals(1, ran)
        assertEquals(Standing.Outcome.Granted, Standing.state())

        // the platform's refusal, in its own words: recorded, and nothing thrown
        val refused = Standing.take { throw IllegalStateException("Not allowed to start service: app is in background") }
        assertTrue(refused is Standing.Outcome.Refused)
        assertTrue((refused as Standing.Outcome.Refused).why.contains("app is in background"))
        assertEquals(refused, Standing.state())
        // and a permission withheld is the same kind of outcome
        val withheld = Standing.take { throw SecurityException("FOREGROUND_SERVICE_REMOTE_MESSAGING") }
        assertTrue(withheld is Standing.Outcome.Refused)
        // asked again later, it may be granted: the refusal is not sticky
        assertEquals(Standing.Outcome.Granted, Standing.take { })
        Standing.reset()
    }

    // functional: APP-008
    @Test
    fun the_kernel_starts_the_same_whether_the_standing_is_granted_or_refused() {
        val a = source("Attachments.kt")
        val start = a.substring(a.indexOf("fun start(context: Context)"))
        val thread = start.indexOf("Thread({ bringUp(AndroidShell(app)) }")
        val standing = start.indexOf("KernelService.request(app)")
        assertTrue("the kernel's bring-up starts before the standing is asked for", thread in 0 until standing)
        val between = start.substring(standing, start.indexOf("}", standing))
        assertTrue("nothing waits on, returns on or reads the outcome in the kernel's start", !between.contains("Standing.state()") && !between.contains("return"))
        val svc = source("KernelService.kt")
        assertTrue("the service's own start is taken through the seam, so its refusal is an outcome", svc.contains("Standing.take {") && svc.contains("startForeground(ID, built, ServiceInfo.FOREGROUND_SERVICE_TYPE_REMOTE_MESSAGING)"))
        assertTrue("the request is taken through the seam too", svc.contains("Standing.take {\n                context.startForegroundService("))
    }

    @Test
    fun the_manifest_declares_the_service_and_the_permissions_it_costs() {
        val m = File(module(), "app/src/main/AndroidManifest.xml").readText()
        assertTrue(m.contains("android:name=\".KernelService\""))
        assertTrue(m.contains("android:foregroundServiceType=\"remoteMessaging\""))
        assertTrue(m.contains("android.permission.FOREGROUND_SERVICE\""))
        assertTrue(m.contains("android.permission.FOREGROUND_SERVICE_REMOTE_MESSAGING"))
    }

    private fun source(name: String): String =
        File(module(), "app/src/main/kotlin/com/comptus/fueros/$name").readText()

    private fun module(): File {
        var at: File? = File(".").canonicalFile
        while (at != null) {
            if (File(at, "app/src/main/AndroidManifest.xml").isFile) return at
            at = at.parentFile
        }
        error("the android module was not found above ${File(".").canonicalFile}")
    }
}
