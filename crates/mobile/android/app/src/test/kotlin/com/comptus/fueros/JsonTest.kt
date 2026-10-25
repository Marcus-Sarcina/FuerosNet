package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class JsonTest {
    @Test
    fun the_five_kinds_read_back() {
        val v = Json.parse("""{"a": [1, 2.5, "x\"y\\nA", true, null], "b": {"c": false}} """)
        val m = Json.obj(v)
        val a = Json.list(m["a"])
        assertEquals(1.0, Json.number(a[0]))
        assertEquals(2.5, Json.number(a[1]))
        assertEquals("x\"y\\nA", Json.string(a[2]))
        assertEquals(true, Json.bool(a[3]))
        assertNull(a[4])
        assertEquals(false, Json.bool(Json.obj(m["b"])["c"]))
    }

    @Test
    fun quoting_round_trips() {
        val s = "a \"b\" \\ c\nd\te" + 1.toChar()
        assertEquals(s, Json.string(Json.parse(Json.quote(s))))
    }

    @Test
    fun trailing_garbage_is_refused() {
        assertTrue(runCatching { Json.parse("{} x") }.isFailure)
    }
}
