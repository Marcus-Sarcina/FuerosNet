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

    /**
     * **An unassigned instance is offered every subnet this key holds, and
     * its record, once assigned, serves the one chosen** [author,
     * 2026-11-21: "Both A and B until all nodes are assigned"]: the
     * Operator screen offers only instances flagged as serving none, one
     * button per held subnet, and the rewritten record round-trips as
     * assigned to it, keeping what its records name.
     */
    // functional: OPS-032
    @Test
    fun an_unassigned_instance_assigned_to_a_held_subnet_is_recorded_as_serving_it() {
        val none = OperatorStore.Instance(
            "203.0.113.7:7449", null, "an instance serving no subnet", "digitalocean", "42", 3,
            unassigned = true, endpoint = "203.0.113.7:7431", asn = 14061, zone = "nyc3",
        )
        val served = OperatorStore.Instance("10.0.0.1:7449", a, "Mirror")
        // only the instance serving no subnet is offered, and none without
        // an address to reach it at
        assertEquals(listOf(none), OperatorStore.unassigned(listOf(none, served, none.copy(at = ""))))
        assertEquals(emptyList<OperatorStore.Instance>(), OperatorStore.unassigned(listOf(served)))
        // every subnet held, once apiece; never a new subnet or no scope
        val league = Meet.Scope(a, "Bowling League", null)
        val bare = Meet.Scope(b, null, null)
        assertEquals(
            listOf(league, bare),
            OperatorStore.assignable(listOf(league, bare, league, Meet.Scope.OWN, Meet.Scope.INDIVIDUAL)),
        )
        assertEquals("holding none, nothing is offered", emptyList<Meet.Scope>(), OperatorStore.assignable(emptyList()))
        // the record rewritten: the subnet named, labelled as the chooser
        // labels it, no longer unassigned, and the rest as it was
        val to = OperatorStore.assigned(none, league)
        assertEquals(a, to.subnet)
        assertEquals("Bowling League", to.label)
        assertFalse(to.unassigned)
        assertEquals(none.copy(subnet = a, label = "Bowling League", unassigned = false), to)
        val back = OperatorStore.parse(OperatorStore.line(to))
        assertEquals(to, back)
        assertFalse(back.unassigned)
        assertEquals(listOf<OperatorStore.Instance>(), OperatorStore.unassigned(listOf(back)))
        assertEquals("subnet bbbbbbbb…", OperatorStore.assigned(none, bare).label)
        // **and both paths assign through the one function**: the Operator
        // screen and the adoption's answer
        val screen = File("src/main/kotlin/com/comptus/fueros/OperatorActivity.kt").readText()
        val meet = File("src/main/kotlin/com/comptus/fueros/MeetActivity.kt").readText()
        assertTrue(screen.contains("OperatorStore.assign(this@OperatorActivity, by, now, token, s)"))
        assertTrue(meet.contains("OperatorStore.assign(this, by, i, token, into)"))
        assertTrue(screen.contains("OperatorStore.assignable(Gateway.subnets(by).map(Meet.Scope::of))"))
        assertFalse("the adoption's question is gated on the record's flag alone", meet.contains("Gateway.unassigned(by)"))
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

    // functional: NET-036
    /**
     * **An adoption carries the patron's own node to the member** [author,
     * 2026-11-21]: the instance chosen at SERVE_FROM, else the newest
     * record serving the subnet the adoption is into, as members dial it;
     * nothing as the party adopted, at a regular meeting, from an
     * instance serving no subnet or another, or before a scope is chosen.
     */
    @Test
    fun an_adoption_carries_the_patrons_own_node_and_nothing_else_does() {
        val inA = OperatorStore.Instance("64.227.31.70:7449", a, "Mirror")
        val newerInA = OperatorStore.Instance("167.99.153.102:7449", a.uppercase(), "Mirror two")
        val inB = OperatorStore.Instance("10.0.0.9:7449", b, "Other")
        val own = OperatorStore.Instance("10.0.0.5:7449", null, "Under my key")
        val loose = OperatorStore.Instance("203.0.113.7:7449", null, "", unassigned = true)
        val held = Meet.Scope(a, "Garden", null)
        val them = Meet.Adopt.THEM_UNDER_ME
        val all = listOf(inA, newerInA, inB, own, loose)
        fun node(adopt: Meet.Adopt, scope: Meet.Scope?, serving: Meet.Serving?, xs: List<OperatorStore.Instance> = all) =
            OperatorStore.meetingNode(adopt, scope, serving, xs)

        assertEquals("the newest record serving it, where members dial it", "167.99.153.102:7431", node(them, held, null))
        assertEquals("one record", "64.227.31.70:7431", node(them, held, null, listOf(inA, inB)))
        assertEquals("the one chosen at SERVE_FROM before any", "203.0.113.7:7431", node(them, held, Meet.Serving(loose.at)))
        assertEquals("none chosen: what already serves it", "167.99.153.102:7431", node(them, held, Meet.Serving.NONE))
        assertEquals("a new subnet, served by the record under this key", "10.0.0.5:7431", node(them, Meet.Scope.OWN, null))
        val rootB = Meet.Scope(b, "Mine", null)
        assertEquals(
            "the subnet this key roots, served by the record naming none",
            "10.0.0.5:7431",
            OperatorStore.meetingNode(them, rootB, null, listOf(own), b),
        )
        assertEquals(
            "a held subnet this key does not root is not the record naming none's",
            null,
            OperatorStore.meetingNode(them, held, null, listOf(own), b),
        )
        assertEquals("a new subnet with no node and none chosen", null, node(them, Meet.Scope.OWN, Meet.Serving.NONE, listOf(inA, loose)))
        assertEquals("a subnet no own instance serves", null, node(them, Meet.Scope(b.replace('b', 'c'), null, null), null))
        assertEquals("an unassigned instance serves nobody", null, node(them, Meet.Scope.OWN, null, listOf(loose)))
        for (side in listOf(Meet.Adopt.ME_UNDER_THEM, Meet.Adopt.NONE)) {
            assertEquals("$side carries none", null, node(side, held, Meet.Serving(loose.at)))
        }
        assertEquals("before the scope is settled", null, node(them, null, null))
        assertEquals("joining as an individual names no subnet", null, node(them, Meet.Scope.INDIVIDUAL, null))

        // **declared with every choice answered, before the intent is
        // built, and for every meeting**, so none leaks into the next
        val ceremony = File("src/main/kotlin/com/comptus/fueros/Ceremony.kt").readText()
        val declared = ceremony.indexOf("Gateway.declareNode(m.by, meetingNode(m))")
        assertTrue(declared > 0)
        assertTrue("before the carriage", declared < ceremony.indexOf("p.intentCarriage()"))
        // and the member's side proves what the kernel held, as a link's
        assertTrue(ceremony.contains(".also { if (it is Adoption.Result.Ok) provedInvitation(p) }"))
    }
}
