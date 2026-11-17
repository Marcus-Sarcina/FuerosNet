package com.comptus.fueros

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * **A `NetworkPoint`'s address, read and replaced** (`wire-format.md`
 * §7.6) for re-signing a moved node's records ([Provision.pointIp],
 * [Provision.pointAt]): the address changes, and what the point was signed
 * with besides — the ASN, the zone, the port — does not.
 */
class PointTest {
    // {1: h'9df5f2b6', 2: 14061, 4: "nyc1"}: 157.245.242.182, AS14061, nyc1
    private val held = "a301449df5f2b6021936ed04646e796331"

    // functional: OPS-030
    @Test
    fun the_address_is_read_and_replaced_and_nothing_else_moves() {
        assertEquals("157.245.242.182", Provision.pointIp(held))
        val moved = Provision.pointAt(held, "165.22.9.217")!!
        assertArrayEquals(Provision.unhex("a30144a51609d9021936ed04646e796331"), moved)
        assertEquals("165.22.9.217", Provision.pointIp(Provision.hex(moved)))
        assertNull(Provision.pointAt(held, "300.1.1.1"))
        assertNull(Provision.pointIp("a10264"))
    }
}
