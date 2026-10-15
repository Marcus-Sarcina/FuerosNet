package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * The bootstrap carries the inviter's claim about where an adoption leads
 * (`light-client-requirements.md` §7), and the older form still reads.
 */
class BootstrapTest {
    private val me = ByteArray(32) { 0xab.toByte() }
    private val subnet = "cd".repeat(32)

    // acceptance: PRD-15
    @Test
    fun a_version_two_code_carries_the_subnet_and_both_aliases_as_claims() {
        val code = Bootstrap.encode(me, 1, false, Meet.Scope(subnet, "Bowling League", "Alice"))
        val read = Bootstrap.decode(code)!!
        assertEquals("ab".repeat(32), read.me)
        assertEquals(1, read.kind)
        assertEquals(false, read.askBackup)
        assertEquals(subnet, read.subnet)
        assertEquals("Alice", read.alias)
        assertEquals("Bowling League", read.subnetAlias)
        // and the brief's label is the patron's alias and the subnet's
        val claim = Meet.Claim(read.subnet, read.alias, read.subnetAlias)
        assertEquals("Alice's Bowling League", claim.label("the other person"))
    }

    @Test
    fun a_lone_root_names_no_subnet_and_the_label_says_so() {
        val code = Bootstrap.encode(me, 1, true, Meet.Scope.OWN)
        val read = Bootstrap.decode(code)!!
        assertNull(read.subnet)
        assertNull(read.alias)
        assertEquals(true, read.askBackup)
        assertEquals("carol's a new subnet", Meet.Claim(read.subnet, read.alias, read.subnetAlias).label("carol"))
        // no scope at all, a regular meeting: the same empty claim
        assertNull(Bootstrap.decode(Bootstrap.encode(me, 0, false, null))!!.subnet)
    }

    @Test
    fun the_older_form_still_reads_and_claims_nothing() {
        val v1 = byteArrayOf(1, 2, 0) + me
        val read = Bootstrap.decode(v1)!!
        assertEquals(2, read.kind)
        assertNull(read.subnet)
        assertNull(read.alias)
        assertNull(Bootstrap.decode(byteArrayOf(1, 2, 0) + me + byteArrayOf(0)))
        assertNull(Bootstrap.decode(byteArrayOf(3, 0, 0) + me))
        assertNull(Bootstrap.decode(byteArrayOf(2, 0, 0) + me))
        // a truncated alias is not a code
        val short = Bootstrap.encode(me, 0, false, Meet.Scope(subnet, "x", "y"))
        assertNull(Bootstrap.decode(short.copyOf(short.size - 1)))
    }
}
