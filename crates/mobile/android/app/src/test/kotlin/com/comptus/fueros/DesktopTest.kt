package com.comptus.fueros

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.IOException
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

class DesktopTest {
    @Test
    fun aCarriageIsItsLengthBigEndianThenItsBytes() {
        val buf = ByteArrayOutputStream()
        Desktop.write(DataOutputStream(buf), byteArrayOf(7, 8, 9))
        assertArrayEquals(byteArrayOf(0, 0, 0, 3, 7, 8, 9), buf.toByteArray())
        val back = Desktop.read(DataInputStream(ByteArrayInputStream(buf.toByteArray())))
        assertArrayEquals(byteArrayOf(7, 8, 9), back)
    }

    @Test
    fun aCarriageOverTheCeilingIsRefusedBeforeItIsRead() {
        val over = byteArrayOf(0x7f, -1, -1, -1)
        assertThrows(IOException::class.java) { Desktop.read(DataInputStream(ByteArrayInputStream(over))) }
    }

    @Test
    fun aReachIsHostAndPortWithAnIpv6HostInBrackets() {
        assertEquals("192.168.1.20" to 41234, Desktop.hostPort("192.168.1.20:41234"))
        assertEquals("fe80::1" to 9, Desktop.hostPort("[fe80::1]:9"))
        assertNull(Desktop.hostPort("192.168.1.20"))
        assertNull(Desktop.hostPort(":80"))
        assertNull(Desktop.hostPort("host:0"))
    }

    @Test
    fun theRunIsNinetyDays() {
        assertEquals(90, Desktop.RUN * 2)
    }
}
