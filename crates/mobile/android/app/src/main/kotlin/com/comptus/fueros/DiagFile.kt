package com.comptus.fueros

import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/**
 * The event file: `<dir>/<run>.jsonl`, appended by one thread of its own
 * (`Robot/field-test-diagnostics.md`, section 4).
 *
 * **The kernel's thread never waits on the disk.** [offer] puts the line
 * on a bounded queue and returns; a full queue drops the line and counts
 * it, and the count is written as a `diag.dropped` event when the writer
 * catches up, so a loss is visible in the file rather than silent. The
 * writer thread drains the queue to the file.
 *
 * **One file and one previous.** When the current file passes [cap] it is
 * renamed to `<run>.prev.jsonl`, replacing any earlier one, and a new
 * current begins: at most twice the cap on disk per run. [prune] at start
 * keeps the newest runs' files and deletes the rest, so a phone that has
 * run many sessions does not fill up.
 *
 * [flush] is for the crash handler: it drains the queue on the caller's
 * thread, under the writer's lock, and syncs the file before the process
 * dies.
 */
class DiagFile(
    private val dir: File,
    private val run: String,
    private val cap: Long = DEFAULT_CAP,
    capacity: Int = QUEUE_LINES,
    /** Without a writer thread, for a test that drains by [flush]. */
    start: Boolean = true,
) {
    companion object {
        const val DEFAULT_CAP = 16L * 1024 * 1024
        const val QUEUE_LINES = 4096
        private const val SUFFIX = ".jsonl"
        private const val PREV = ".prev"

        /** Delete every run's files but the newest `keepRuns` runs'. Run
         *  ids sort by time (seconds first), so the newest sort last. */
        fun prune(dir: File, keepRuns: Int) {
            val runs = dir.listFiles()
                ?.filter { it.name.endsWith(SUFFIX) }
                ?.map { it.name.removeSuffix(SUFFIX).removeSuffix(PREV) }
                ?.toSortedSet()
                ?: return
            val stale = runs.toList().dropLast(keepRuns)
            for (r in stale) {
                File(dir, "$r$SUFFIX").delete()
                File(dir, "$r$PREV$SUFFIX").delete()
            }
        }
    }

    private val queue = ArrayBlockingQueue<String>(capacity)
    private val dropped = AtomicInteger(0)
    private val writeLock = Any()
    private var out: FileOutputStream? = null
    private var written: Long = 0
    @Volatile private var closed = false

    /** The current file. */
    val current: File = File(dir, "$run$SUFFIX")

    /** The rotated-out file, present once the cap was passed. */
    val previous: File = File(dir, "$run$PREV$SUFFIX")

    private val writer: Thread? = if (start) {
        Thread({ pump() }, "fueros-diag").apply {
            isDaemon = true
            start()
        }
    } else {
        null
    }

    /** Queue one line. Never blocks; a full queue drops and counts. */
    fun offer(line: String) {
        if (closed) return
        if (!queue.offer(line)) dropped.incrementAndGet()
    }

    /** Lines dropped for a full queue and not yet reported to the file. */
    fun droppedPending(): Int = dropped.get()

    /** The files that exist, current first. */
    fun files(): List<File> = listOf(current, previous).filter { it.isFile }

    /** Drain everything queued to disk on this thread and sync it. */
    fun flush() {
        synchronized(writeLock) {
            drain()
            runCatching { out?.fd?.sync() }
        }
    }

    /** Stop the writer, after a last drain. */
    fun close() {
        closed = true
        writer?.interrupt()
        flush()
        synchronized(writeLock) {
            runCatching { out?.close() }
            out = null
        }
    }

    private fun pump() {
        try {
            while (!closed) {
                val first = queue.poll(500, TimeUnit.MILLISECONDS) ?: continue
                synchronized(writeLock) {
                    write(first)
                    drain()
                }
            }
        } catch (_: InterruptedException) {
            // closing: the final flush is the closer's
        }
    }

    /** Under [writeLock]: everything queued, then the drop count if any. */
    private fun drain() {
        while (true) {
            val line = queue.poll() ?: break
            write(line)
        }
        val lost = dropped.getAndSet(0)
        if (lost > 0) {
            write(Diag.render(Diag.ms(), "warn", "shell", "diag.dropped", listOf("lines" to lost)))
        }
    }

    /** Under [writeLock]: one line to the file, rotating past the cap. */
    private fun write(line: String) {
        try {
            val o = out ?: open()
            val bytes = (line + "\n").toByteArray()
            o.write(bytes)
            written += bytes.size
            if (written > cap) rotate()
        } catch (_: Exception) {
            // a disk that refuses is a diagnostic that cannot be written
            // anywhere; the ceremony is not stopped for it
        }
    }

    private fun open(): FileOutputStream {
        dir.mkdirs()
        written = if (current.isFile) current.length() else 0
        return FileOutputStream(current, true).also { out = it }
    }

    private fun rotate() {
        runCatching { out?.close() }
        out = null
        previous.delete()
        current.renameTo(previous)
        written = 0
    }
}
