package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **The conversation store** (APP-015): per-message records appended,
 * sealed under a label naming their place, read backwards a page at a
 * time with delivery changes folded in; the index apart from the
 * messages; the 12th's file still readable for the migration.
 */
class ThreadsTest {
    private fun id(n: Int) = "%064x".format(n)

    /** Files in memory, with the appends and ranged reads a thread file needs. */
    private class Mem : Threads.Files {
        val held = HashMap<String, ByteArray>()
        override fun read(name: String) = held[name]
        override fun write(name: String, bytes: ByteArray): Boolean { held[name] = bytes; return true }
        override fun append(name: String, bytes: ByteArray): Boolean { held[name] = (held[name] ?: ByteArray(0)) + bytes; return true }
        override fun size(name: String) = (held[name]?.size ?: 0).toLong()
        override fun readAt(name: String, at: Long, len: Int): ByteArray? {
            val f = held[name] ?: return null
            if (at < 0 || at + len > f.size) return null
            return f.copyOfRange(at.toInt(), at.toInt() + len)
        }
    }

    /** A sealer that binds the label the way the kernel's does: another label does not open. */
    private class Bound : Threads.Sealer {
        override fun seal(label: String, bytes: ByteArray): ByteArray {
            val tag = label.hashCode()
            return byteArrayOf((tag ushr 24).toByte(), (tag ushr 16).toByte(), (tag ushr 8).toByte(), tag.toByte()) + bytes.map { (it.toInt() xor 0x5a).toByte() }.toByteArray()
        }
        override fun unseal(label: String, bytes: ByteArray): ByteArray? {
            if (bytes.size < 4) return null
            val tag = label.hashCode()
            val got = ((bytes[0].toInt() and 0xff) shl 24) or ((bytes[1].toInt() and 0xff) shl 16) or ((bytes[2].toInt() and 0xff) shl 8) or (bytes[3].toInt() and 0xff)
            if (got != tag) return null
            return bytes.copyOfRange(4, bytes.size).map { (it.toInt() xor 0x5a).toByte() }.toByteArray()
        }
    }

    @Test
    fun messages_append_and_read_back_newest_first_in_pages_with_delivery_changes_folded_in() {
        val files = Mem()
        val t = Threads(files, Bound(), id(1))
        val peer = id(2)
        val a = t.append(peer, Threads.Record.Said(true, "me", "first \"quoted\"\nline", Front.Delivery.SENDING, null))!!
        val b = t.append(peer, Threads.Record.Said(false, "MRK", "back", Front.Delivery.SENT, null))!!
        val c = t.append(peer, Threads.Record.Said(true, "me", "lost", Front.Delivery.SENDING, null))!!
        assertTrue(t.append(peer, Threads.Record.Settled(a, Front.Delivery.SENT, null)) != null)
        assertTrue(t.append(peer, Threads.Record.Settled(c, Front.Delivery.UNSENT, "nowhere")) != null)
        assertTrue("ids are offsets, so they rise", a < b && b < c)
        assertFalse("the file holds no plaintext", String(files.held[Threads.fileOf(peer)]!!, Charsets.ISO_8859_1).contains("back"))

        val newest = t.page(peer, null, 2)
        assertEquals(listOf(b, c), newest.messages.map { it.id })
        assertEquals("the change after the message is folded in", Front.Delivery.UNSENT, newest.messages[1].delivery)
        assertEquals("nowhere", newest.messages[1].reason)
        assertEquals(Front.Delivery.SENT, newest.messages[0].delivery)
        assertTrue("an earlier page remains", newest.before != null)
        assertEquals("a change whose message lies earlier is carried", setOf(a), newest.pending.keys)
        assertFalse(newest.broken)

        val earlier = t.page(peer, newest.before, 2, newest.pending)
        assertEquals(listOf(a), earlier.messages.map { it.id })
        assertEquals("folded in from the carried change", Front.Delivery.SENT, earlier.messages[0].delivery)
        assertEquals("first \"quoted\"\nline", earlier.messages[0].text)
        assertNull("the start was reached", earlier.before)
        assertTrue(earlier.pending.isEmpty())
    }

    @Test
    fun a_record_that_does_not_open_stops_the_walk_and_what_opened_is_returned() {
        val files = Mem()
        val t = Threads(files, Bound(), id(1))
        val peer = id(2)
        t.append(peer, Threads.Record.Said(true, "me", "one", Front.Delivery.SENT, null))
        val two = t.append(peer, Threads.Record.Said(true, "me", "two", Front.Delivery.SENT, null))!!
        val f = files.held[Threads.fileOf(peer)]!!
        f[two.toInt() + 6] = (f[two.toInt() + 6].toInt() xor 1).toByte()
        val p = t.page(peer, null, 10)
        assertTrue(p.broken)
        assertTrue(p.messages.isEmpty())
        assertNull(p.before)
        val moved = Threads(files, Bound(), id(1)).page(id(3), null, 10)
        assertTrue("a thread with no file is empty, not broken", moved.messages.isEmpty() && !moved.broken && moved.before == null)
    }

    @Test
    fun the_index_round_trips_sealed_and_a_bad_or_missing_index_is_no_threads() {
        val files = Mem()
        val t = Threads(files, Bound(), id(1))
        assertEquals(emptyList<Threads.Entry>(), t.index())
        val entries = listOf(Threads.Entry(id(2), "MRK", true), Threads.Entry(id(3), id(3), false))
        assertTrue(t.writeIndex(entries))
        assertEquals(entries, t.index())
        assertFalse(String(files.held[Threads.INDEX]!!, Charsets.ISO_8859_1).contains("MRK"))
        files.held[Threads.INDEX] = "not sealed".toByteArray()
        assertEquals(emptyList<Threads.Entry>(), t.index())
        assertEquals("another identity's index does not open", emptyList<Threads.Entry>(), Threads(files, Bound(), id(9)).also { it.writeIndex(entries) }.let { Threads(files, Bound(), id(1)).index() })
    }

    @Test
    fun the_file_of_the_twelfth_still_reads_for_the_migration() {
        val legacy = """[{"by":"${id(1)}","peer":"${id(2)}","name":"MRK","direct":true,"messages":[{"id":0,"mine":true,"who":"me","text":"hi","delivery":"SENT","reason":null},{"id":1,"mine":false,"who":"MRK","text":"back","delivery":"SENT","reason":null}]}]"""
        val t = Threads.decodeLegacy(legacy.toByteArray())
        assertEquals(1, t.size)
        assertEquals(2, t[0].messages.size)
        assertEquals("back", t[0].messages[1].text)
        assertEquals(emptyList<Front.Thread>(), Threads.decodeLegacy(null))
        assertEquals(emptyList<Front.Thread>(), Threads.decodeLegacy("not json".toByteArray()))
    }

    @Test
    fun the_shell_wires_the_store_per_identity_and_opens_a_thread_lazily() {
        val k = source("Kernel.kt")
        assertTrue("the store is not built per identity over the kernel's sealing", k.contains("Threads(shell.files(), sealerOf(p), by)"))
        assertTrue("the index is not restored per identity at bring-up", k.contains("front.restore(h.id, s.index())"))
        assertTrue("the old file is not migrated", k.contains("Threads.decodeLegacy("))
        val c = source("ConversationActivity.kt")
        assertTrue("a conversation does not open its thread lazily", c.contains("Kernel.front.open("))
        assertTrue("there is no way to read earlier messages", c.contains("Kernel.front.earlier("))
        val f = source("Front.kt")
        assertTrue("a message is not appended as it accumulates", f.contains("appender?.invoke("))
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
