package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
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

    // functional: OPS-029
    @Test
    fun the_operator_is_told_of_a_newer_runner_only_where_the_machine_is_behind() {
        assertEquals(null, OperatorStore.runnerPrompt(0, 2))
        assertEquals(null, OperatorStore.runnerPrompt(2, 2))
        assertEquals(null, OperatorStore.runnerPrompt(3, 2))
        val (old, canOld) = OperatorStore.runnerPrompt(1, 2)!!
        assertEquals(false, canOld)
        assertTrue(old, old.contains("relaunch"))
        val (behind, can) = OperatorStore.runnerPrompt(2, 3)!!
        assertEquals(true, can)
        assertTrue(behind, behind.contains("version 2") && behind.contains("carries 3"))
        assertEquals(3, OperatorStore.runnerSaid("runner 3 ${"a".repeat(64)}\n"))
        assertEquals(null, OperatorStore.runnerSaid("rhtn-package-runner: no such verb: runner-refresh"))
    }
}
