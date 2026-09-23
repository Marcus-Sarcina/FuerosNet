// One round trip through the generated Kotlin binding, against a real
// serving node: the milestone's proof that a shell in its own language can
// start a participant, attach, and send and receive.  Run by
// `tools/kotlin-roundtrip.sh`; the node and the identities come from the
// facade's `harness` feature, which a shell never builds.
import uniffi.rhtn_ffi.*

class Shell : Proximity, Camera, Clock, Random, Operator, Notices, Storage {
    private val kept = HashMap<String, ByteArray>()
    private val rng = java.security.SecureRandom()
    override fun `supported`(): List<Channel> = listOf()
    override fun `run`(`channel`: Channel, `peer`: ByteArray): ChannelOutcome = ChannelOutcome.UNAVAILABLE
    override fun `resolutionM`(`channel`: Channel): ULong? = null
    override fun `capture`(`ask`: Ask): ByteArray = ByteArray(64) { 7 }
    override fun `nowMs`(): ULong = System.currentTimeMillis().toULong()
    override fun `waitMs`(`ms`: ULong) {}
    override fun `fill`(`n`: UInt): ByteArray { val b = ByteArray(n.toInt()); rng.nextBytes(b); return b }
    override fun `ask`(`question`: String): Boolean = true
    override fun `told`(`notice`: Told) {}
    override fun `read`(`name`: String): ByteArray? = kept[`name`]
    override fun `write`(`name`: String, `bytes`: ByteArray): Boolean { kept[`name`] = `bytes`; return true }
}

fun platformOf(): Platform { val s = Shell(); return Platform(s, s, s, s, s, s, s) }

fun main() {
    val node = TestNode.start("bob", listOf("alice", "carol"))
    val addr = node.address()
    val (alice, bob, carol) = Triple(testIdentity("alice"), testIdentity("bob"), testIdentity("carol"))
    val known = listOf(alice.material, bob.material, carol.material)
    val a = Participant.start(alice.seeds, known, platformOf())
    val c = Participant.start(carol.seeds, known, platformOf())
    check(a.status() == Status.Detached) { "detached before attaching" }
    c.attach(bob.id, listOf(addr), listOf())
    val attached = a.attach(bob.id, listOf(addr), listOf(carol.id))
    check(attached.primary) { "alice is in bob's subtree" }
    c.attach(bob.id, listOf(addr), listOf(alice.id))
    a.send(carol.id, kindApplication(), "over the binding".toByteArray())
    val got = c.nextEvent(5000UL)
    check(got is Event.Payload) { "carol received a payload, got $got" }
    check(got.from.contentEquals(alice.id)) { "from alice" }
    check(String(got.bytes) == "over the binding") { "the bytes" }
    var refused = false
    try { a.send(ByteArray(3), kindApplication(), ByteArray(1)) } catch (e: Refused.Reason) { refused = e.reason.contains("32 bytes") }
    check(refused) { "a bad recipient is refused with its reason" }
    a.detach(); c.detach()
    println("KOTLIN ROUND TRIP OK")
}
