package com.comptus.fueros

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * The shell's event line: the kernel's shape (`ms`, `level`, `layer`,
 * `event`, then the fields in order), the escaping, the truncations the
 * plan's section 2 asks for, and the Meet flow's transitions reaching it.
 */
class DiagTest {

    private val lines = mutableListOf<String>()

    @Before
    fun collect() = Diag.install({ lines.add(it) })

    @After
    fun release() = Diag.uninstall()

    @Test
    fun an_event_is_one_json_object_in_the_kernels_field_order() {
        val line = Diag.render(
            42,
            "info",
            "shell",
            "meet.step",
            listOf("from" to "BRIEF", "to" to "OPTICAL", "n" to 3, "ok" to true),
        )
        assertEquals(
            """{"ms":42,"level":"info","layer":"shell","event":"meet.step","from":"BRIEF","to":"OPTICAL","n":3,"ok":true}""",
            line,
        )
    }

    @Test
    fun a_null_field_is_left_out_and_strings_are_escaped() {
        val line = Diag.render(0, "warn", "shell", "x", listOf("gone" to null, "s" to "a\"b\\c\nd\u0001"))
        assertEquals("""{"ms":0,"level":"warn","layer":"shell","event":"x","s":"a\"b\\c\nd\u0001"}""", line)
    }

    @Test
    fun enums_lists_and_maps_render_as_json() {
        assertEquals("\"OPTICAL\"", Diag.json(Meet.Step.OPTICAL))
        assertEquals("[1,\"two\",false]", Diag.json(listOf(1, "two", false)))
        assertEquals(
            "{\"a\":1,\"b\":{\"c\":true}}",
            Diag.json(linkedMapOf("a" to 1, "b" to mapOf("c" to true))),
        )
        assertEquals("{\"run\":\"r\",\"n\":2}", Diag.obj(listOf("run" to "r", "skip" to null, "n" to 2)))
    }

    @Test
    fun the_emitted_line_carries_level_layer_and_a_rising_ms() {
        Diag.event("a", "k" to 1)
        Diag.warn("b")
        Diag.debug("c")
        assertEquals(3, lines.size)
        assertTrue(lines[0].startsWith("{\"ms\":"))
        assertTrue(lines[0].contains("\"level\":\"info\",\"layer\":\"shell\",\"event\":\"a\",\"k\":1}"))
        assertTrue(lines[1].contains("\"level\":\"warn\""))
        assertTrue(lines[2].contains("\"level\":\"debug\""))
        val ms = lines.map { Regex("\"ms\":(\\d+)").find(it)!!.groupValues[1].toLong() }
        assertTrue(ms.zipWithNext().all { (a, b) -> a <= b })
    }

    @Test
    fun the_kernels_line_passes_through_untouched() {
        val k = """{"ms":7,"level":"info","layer":"ffi","event":"ffi.call","method":"begin"}"""
        Diag.kernel(k)
        assertEquals(listOf(k), lines)
    }

    @Test
    fun nothing_is_rendered_with_no_sink() {
        Diag.uninstall()
        assertFalse(Diag.enabled())
        Diag.event("a")
        Diag.kernel("x")
        assertTrue(lines.isEmpty())
    }

    @Test
    fun identities_are_eight_hex_characters_and_long_hex_runs_are_cut() {
        val id = ByteArray(32) { (it + 0x10).toByte() }
        assertEquals("10111213", Diag.id8(id))
        assertEquals(null, Diag.id8(null as ByteArray?))
        assertEquals("deadbeef", Diag.id8("deadbeefcafe0123"))
        assertEquals(
            "the two meeting ids agree: 00112233...",
            Diag.scrub("the two meeting ids agree: 00112233445566778899aabbccddeeff..."),
        )
        // fifteen hex characters is a word, not an identity
        assertEquals("abcdefabcdefabc", Diag.scrub("abcdefabcdefabc"))
    }

    @Test
    fun the_meet_flows_transitions_notes_and_stop_are_events() {
        val m = Meet("aa", "carol", Meet.Kind())
        m.crossBootstrap()
        m.accept()
        m.note("their contribution is in: 0123456789abcdef0123")
        m.opticalDone()
        m.stop("the carried intent was refused")
        val events = lines.map { Regex("\"event\":\"([^\"]+)\"").find(it)!!.groupValues[1] }
        assertEquals(listOf("meet.step", "meet.step", "meet.note", "meet.step", "meet.stop"), events)
        assertTrue(lines[0].contains("\"from\":\"INTENT\",\"to\":\"BRIEF\",\"trigger\":\"tap\""))
        assertTrue(lines[1].contains("\"from\":\"BRIEF\",\"to\":\"OPTICAL\",\"trigger\":\"tap\""))
        assertTrue(lines[2].contains("\"line\":\"their contribution is in: 01234567\""))
        assertTrue(lines[3].contains("\"trigger\":\"kernel\""))
        assertTrue(lines[4].contains("\"level\":\"warn\""))
        assertTrue(lines[4].contains("\"from\":\"PROXIMITY\",\"reason\":\"the carried intent was refused\""))
    }

    @Test
    fun the_responders_bootstrap_is_read_by_the_camera() {
        Meet("aa", "carol", Meet.Kind(), Meet.Role.RESPONDER).crossBootstrap()
        assertTrue(lines.single().contains("\"trigger\":\"camera\""))
    }

    @Test
    fun the_spec_pins_field_decodes_to_a_map() {
        val pins = Report.pins("network-design.md=61663b6c74ef,wire-format.md=683db42970ba")
        assertEquals(mapOf("network-design.md" to "61663b6c74ef", "wire-format.md" to "683db42970ba"), pins)
        assertEquals(emptyMap<String, String>(), Report.pins(""))
    }
}
