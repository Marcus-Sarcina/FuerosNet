package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **The nodes an identity agreed to are the kernel's** [review, 2026-11-13];
 * the shell's record is read for its import alone, in both forms it took.
 */
class ServingTest {
    private fun id(n: Int) = "%064x".format(n)

    @Test
    fun both_forms_of_the_older_record_read() {
        val old = "{\"node\":\"${id(1)}\",\"addr\":\"192.168.0.8:7447\"}".toByteArray()
        assertEquals(listOf(Serving.Tab(id(1), "192.168.0.8:7447")), Serving.read(old))
        val list = "{\"tabs\":[{\"node\":\"${id(1)}\",\"addr\":\"a:1\"},{\"node\":\"${id(2)}\",\"addr\":\"b:2\"}]}".toByteArray()
        assertEquals(2, Serving.read(list).size)
        assertEquals(emptyList<Serving.Tab>(), Serving.read(null))
        assertEquals(emptyList<Serving.Tab>(), Serving.read("not json".toByteArray()))
    }

    @Test
    fun the_kernel_keeps_the_agreement_and_the_shell_attaches_what_it_holds() {
        val k = source("Kernel.kt")
        assertTrue("serveFrom does not hand the agreement to the kernel", k.contains("p.agreeTo(node, listOf(address))"))
        assertTrue("the shell still writes its own serving record", !k.contains("shell.write(SERVING"))
        assertTrue(
            "a held identity does not attach to each node the kernel holds",
            k.contains("val tabs = call(\"agreed\", { emptyList() }) { p.agreed() }") &&
                k.contains("Model.connectionOf(id, first ?: Connection.Detached)"),
        )
        assertTrue(
            "the focused identity does not attach to the agreed nodes beside the provision's",
            k.contains("if (hex == nodeHex) continue"),
        )
        assertTrue(
            "the older record is not imported once and forgotten",
            k.contains("private fun importServingRecord(p: Participant, scope: String?)") && k.contains("shell.forget(SERVING)"),
        )
        // **an attach that never held is tried again with backoff** [bench, 2026-11-13]
        assertTrue(
            "an attach that got no answer is not tried again",
            !k.contains("if (e.v1 is Connection.Lost) {") &&
                k.contains("if (missed > 0) reattachLater(id, p, scope, attempt + 1)") &&
                k.contains("longArrayOf(15, 30, 60, 120, 300)"),
        )
    }

    private fun source(name: String): String {
        val f = java.io.File(module(), "app/src/main/kotlin/com/comptus/fueros/$name")
        return f.readText()
            .replace(Regex("""/\*[\s\S]*?\*/"""), " ")
            .lines()
            .joinToString("\n") { it.substringBefore("//") }
    }

    private fun module(): java.io.File {
        var at: java.io.File? = java.io.File(".").canonicalFile
        while (at != null) {
            if (java.io.File(at, "app/src/main/kotlin/com/comptus/fueros/Kernel.kt").exists()) return at
            at = at.parentFile
        }
        error("the android module was not found above ${java.io.File(".").canonicalFile}")
    }
}
