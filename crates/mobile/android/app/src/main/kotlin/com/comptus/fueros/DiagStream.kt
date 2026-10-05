package com.comptus.fueros

import java.io.BufferedOutputStream
import java.io.File
import java.io.IOException
import java.net.InetSocketAddress
import java.net.Socket
import java.util.ArrayDeque
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/**
 * The live stream: the same lines the event file takes, sent to the bench
 * over one TCP connection on the tester's own LAN (`rhtn diag collect` on
 * the laptop; `Robot/field-test-diagnostics.md`, M4b). The file remains the
 * record; this is the view of a run while it happens, and it is lossy by
 * design.
 *
 * **The kernel's thread never waits on the network.** [offer] puts the line
 * on a bounded backlog under a lock held for a few instructions and
 * returns. A full backlog drops its *oldest* line, since the newest is the
 * one the bench is waiting for, and the count of lines dropped is sent as a
 * `diag.dropped` event (`sink: stream`) at the place in the stream where
 * they are missing. One thread of its own connects, says hello, drains the
 * backlog to the socket, and on any failure closes, waits with a growing
 * backoff and connects again, saying hello again, for as long as the
 * process lives.
 *
 * **The hello** is the first line of every connection: a `diag.hello` event
 * whose fields are the bundle header's (`Report.headerFields`) plus the
 * serial the bench gave this phone, and `unix_ms`, the wall clock at the
 * line's `ms`, so the collector's file carries its own anchor and the merge
 * tool places it without help. The collector keeps one hello per run and
 * names the file by the serial.
 *
 * **No Android in this file**, so a JVM test can hold it against a
 * loopback server. The releasable flavour never instantiates it and never
 * reads an address: `FuerosApp` alone constructs one, in the fieldtest
 * flavour, from the target [store]d by the bench's provisioning step.
 */
class DiagStream(
    private val host: String,
    private val port: Int,
    /** The hello line, rendered afresh for each connection. */
    private val hello: () -> String,
    private val capacity: Int = BACKLOG_LINES,
    /** The waits between attempts, the last repeated. */
    private val backoffMs: LongArray = longArrayOf(1_000, 2_000, 5_000, 10_000, 30_000),
    private val connectTimeoutMs: Int = 3_000,
    /** Without the thread, for a test that fills the backlog first. */
    start: Boolean = true,
) {
    companion object {
        /** How many lines are held for a collector that is not connected. */
        const val BACKLOG_LINES = 4096
        /** The event a stream opens with, which anchors the collector's
         *  file to the wall clock. */
        const val EVENT_HELLO = "diag.hello"

        /** `host:port`, IPv6 in brackets, or null when it does not parse. */
        fun parseAddress(s: String): Pair<String, Int>? {
            val t = s.trim()
            val colon = t.lastIndexOf(':')
            if (colon <= 0 || colon == t.length - 1) return null
            val host = t.substring(0, colon).removePrefix("[").removeSuffix("]")
            val port = t.substring(colon + 1).toIntOrNull() ?: return null
            if (host.isEmpty() || port !in 1..65535) return null
            return host to port
        }

        /** Where the bench's target is kept between launches: two lines,
         *  the address and the serial, under the diag directory. */
        fun targetFile(diagDir: File): File = File(diagDir, "stream.target")

        /** Keep a target, or forget it when the address is blank or `off`. */
        fun store(diagDir: File, address: String, serial: String?) {
            val f = targetFile(diagDir)
            val a = address.trim()
            if (a.isEmpty() || a == "off") {
                f.delete()
                return
            }
            diagDir.mkdirs()
            f.writeText(a + "\n" + (serial ?: "").trim() + "\n")
        }

        /** The kept target, as (address, serial or null), or null. */
        fun stored(diagDir: File): Pair<String, String?>? {
            val f = targetFile(diagDir)
            if (!f.isFile) return null
            val lines = runCatching { f.readLines() }.getOrNull() ?: return null
            val address = lines.getOrNull(0)?.trim().orEmpty()
            if (address.isEmpty()) return null
            val serial = lines.getOrNull(1)?.trim()?.ifEmpty { null }
            return address to serial
        }
    }

    private val lock = ReentrantLock()
    private val more = lock.newCondition()
    private val backlog = ArrayDeque<String>(capacity)
    private var dropped = 0
    @Volatile private var closed = false
    @Volatile private var connections = 0
    @Volatile private var connected = false
    private var writer: Thread? = null

    init {
        if (start) start()
    }

    /** Begin connecting; once. */
    fun start() {
        lock.withLock {
            if (writer != null || closed) return
            writer = Thread({ pump() }, "fueros-diag-stream").apply {
                isDaemon = true
                start()
            }
        }
    }

    /** Queue one line. Never blocks; a full backlog drops its oldest. */
    fun offer(line: String) {
        if (closed) return
        lock.withLock {
            if (backlog.size >= capacity) {
                backlog.pollFirst()
                dropped++
            }
            backlog.addLast(line)
            more.signal()
        }
    }

    /** Lines dropped and not yet reported down the stream. */
    fun droppedPending(): Int = lock.withLock { dropped }

    /** Lines waiting to be sent. */
    fun backlogSize(): Int = lock.withLock { backlog.size }

    /** Connections made so far, for a test of the reconnect. */
    fun connections(): Int = connections

    /** Whether a connection is up and past its hello. */
    fun connected(): Boolean = connected

    /** Stop: the thread ends and the backlog is let go. */
    fun close() {
        closed = true
        lock.withLock {
            more.signalAll()
            writer?.interrupt()
        }
    }

    private fun pump() {
        var attempt = 0
        while (!closed) {
            try {
                Socket().use { s ->
                    s.connect(InetSocketAddress(host, port), connectTimeoutMs)
                    s.tcpNoDelay = true
                    connections++
                    attempt = 0
                    val out = BufferedOutputStream(s.getOutputStream(), 16 * 1024)
                    out.write((hello() + "\n").toByteArray())
                    out.flush()
                    // the bench sends nothing, so a read that ends is the
                    // bench gone: noticed at once rather than at the next
                    // write, which a closed socket may still accept
                    val dead = AtomicBoolean(false)
                    val input = s.getInputStream()
                    Thread({
                        try {
                            while (input.read() != -1) { /* nothing is expected */ }
                        } catch (_: Exception) {
                            // closed from this side, or failed: the same
                        }
                        dead.set(true)
                        runCatching { s.close() }
                        lock.withLock { more.signalAll() }
                    }, "fueros-diag-stream-eof").apply { isDaemon = true }.start()
                    connected = true
                    try {
                        drain(out, dead)
                    } finally {
                        connected = false
                    }
                }
            } catch (_: InterruptedException) {
                return
            } catch (_: IOException) {
                // the bench is away or gone; wait and try again
            } catch (_: Exception) {
                // an address that does not resolve, a refused connect
            }
            if (closed) return
            val wait = backoffMs[minOf(attempt, backoffMs.size - 1)]
            attempt++
            try {
                lock.withLock { more.await(wait, TimeUnit.MILLISECONDS) }
            } catch (_: InterruptedException) {
                return
            }
        }
    }

    /** Send lines as they come until the socket fails. A line taken off
     *  the backlog and not written goes back to its head, so a failure
     *  mid-write repeats at most that one line after the reconnect. */
    private fun drain(out: BufferedOutputStream, dead: AtomicBoolean) {
        while (!closed) {
            val line: String
            val lost: Int
            lock.withLock {
                while (backlog.isEmpty() && !closed && !dead.get()) more.await()
                if (closed) return
                if (dead.get()) throw IOException("closed by the bench")
                line = backlog.pollFirst()
                lost = dropped
                dropped = 0
            }
            try {
                if (lost > 0) {
                    val d = Diag.render(Diag.ms(), "warn", "shell", "diag.dropped", listOf("lines" to lost, "sink" to "stream"))
                    out.write((d + "\n").toByteArray())
                }
                out.write((line + "\n").toByteArray())
                if (lock.withLock { backlog.isEmpty() }) out.flush()
            } catch (e: IOException) {
                lock.withLock {
                    if (backlog.size < capacity) backlog.addFirst(line) else dropped++
                    dropped += lost
                }
                throw e
            }
        }
    }
}
