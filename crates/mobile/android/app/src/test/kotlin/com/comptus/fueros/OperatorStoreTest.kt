package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Test

class OperatorStoreTest {

    private val a = "a".repeat(64)
    private val b = "b".repeat(64)

    /** The records written before one file per identity go to the identity
     *  whose subnet they name, and one naming none goes to the first to ask. */
    @Test
    fun an_old_record_goes_to_the_identity_whose_subnet_it_names() {
        val lines = listOf(
            "64.227.31.70:7449\t$a\tMirror Manse\tdigitalocean\t1",
            "167.99.153.102:7449\t$b\tE Two\tdigitalocean\t2",
            "10.0.0.1:7449\t\tenrolled\t\t",
            "",
        )
        val (takenA, leftA) = OperatorStore.split(lines, setOf(a))
        assertEquals(listOf(lines[0], lines[2]), takenA)
        assertEquals(listOf(lines[1]), leftA)
        val (takenB, leftB) = OperatorStore.split(leftA, setOf(b))
        assertEquals(listOf(lines[1]), takenB)
        assertEquals(emptyList<String>(), leftB)
        assertEquals("an identity in neither subnet takes nothing", emptyList<String>(), OperatorStore.split(leftA, setOf("c".repeat(64))).first)
    }
}
