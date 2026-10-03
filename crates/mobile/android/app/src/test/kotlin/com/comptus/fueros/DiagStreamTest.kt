package com.comptus.fueros

import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.ServerSocket
import java.net.Socket
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.rules.Timeout

/**
 * The live stream against a loopback server: the hello first on every
 * connection, lines in order, a full backlog dropping its oldest and
 * saying so in the stream, the reconnect after the bench closes, and the
 * target kept between launches.
 */
class DiagStreamTest {

    @get:Rule
    val tmp = TemporaryFolder()

    /** A stream that never answers must fail the test, not hold the suite. */
    @get:Rule
    val timeout: Timeout = Timeout.seconds(30)

    private val hello = { """{"ms":1,"level":"info","layer":"shell","event":"diag.hello","run":"r"}""" }

    private fun await(what: () -> Boolean) {
        val deadline = System.currentTimeMillis() + 5_000
        while (!what() && System.currentTimeMillis() < deadline) Thread.sleep(5)
        assertTrue("not within 5 s", what())
    }

    private fun reader(s: Socket): BufferedReader {
        s.soTimeout = 5_000
        return BufferedReader(InputStreamReader(s.getInputStream()))
    }

    @Test
    fun the_hello_is_the_first_line_and_the_rest_follow_in_order() {
        ServerSocket(0, 1, java.net.InetAddress.getLoopbackAddress()).use { server ->
            server.soTimeout = 5_000
            val stream = DiagStream("127.0.0.1", server.localPort, hello, backoffMs = longArrayOf(50))
            try {
                server.accept().use { c ->
                    val r = reader(c)
                    assertEquals(hello(), r.readLine())
                    for (i in 0 until 50) stream.offer("""{"n":$i}""")
                    for (i in 0 until 50) assertEquals("""{"n":$i}""", r.readLine())
                }
            } finally {
                stream.close()
            }
        }
    }

    @Test
    fun a_full_backlog_drops_its_oldest_and_the_count_is_sent_where_they_are_missing() {
        ServerSocket(0, 1, java.net.InetAddress.getLoopbackAddress()).use { server ->
            server.soTimeout = 5_000
            // no thread yet: the backlog fills deterministically
            val stream = DiagStream("127.0.0.1", server.localPort, hello, capacity = 8, backoffMs = longArrayOf(50), start = false)
            for (i in 0 until 13) stream.offer("line $i")
            assertEquals(5, stream.droppedPending())
            assertEquals(8, stream.backlogSize())
            stream.start()
            try {
                server.accept().use { c ->
                    val r = reader(c)
                    assertEquals(hello(), r.readLine())
                    val dropped = r.readLine()
                    assertTrue(dropped, dropped.contains("\"event\":\"diag.dropped\",\"lines\":5,\"sink\":\"stream\"}"))
                    for (i in 5 until 13) assertEquals("line $i", r.readLine())
                    assertEquals(0, stream.droppedPending())
                    // nothing more is pending, so a new line comes through alone
                    stream.offer("after")
                    assertEquals("after", r.readLine())
                }
            } finally {
                stream.close()
            }
        }
    }

    @Test
    fun after_the_bench_closes_the_stream_reconnects_and_says_hello_again() {
        ServerSocket(0, 1, java.net.InetAddress.getLoopbackAddress()).use { server ->
            server.soTimeout = 5_000
            val stream = DiagStream("127.0.0.1", server.localPort, hello, backoffMs = longArrayOf(50))
            try {
                server.accept().use { c ->
                    assertEquals(hello(), reader(c).readLine())
                    await { stream.connected() }
                }
                // the bench is gone, and the stream notices without a write
                await { !stream.connected() }
                // the next line waits in the backlog for the reconnect
                stream.offer("while away")
                server.accept().use { c ->
                    val r = reader(c)
                    assertEquals(hello(), r.readLine())
                    assertEquals("while away", r.readLine())
                }
                assertEquals(2, stream.connections())
            } finally {
                stream.close()
            }
        }
    }

    @Test
    fun the_target_is_parsed_kept_and_forgotten() {
        assertEquals("192.168.1.7" to 7448, DiagStream.parseAddress("192.168.1.7:7448"))
        assertEquals("fe80::1" to 7448, DiagStream.parseAddress("[fe80::1]:7448"))
        assertNull(DiagStream.parseAddress("192.168.1.7"))
        assertNull(DiagStream.parseAddress(":7448"))
        assertNull(DiagStream.parseAddress("host:0"))
        assertNull(DiagStream.parseAddress("host:port"))

        val dir = tmp.newFolder("diag")
        assertNull(DiagStream.stored(dir))
        DiagStream.store(dir, "10.0.0.2:7448", "R58M1234")
        assertEquals("10.0.0.2:7448" to "R58M1234", DiagStream.stored(dir))
        DiagStream.store(dir, "10.0.0.2:7448", null)
        assertEquals("10.0.0.2:7448" to null, DiagStream.stored(dir))
        DiagStream.store(dir, "off", "R58M1234")
        assertNull(DiagStream.stored(dir))
        assertTrue(!DiagStream.targetFile(dir).exists())
    }
}
