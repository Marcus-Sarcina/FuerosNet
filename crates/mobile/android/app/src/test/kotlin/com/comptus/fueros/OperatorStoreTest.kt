package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
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
    fun a_machine_behind_this_app_is_brought_up_unasked_and_never_one_that_cannot_take_it() {
        assertTrue("not known: tried", Runner.behind(0, 3))
        assertTrue("behind", Runner.behind(2, 3))
        assertFalse("current", Runner.behind(3, 3))
        assertFalse("newer than this app", Runner.behind(4, 3))
        assertFalse("predates refreshing", Runner.behind(1, 3))
        val key = "a".repeat(64)
        assertEquals(3, OperatorStore.runnerIn("1.2.3.4:7432\t$key\tMirror\tdigitalocean\t42\t3"))
        assertEquals("launched before any was recorded", 1, OperatorStore.runnerIn("1.2.3.4:7432\t\tMirror\tdigitalocean\t42\t"))
        assertEquals("not launched here", 0, OperatorStore.runnerIn("1.2.3.4:7432\t\tMirror\t\t\t"))
        // **nothing on the Operator screen names it** [author, 2026-11-17]
        val screen = File("src/main/kotlin/com/comptus/fueros/OperatorActivity.kt").readText()
        for (said in listOf("app's runner", "runner is version", "runner-refresh")) {
            assertFalse(said, screen.contains(said))
        }
        val store = File("src/main/kotlin/com/comptus/fueros/OperatorStore.kt").readText()
        assertFalse(store.contains("newer runner is available"))
        assertTrue(screen.contains("Runner.bring(this, by, i.at, oldToken)"))
        val act = File("src/main/kotlin/com/comptus/fueros/ActActivity.kt").readText()
        assertTrue(act.contains("Runner.bring(this, by, at, token)"))
        assertEquals(3, OperatorStore.runnerSaid("runner 3 ${"a".repeat(64)}\n"))
        assertEquals(null, OperatorStore.runnerSaid("rhtn-package-runner: no such verb: runner-refresh"))
    }

    @Test
    fun a_machine_is_destroyed_only_after_the_operator_confirms() {
        // **asked once before it goes** [author, 2026-11-17]
        val screen = File("src/main/kotlin/com/comptus/fueros/OperatorActivity.kt").readText()
        assertTrue(screen.contains("setOnClickListener { confirmDestroy(i) }"))
        assertFalse("a control that destroys unasked", screen.contains("setOnClickListener { destroy(i) }"))
        assertTrue(screen.contains(".setPositiveButton(\"Destroy\") { _, _ -> destroy(i) }"))
        assertEquals("destroy(i) is reached from the confirmation alone", 1, Regex("""\bdestroy\(i\)""").findAll(screen).count())
    }
}
