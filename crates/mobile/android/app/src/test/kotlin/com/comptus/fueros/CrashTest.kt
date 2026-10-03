package com.comptus.fueros

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * The crash handler: one `crash` event with the class, message and top
 * frames, the file flushed, and the exception handed on to the handler
 * that was there. Called directly, so the test JVM lives.
 */
class CrashTest {

    private val lines = mutableListOf<String>()
    private var flushed = 0

    @Before
    fun collect() = Diag.install({ lines.add(it) }, flush = { flushed += 1 })

    @After
    fun release() = Diag.uninstall()

    @Test
    fun an_uncaught_exception_is_one_event_then_the_previous_handlers() {
        var seen: Pair<Thread, Throwable>? = null
        val previous = Thread.UncaughtExceptionHandler { t, e -> seen = t to e }
        val handler = Crash.handler(previous)
        val boom = IllegalStateException("cannot go OPTICAL -> PROXIMITY from BRIEF", RuntimeException("why"))
        handler.uncaughtException(Thread.currentThread(), boom)
        assertEquals(1, lines.size)
        val line = lines[0]
        assertTrue(line, line.contains("\"level\":\"warn\",\"layer\":\"shell\",\"event\":\"crash\""))
        assertTrue(line, line.contains("\"thread\":\"${Thread.currentThread().name}\""))
        assertTrue(line, line.contains("\"class\":\"java.lang.IllegalStateException\""))
        assertTrue(line, line.contains("\"message\":\"cannot go OPTICAL -> PROXIMITY from BRIEF\""))
        assertTrue(line, line.contains("\"frames\":[\"com.comptus.fueros.CrashTest."))
        assertTrue(line, line.contains("\"cause\":\"java.lang.RuntimeException\""))
        assertEquals(1, flushed)
        assertSame(boom, seen!!.second)
    }

    @Test
    fun the_frames_are_bounded_and_named() {
        val e = RuntimeException("deep")
        val frames = Crash.frames(e)
        assertTrue(frames.size <= Crash.FRAMES)
        assertTrue(
            frames[0],
            frames[0].startsWith("com.comptus.fueros.CrashTest.the_frames_are_bounded_and_named(CrashTest.kt:"),
        )
    }

    @Test
    fun a_message_quoting_an_identity_is_cut_to_eight() {
        val handler = Crash.handler(null)
        handler.uncaughtException(
            Thread.currentThread(),
            RuntimeException("peer 0011223344556677889900aabbccddeeff gone"),
        )
        assertTrue(lines[0], lines[0].contains("\"message\":\"peer 00112233 gone\""))
    }
}
