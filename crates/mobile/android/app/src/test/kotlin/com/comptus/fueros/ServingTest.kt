package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **A serving record is a list of tabs, one per address** [author,
 * 2026-11-11], and the single pair it once was still reads.
 */
class ServingTest {
    private fun id(n: Int) = "%064x".format(n)

    @Test
    fun the_older_single_record_reads_as_one_tab() {
        val old = "{\"node\":\"${id(1)}\",\"addr\":\"192.168.0.8:7447\"}".toByteArray()
        assertEquals(listOf(Serving.Tab(id(1), "192.168.0.8:7447")), Serving.read(old))
        assertEquals(emptyList<Serving.Tab>(), Serving.read(null))
        assertEquals(emptyList<Serving.Tab>(), Serving.read("not json".toByteArray()))
    }

    @Test
    fun a_tab_added_replaces_the_entry_for_its_node_and_keeps_the_rest() {
        val one = Serving.with(null, Serving.Tab(id(1), "10.0.0.1:7447"))
        val two = Serving.with(one, Serving.Tab(id(2), "10.0.0.2:7447"))
        assertEquals(2, Serving.read(two).size)
        val moved = Serving.with(two, Serving.Tab(id(1), "10.0.0.9:7447"))
        assertEquals(
            listOf(Serving.Tab(id(2), "10.0.0.2:7447"), Serving.Tab(id(1), "10.0.0.9:7447")),
            Serving.read(moved),
        )
        assertEquals(listOf(Serving.Tab(id(1), "10.0.0.9:7447")), Serving.read(Serving.without(moved, id(2))))
    }

    @Test
    fun every_tab_is_attached_and_the_first_is_the_connection_shown() {
        val k = source("Kernel.kt")
        assertTrue(
            "serveFrom does not add a tab to the identity's record",
            k.contains("Serving.with(shell.read(SERVING), Serving.Tab(nodeHex, address))"),
        )
        assertTrue(
            "a held identity does not attach to each of its tabs",
            k.contains("Serving.read(AndroidShell(a, scope).read(SERVING))") &&
                k.contains("Model.connectionOf(id, first ?: Connection.Detached)"),
        )
        assertTrue(
            "the focused identity does not attach to the tabs it agreed to beside the provision's node",
            k.contains("if (t.node == nodeHex) continue"),
        )
        // **a session lost is attached again with backoff** [bench, 2026-11-13]
        assertTrue(
            "a lost session is not tried again",
            k.contains("if (e.v1 is Connection.Lost) {") && k.contains("reattachLater(id, p, ") &&
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
