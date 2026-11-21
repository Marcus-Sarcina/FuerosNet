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

    /**
     * **An instance serving no subnet is recorded so, and nobody is
     * invited to it** [author, 2026-11-21]: the record keeps the state and
     * what its records will name once it is assigned, and reads back as
     * written.
     */
    // functional: OPS-032
    @Test
    fun an_unassigned_instance_is_recorded_so_and_invited_to_by_nobody() {
        val none = OperatorStore.Instance(
            "203.0.113.7:7449", null, "an instance serving no subnet", "digitalocean", "42", 3,
            unassigned = true, endpoint = "203.0.113.7:7431", asn = 14061, zone = "nyc3",
        )
        assertEquals(none, OperatorStore.parse(OperatorStore.line(none)))
        assertEquals(3, OperatorStore.runnerIn(OperatorStore.line(none)))
        // the subnet under this key and a named one are each other states
        val own = OperatorStore.Instance("10.0.0.1:7449", null, "enrolled")
        assertEquals(own, OperatorStore.parse(OperatorStore.line(own)))
        assertFalse(OperatorStore.parse(OperatorStore.line(own)).unassigned)
        val named = none.copy(subnet = a, unassigned = false, label = "subnet aaaaaaaa…")
        assertEquals(named, OperatorStore.parse(OperatorStore.line(named)))
        assertFalse(OperatorStore.parse(OperatorStore.line(named)).unassigned)
        // a record of six fields reads as serving the subnet it names
        val six = OperatorStore.parse("1.2.3.4:7432\t$a\tMirror\tdigitalocean\t42\t3")
        assertFalse(six.unassigned)
        assertEquals(a, six.subnet)
        // **and nothing re-signs its line or invites anybody to it**
        val screen = File("src/main/kotlin/com/comptus/fueros/OperatorActivity.kt").readText()
        assertTrue(screen.contains("for (inst in instances()) {\n            // **nobody is invited"))
        assertTrue(screen.contains("if (inst.unassigned) continue"))
        assertTrue(screen.contains("if (i.unassigned) return"))
        assertTrue(screen.contains("Gateway.renew(by, i.at, token, 45, i.subnet, i.unassigned)"))
        assertEquals(
            "an enrolment's record is unassigned exactly where enrolment said so",
            2,
            Regex("""val none = said\.contains\(Gateway\.SERVES_NONE\)""").findAll(screen).count(),
        )
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

    @Test
    fun an_invitation_is_handed_to_the_composer_and_the_operator_sends_it() {
        // **the same on both platforms** [author, 2026-11-17]
        val screen = File("src/main/kotlin/com/comptus/fueros/OperatorActivity.kt").readText()
        assertTrue(screen.contains("Intent.ACTION_SENDTO"))
        assertTrue(screen.contains("\"smsto:\""))
        for (sent in listOf("SmsManager", "sendTextMessage", "permission.SEND_SMS")) {
            assertFalse("a message sent without the operator: $sent", screen.contains(sent))
        }
        // one composer at a time, the next on coming back from it
        assertTrue(screen.contains("if (composing) return"))
        assertTrue(screen.contains("composing = false\n        sendInvitations()"))
        // **a first invitation, or a root's move** [author, 2026-11-17]
        assertTrue(screen.contains("prior != at && (prior == null || rooted)"))
        assertTrue(screen.contains("?.patron == null"))
    }
}
