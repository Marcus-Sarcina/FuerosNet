package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * What the attach screen reads and says: a node's key, the address it
 * answers at, and the refusal that names which of the two is wrong.
 */
class AttachTest {

    private val key = "5046da37".repeat(8)

    @Test
    fun a_key_is_thirty_two_bytes_of_hex_however_it_was_pasted() {
        assertEquals(key, Attach.node(key))
        assertEquals(key, Attach.node("  $key  "))
        assertEquals(key, Attach.node("0x$key"))
        assertEquals(key, Attach.node(key.uppercase()))
        // a keyhash pasted from a page that groups it
        assertEquals(key, Attach.node(key.chunked(8).joinToString(":")))
    }

    @Test
    fun anything_that_is_not_a_key_is_refused_rather_than_truncated() {
        assertNull(Attach.node(""))
        assertNull(Attach.node(key.dropLast(1)))
        assertNull(Attach.node(key + "a"))
        assertNull(Attach.node("z".repeat(64)))
        assertNull(Attach.node("192.168.0.8:7447"))
    }

    @Test
    fun an_address_is_an_ip_and_a_port() {
        assertEquals("192.168.0.8:7447", Attach.address("192.168.0.8:7447"))
        assertEquals("192.168.0.8:7447", Attach.address(" 192.168.0.8:7447 "))
        assertEquals("[::1]:7447", Attach.address("[::1]:7447"))
        assertEquals("[fe80::1]:1", Attach.address("[fe80::1]:1"))
    }

    @Test
    fun a_name_a_missing_port_and_a_port_out_of_range_are_not_addresses() {
        // **the kernel resolves nothing**, so a name is said no to here
        assertNull(Attach.address("node.example:7447"))
        assertNull(Attach.address("192.168.0.8"))
        assertNull(Attach.address("192.168.0.8:"))
        assertNull(Attach.address("192.168.0.8:0"))
        assertNull(Attach.address("192.168.0.8:65536"))
        assertNull(Attach.address("192.168.0.300:7447"))
        assertNull(Attach.address("192.168.0:7447"))
        assertNull(Attach.address("::1:7447"))
        assertNull(Attach.address(""))
    }

    @Test
    fun the_refusal_names_one_field_at_a_time() {
        assertTrue(Attach.refusal("", "")!!.contains("both"))
        assertTrue(Attach.refusal("abc", "")!!.contains("hex characters"))
        assertTrue(Attach.refusal(key, "")!!.contains("address too"))
        assertTrue(Attach.refusal(key, "node.example:7447")!!.contains("A name is not one"))
        assertNull(Attach.refusal(key, "192.168.0.8:7447"))
        // the forms a person is likely to paste are worth trying, not refused
        assertNull(Attach.refusal("0x${key.uppercase()}", " [::1]:7447 "))
    }

    @Test
    fun the_roll_says_what_to_do_when_no_node_is_known() {
        val empty = Attach.roll(emptyList())
        assertTrue(empty.contains("Type where a node answers"))
        val one = Attach.roll(listOf(uniffi.rhtn_model.Instance(key.unhex(), listOf("192.168.0.8:7447"))))
        assertTrue(one.contains("192.168.0.8:7447"))
        assertTrue(one.startsWith(key.take(16)))
        // a node held with no address says so rather than looking reachable
        assertTrue(Attach.roll(listOf(uniffi.rhtn_model.Instance(key.unhex(), emptyList()))).contains("no address held"))
    }

    @Test
    fun the_status_bar_keeps_the_kernels_words_and_says_the_line_is_a_door() {
        val bar = Attach.statusLine("detached · presents 5046da37…\npayload: the Double Ratchet")
        assertTrue(bar.startsWith("detached · presents 5046da37…"))
        assertTrue(bar.contains("tap to attach"))
        // the second line of the kernel's status is not the bar's
        assertTrue(!bar.contains("payload:"))
        assertEquals("tap to attach", Attach.hint("lost: no serving node reachable"))
        assertEquals("tap to change", Attach.hint("attached, primary · presents x…"))
        assertNotNull(Attach.hint("starting the kernel…"))
    }
}
