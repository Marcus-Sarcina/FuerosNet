package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** **An identity's conversations survive a restart** [author, 2026-11-12]. */
class ThreadsTest {
    private fun id(n: Int) = "%064x".format(n)

    @Test
    fun threads_round_trip_with_every_field_and_a_bad_file_is_no_threads() {
        val t = listOf(
            Front.Thread(
                id(1), id(2), "MRK", true,
                listOf(
                    Front.Message(0, true, "me", "first \"quoted\"\nline", Front.Delivery.SENT, null),
                    Front.Message(1, false, "MRK", "back", Front.Delivery.SENT, null),
                    Front.Message(2, true, "me", "lost", Front.Delivery.UNSENT, "nowhere"),
                ),
            ),
            Front.Thread(id(1), id(3), id(3), false, emptyList()),
        )
        assertEquals(t, Threads.decode(Threads.encode(t)))
        assertEquals(emptyList<Front.Thread>(), Threads.decode(null))
        assertEquals(emptyList<Front.Thread>(), Threads.decode("not json".toByteArray()))
    }

    @Test
    fun the_shell_restores_each_identitys_threads_and_writes_them_after_a_change() {
        val k = source("Kernel.kt")
        assertTrue("threads are not restored per identity at bring-up", k.contains("front.restore(Threads.decode(runCatching { AndroidShell(a, h.scope).read(THREADS) }.getOrNull()))"))
        assertTrue("threads are not written back to the identity's scope", k.contains("AndroidShell(a, scope).write(THREADS, Threads.encode(mine))"))
        val f = source("Front.kt")
        assertEquals("a change does not name the identity it belongs to", 5, Regex("""changed\(by\)""").findAll(f).count())
        assertTrue("a conversation opened without a row falls back to 'the peer'", !source("ConversationActivity.kt").contains("Kernel.peerKey()") && !source("Kernel.kt").contains("fun peerKey(): String?"))
        assertTrue("a row is not named by the identity that holds it", source("ConversationsActivity.kt").contains("Kernel.aliasOfHeld(t.by, t.peerKey)") && source("ConversationActivity.kt").contains("Kernel.aliasOfHeld(it.by, it.peerKey)"))
    }

    private fun source(name: String): String {
        val f = java.io.File(module(), "app/src/main/kotlin/com/comptus/fueros/$name")
        return f.readText().replace(Regex("""/\*[\s\S]*?\*/"""), " ").lines().joinToString("\n") { it.substringBefore("//") }
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
