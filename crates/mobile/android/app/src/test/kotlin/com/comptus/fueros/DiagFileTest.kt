package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/**
 * The event file's writer: lines land in order, a full queue drops and
 * says so, the cap rotates to one previous file, and old runs are pruned.
 */
class DiagFileTest {

    @get:Rule
    val tmp = TemporaryFolder()

    private fun lines(f: File) = if (f.isFile) f.readLines() else listOf()

    @Test
    fun queued_lines_reach_the_file_in_order() {
        val dir = tmp.newFolder("diag")
        val f = DiagFile(dir, "run-a")
        for (i in 0 until 500) f.offer("""{"n":$i}""")
        f.flush()
        assertEquals((0 until 500).map { """{"n":$it}""" }, lines(f.current))
        assertEquals(listOf(f.current), f.files())
        f.close()
    }

    @Test
    fun a_full_queue_drops_and_the_count_is_written_when_it_drains() {
        val dir = tmp.newFolder("diag")
        // no writer thread: the queue fills deterministically
        val f = DiagFile(dir, "run-b", capacity = 8, start = false)
        for (i in 0 until 13) f.offer("line $i")
        assertEquals(5, f.droppedPending())
        f.flush()
        val got = lines(f.current)
        assertEquals(9, got.size)
        assertEquals((0 until 8).map { "line $it" }, got.take(8))
        assertTrue(got[8], got[8].contains("\"event\":\"diag.dropped\",\"lines\":5}"))
        assertEquals(0, f.droppedPending())
        // a second flush writes nothing new
        f.flush()
        assertEquals(9, lines(f.current).size)
    }

    @Test
    fun past_the_cap_the_file_rotates_to_one_previous() {
        val dir = tmp.newFolder("diag")
        val line = "x".repeat(99)
        val f = DiagFile(dir, "run-c", cap = 1000, start = false)
        // 10 lines of 100 bytes reach the cap; the 11th passes it and
        // rotates, so the previous holds eleven and the current the rest
        for (i in 0 until 15) f.offer(line)
        f.flush()
        assertTrue(f.previous.isFile)
        assertTrue(f.current.isFile)
        assertEquals(11, lines(f.previous).size)
        assertEquals(4, lines(f.current).size)
        assertEquals(listOf(f.current, f.previous), f.files())
        // the previous is replaced on each rotation, never a third kept:
        // twice the cap on disk at most. 41 more lines rotate three times
        // (7, 11, 11) and leave one line in a fresh current
        for (i in 0 until 41) f.offer(line)
        f.flush()
        assertEquals(2, dir.listFiles()!!.size)
        assertEquals(11, lines(f.previous).size)
        assertEquals(1, lines(f.current).size)
    }

    @Test
    fun the_writer_thread_drains_without_a_flush() {
        val dir = tmp.newFolder("diag")
        val f = DiagFile(dir, "run-d")
        f.offer("one")
        val deadline = System.currentTimeMillis() + 5_000
        while (lines(f.current).isEmpty() && System.currentTimeMillis() < deadline) Thread.sleep(20)
        assertEquals(listOf("one"), lines(f.current))
        f.close()
    }

    @Test
    fun prune_keeps_the_newest_runs() {
        val dir = tmp.newFolder("diag")
        for (r in listOf("100-aa", "200-bb", "300-cc", "400-dd")) {
            File(dir, "$r.jsonl").writeText("x")
            File(dir, "$r.prev.jsonl").writeText("x")
        }
        File(dir, "other.txt").writeText("x")
        DiagFile.prune(dir, keepRuns = 2)
        val left = dir.list()!!.sorted()
        assertEquals(
            listOf("300-cc.jsonl", "300-cc.prev.jsonl", "400-dd.jsonl", "400-dd.prev.jsonl", "other.txt"),
            left,
        )
        assertFalse(File(dir, "100-aa.jsonl").exists())
    }
}
