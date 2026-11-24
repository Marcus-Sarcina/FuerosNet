package com.comptus.fueros

import android.content.Context
import org.json.JSONObject
import uniffi.rhtn_ffi.Answer
import uniffi.rhtn_ffi.Conversation
import uniffi.rhtn_ffi.Event
import uniffi.rhtn_ffi.Participant
import uniffi.rhtn_ffi.Refused
import uniffi.rhtn_ffi.Told
import uniffi.rhtn_ffi.TraceStep
import uniffi.rhtn_ffi.kindApplication
import uniffi.rhtn_model.Connection
import uniffi.rhtn_model.Contact
import uniffi.rhtn_model.Cost
import uniffi.rhtn_model.How
import uniffi.rhtn_model.Instance
import uniffi.rhtn_model.Line
import uniffi.rhtn_model.Meeting
import uniffi.rhtn_model.Resource
import uniffi.rhtn_model.Subnet

object Attachments {
    /** An identity's conversations, in its own scope ([Threads]). */
    internal const val THREADS = "threads"

    /** Who this device nominates to witness, where the provision names
     *  anybody: a bench stand-in for the horizon a phone at genesis does
     *  not have (see [beginCeremony]). */
    @Volatile internal var nominees: List<ByteArray> = listOf()

    internal var peerName: String = "peer"

    @Volatile internal var peerKey: String? = null

    @Volatile internal var peer: ByteArray? = null

    internal var provisioned = false

    internal var started = false

    internal val reattaching = java.util.concurrent.ConcurrentHashMap<String, Int>()

    /** The waits before each attempt to attach again, in seconds; the last repeats. */
    internal val REATTACH_AFTER_S = longArrayOf(15, 30, 60, 120, 300)

    fun forgetCaptures(by: String, hexKey: String) {
        val p = Gateway.loaded[by] ?: run { Gateway.front.note("· no kernel yet"); return }
        val who = runCatching { Gateway.unhex(hexKey) }.getOrNull() ?: run { Gateway.front.note("· not a key: $hexKey"); return }
        Thread({
            Gateway.call("forget_captures", { e -> Gateway.front.note("· forget refused: ${e.reason}") }) {
                val n = p.forgetCaptures(who)
                Gateway.front.note("· forgot $n sealed capture(s) of ${hexKey.take(8)}…")
            }
        }, "fueros-forget").start()
    }

    fun contactPeersNow(by: String) {
        val p = Gateway.loaded[by] ?: run { Gateway.front.note("· no kernel yet"); return }
        Thread({
            Gateway.call("contact_peers", { e -> Gateway.front.note("· contact refused: ${e.reason}") }) {
                val round = p.contactPeers()
                Diag.event("peer.round", "peers" to round.size, "held" to round.count { it.held }, "how" to round.joinToString(",") { it.how })
                Gateway.front.note("· peers: " + round.joinToString("; ") { "${Gateway.hex(it.who).take(8)}… ${it.how}" }.ifEmpty { "none" })
            }
        }, "fueros-contact").start()
    }

    /** The one JSON file of 2026-11-12, written out as sealed records once and forgotten. */
    internal fun migrateThreads(shell: AndroidShell, s: Threads) {
        val old = shell.read(THREADS) ?: return
        val threads = Threads.decodeLegacy(old)
        for (t in threads) {
            for (m in t.messages) s.append(t.peerKey, Threads.Record.Said(m.mine, m.who, m.text, m.delivery, m.reason))
        }
        if (threads.isNotEmpty()) s.writeIndex(threads.map { Threads.Entry(it.peerKey, it.peerName, it.direct) })
        shell.forget(THREADS)
        Diag.event("threads.migrated", "threads" to threads.size, "messages" to threads.sumOf { it.messages.size })
    }

    /** The kernel's sealing service for one identity, as [Threads] reaches it. */
    internal fun sealerOf(p: Participant) = object : Threads.Sealer {
        override fun seal(label: String, bytes: ByteArray): ByteArray = p.seal(label, bytes)
        override fun unseal(label: String, bytes: ByteArray): ByteArray? = runCatching { p.unseal(label, bytes) }.getOrNull()
    }

    /**
     * Send application bytes **to the party whose thread this is**, named
     * by its keyhash in hex [2026-10-27].
     *
     * The bubble goes up at once as *sending*; the kernel's own send blocks
     * on the network, so it runs off the caller's thread and settles the
     * bubble to *sent* — accepted for Ceremony.carriage, never *delivered* — or to
     * *unsent* with the refusal's reason.
     *
     * **The recipient was this kernel's one `peer` slot until now**, which
     * every meeting overwrote: a conversation screen drew the thread it was
     * opened on and then sent to whoever had been met last, putting the
     * bubble in that other thread as it went. A screen showing one party
     * and sending to another is a misdelivery, not a missing feature, so
     * the recipient is the screen's own and this kernel keeps no current
     * one. Who may be sent to is every party [adopt] has pinned, which is
     * every meeting this device has reached the optical step of.
     */
    fun send(by: String, to: String, text: String) {
        // **as the identity that holds the thread, never the one in
        // focus** — step 2b of the browser model [author, 2026-11-11]: a
        // reply typed into another identity's conversation went out under
        // the focused key before this, which is the wrong-key act the
        // row-holder rule exists to prevent, and nothing would have
        // refused it. Refused here rather than fallen back from
        // ([asHolder]).
        val p = Gateway.asHolder(by)
        val who = runCatching { Gateway.unhex(to) }.getOrNull()
        if (p == null || who == null || who.size != 32) {
            Gateway.front.note(
                if (p == null) "· that identity is not open on this device" else "· not a party to send to: ${to.take(8)}…",
            )
            return
        }
        val id = Gateway.front.outgoing(by, to, text)
        Thread({
            Gateway.call("send", { e -> Gateway.front.settle(by, to, id, Front.Delivery.UNSENT, e.reason) }) {
                p.send(who, kindApplication(), text.toByteArray())
                Gateway.front.settle(by, to, id, Front.Delivery.SENT, null)
            }
        }, "fueros-send").start()
    }

    fun provision(context: Context, blob: String) {
        AndroidShell(context.applicationContext).write("provision", blob.toByteArray())
        synchronized(Gateway.lock) {
            if (!started || provisioned) return
        }
        Gateway.front.note("· provision stored. Restart the app for it to take effect.")
    }

    /**
     * **Detach, which the person may do and the screen offers**: the
     * session closes cleanly so the serving node queues for this device
     * from now rather than after three missed intervals. Everything else
     * is kept.
     */
    fun detachNow(by: String) {
        val p = Gateway.loaded[by] ?: run { Gateway.front.note("· no kernel yet"); return }
        Thread({
            Gateway.call("detach", { e -> Gateway.front.note("· detach refused: ${e.reason}") }) {
                p.detach()
                Gateway.front.note("· detached. Nothing is queued for this device from here.")
                show(p, Connection.Detached)
            }
        }, "fueros-detach").start()
    }

    /**
     * **Attach to a serving node the person named** [2026-10-27].
     *
     * Until this, the only attach was the one `bringUp` performs from the
     * bench's provision blob, read once at a cold start — so an installed
     * app had no way to reach a node at all, and every run that needed one
     * was a run a script had set up. A second attach replaces the first,
     * which is what returning to a patron after a spell on a sibling is.
     *
     * **The population swept is the parties this device holds**, not the
     * nodes: a sweep stocks payload material for people, and a node is
     * where they are reached rather than somebody to reach.
     *
     * Off the caller's thread: an attach dials, publishes and sweeps.
     */
    fun attachTo(by: String, nodeHex: String, address: String) {
        val p = Gateway.loaded[by] ?: run { Gateway.front.note("· no kernel yet"); return }
        val node = Attach.node(nodeHex)?.let { runCatching { Gateway.unhex(it) }.getOrNull() }
        val at = Attach.address(address)
        if (node == null || at == null) {
            Gateway.front.note("· not a node and an address: ${nodeHex.take(8)}… at $address")
            return
        }
        Thread({
            val population = Gateway.call("contacts", { emptyList() }) {
                // the people, not the infrastructure: `how` 4 is a node
                p.contacts().filter { it.how != How.INFRA }.map { it.keyhash }
            }
            val a = Gateway.call("attach", { e -> Gateway.front.note("· attach refused: ${e.reason}"); null }) {
                p.attach(node, listOf(at), population)
            }
            if (a == null) {
                // **a node that did not answer is the detached case**, as
                // it is on the provision's path: the loop goes on running
                Gateway.front.note("· $at did not answer: this device stays detached.")
                show(p, Connection.Detached)
                // **and it is tried again, as the start's attach is** [bench,
                // 2026-11-17]: an accepted move whose first dial met a dead
                // network stayed detached until the app was restarted
                val scope = Gateway.appContext?.let { c -> Identities.identities(c).firstOrNull { it.id == by }?.scope }
                reattachLater(by, p, scope, 1)
            } else {
                // **not the provision's flag**: that one says a blob was
                // read, and this attach is the person's own
                Gateway.front.note("· attached to ${Gateway.hex(node).take(8)}… at $at; ${a.queued} queued there.")
                show(p, Connection.Attached(node, a.primary))
            }
        }, "fueros-attach").start()
    }

    /** The bench's lever: `<node hex>@<host:port>` agreed to by the identity in focus. */
    fun agreeToNow(by: String, spec: String) {
        val p = Gateway.loaded[by] ?: return
        val cut = spec.indexOf('@')
        if (cut <= 0) return
        val node = runCatching { Gateway.unhex(spec.substring(0, cut)) }.getOrNull() ?: return
        val addr = spec.substring(cut + 1)
        Gateway.call("agreeTo", { e -> Diag.warn("agree_to", "why" to e.reason) }) { p.agreeTo(node, listOf(addr)) }
        Diag.event("agree_to", "node" to spec.take(8), "addr" to addr)
    }

    /**
     * Keep a provision blob for the kernel to read when it comes up.
     *
     * Written before [start] on a cold launch, which is where a provision
     * arrives. **A kernel already up does not re-read it**: it is attached
     * where it was told to attach, and the screen says so rather than
     * appearing to have taken an instruction it did not.
     */
    /**
     * **Serve from this node from now on**, which is the whole of attaching
     * [author, 2026-11-04: "attachment is not a separate act. The operator
     * should attach upon launch and install, other horizon members should
     * agree to attach once when invited and then all horizon members
     * should re-attach every time they become detached."].
     *
     * Two things, because attaching once is not what was asked for. The
     * node is **remembered in the provision**, which is what [bringUp]
     * attaches from at every start — so a device that becomes detached
     * comes back to the same node without being asked again. And it is
     * **attached to now**, because a provision alone takes effect only at
     * the next start and the operator who just launched a machine is
     * entitled to be on it.
     *
     * **Nobody is asked for a key or an address.** The operator's own node
     * speaks as its operator, so the key is this device's own keyhash; the
     * address is the machine this device launched. A member learns both
     * from the invitation it agreed to. There is nothing here a person
     * could usefully type.
     */
    fun serveFrom(by: String, context: Context, nodeHex: String, address: String) {
        // **the node an identity agrees to is that identity's** — step 4
        // of the browser model [author, 2026-11-11]. Written into the
        // device's provision before this, so that with an attach per
        // identity every identity on the device would have joined a node
        // one of them agreed to at the next start: a profile joining a
        // server it never agreed to. Remembered in the focused identity's
        // own scope instead, which is the one that agreed (the kernel's agreement).
        val app = context.applicationContext
        val scope = Identities.activeId?.let { id -> Identities.identities(app).firstOrNull { it.id == id }?.scope }
        // **one tab per address** [author, 2026-11-11], **kept by the kernel**
        // [review, 2026-11-13: the one store of what an identity agreed to]
        val p = Gateway.loaded[by] ?: run { Gateway.front.note("· no kernel yet"); return }
        val node = Attach.node(nodeHex)?.let { runCatching { Gateway.unhex(it) }.getOrNull() }
        if (node == null) {
            Gateway.front.note("· not a node: ${nodeHex.take(8)}…")
            return
        }
        Gateway.call("agreeTo", { e -> Gateway.front.note("· the node could not be remembered: ${e.reason}") }) {
            p.agreeTo(node, listOf(address))
        }
        attachTo(by, nodeHex, address)
    }

    /**
     * **The network is the platform's to know** (`light-client-requirements.md`
     * §9: the hardware crosses inward). The default network coming up —
     * at start, and again after it was lost — is handed to the kernel,
     * which stands its direct socket again where the device now routes
     * out of another interface and reaches for every peer (design §6.3).
     * The kernel reaches for them on its own clock besides; this is the
     * reconnect case, which a clock alone would Ceremony.meet() late. Off the
     * callback's thread: a round dials, and a dial waits.
     */
    internal fun watchNetwork(p: Participant) {
        val cm = Gateway.appContext?.getSystemService(android.net.ConnectivityManager::class.java) ?: return
        try {
            cm.registerDefaultNetworkCallback(object : android.net.ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: android.net.Network) {
                    Diag.event("Gateway.shell.network", "state" to "available")
                    Thread({
                        Gateway.call("network_changed", { _ -> }) {
                            val round = Gateway.loaded.values.flatMap { q -> q.networkChanged() }
                            Diag.event(
                                "peer.round",
                                "peers" to round.size,
                                "held" to round.count { it.held },
                                "how" to round.joinToString(",") { it.how },
                            )
                            round.forEach { c ->
                                val who = Gateway.hex(c.who).take(8)
                                if (c.how == "dialled") Gateway.front.note("· peer $who… reached directly")
                            }
                        }
                    }, "fueros-network").start()
                }

                override fun onLost(network: android.net.Network) {
                    Diag.event("Gateway.shell.network", "state" to "lost")
                }
            })
        } catch (e: Exception) {
            Diag.warn("Gateway.shell.network", "state" to "unwatched", "error" to (e.message ?: e.javaClass.simpleName))
        }
    }

    /**
     * The connection, what this device presents, and which construction the
     * payload session runs.
     *
     * **The construction is the client's own statement** and nothing a peer
     * can check (`light-client-requirements.md` §3): the session derives the
     * same keys or it does not, so the only party who can say which half is
     * running is this one, and it says it where its user can read it. The
     * direct-path chip is updated here too — **status, never a choice**: the
     * path order is fixed (design §12.6.3).
     */
    internal fun show(p: Participant, s: Connection) {
        val key = p.presentedKey().joinToString("") { "%02x".format(it) }.take(16)
        val line = when (s) {
            is Connection.Detached -> "detached"
            is Connection.Attached -> if (s.primary) "attached, primary" else "attached"
            is Connection.Reconnecting -> "reconnecting…"
            is Connection.Lost -> "lost: no serving node reachable"
        }
        Gateway.front.setStatus("$line · presents $key…")
        // **every thread's chip, not the latest party's** [2026-10-27]:
        // the chip is per peer on the thread list (C1), and reading it off
        // one slot left every other thread's showing whatever it showed
        // when that party was the current one
        for (t in Gateway.front.threads()) {
            runCatching { Gateway.unhex(t.peerKey) }.getOrNull()?.let { who ->
                if (who.size == 32) Gateway.loaded[t.by]?.let { h -> Gateway.front.directPath(t.by, t.peerKey, h.directTo(who)) }
            }
        }
    }

    /**
     * **The event loop, over one participant** — step 1 of the browser
     * model [author, 2026-11-11: *"Begin at 1"*]: extracted from [bringUp]
     * so that a loop can be run per identity, which is what lets an
     * identity receive while it is not the one in focus. **This step
     * changes no behaviour**: [bringUp] still runs exactly one, for the
     * identity that acts, on the thread it always did.
     *
     * What the loop still shares with the device rather than the identity
     * — the meeting ([Ceremony.meet()]), the front's notices, the model's refresh —
     * is step 2's to separate, and is left as it was here so that this
     * extraction can be read as a move and nothing else.
     */
    internal fun runLoop(p: Participant, id: String) {
        // **the loop's own first word.** Said once, before any turn, so
        // that a run which never reaches the loop is distinguishable from
        // one whose loop parks in its first call (2026-10-24).
        Diag.event("kernel.loop", "state" to "entered", "who" to id.take(8))
        val diag = Ceremony.TurnDiag()
        var turns = 0L
        while (true) {
            // **before the call that can park, not after.** The turn
            // heartbeat sits past `nextEvent` and the event handling, so a
            // loop parked in `nextEvent` emits nothing at all and says
            // nothing about where it stopped — which is how A6's fifth and
            // sixth attempts left the question open. `next_event` is
            // wrapped in the diagnostics' plain call, which emits nothing,
            // so this is the only line that can place it. On the clock
            // rather than every turn: a 2 s poll would otherwise write
            // thirty lines a minute for a loop doing nothing.
            Ceremony.turnEntry(diag, turns)
            turns += 1
            val e = p.nextEvent(2000UL)
            // **the meeting belongs to the identity in focus** — step 2 of
            // the browser model [author, 2026-11-11]: a meeting, the
            // Ceremony.adoption() and Ceremony.peering() it carries, and the tick that drives
            // them are the device's one at a time and the focused
            // identity's, where everything else below — a message, a
            // peer's word, a changed noun — is this identity's whichever
            // is in focus. Read every turn, since focus can move while
            // this loop runs. **No behaviour changes while bring-up runs
            // one loop for the identity that acts**: that loop is always
            // the focused one, and the unfocused branches are reached only
            // once step 3 runs a loop per identity.
            val focused = id == Identities.activeId
            // the kind of thing that arrived and who from, as eight hex
            // characters; a payload's bytes are counted, never shown
            if (e != null && Diag.enabled()) {
                Diag.event(
                    "Gateway.shell.event",
                    "kind" to e::class.simpleName,
                    "from" to when (e) {
                        is Event.Payload -> Diag.id8(e.from)
                        is Event.Unverified -> Diag.id8(e.from)
                        is Event.Answered -> Diag.id8(e.from)
                        is Event.ResponseCopy -> Diag.id8(e.from)
                        is Event.Late -> Diag.id8(e.from)
                        is Event.Conversed -> Diag.id8(e.from)
                        is Event.Peer -> Diag.id8(e.from)
                        else -> null
                    },
                    "bytes" to ((e as? Event.Payload)?.bytes ?: (e as? Event.Unverified)?.bytes)?.size,
                    "refused" to when (e) {
                        is Event.Answered -> e.refused
                        is Event.ResponseCopy -> e.refused
                        is Event.Late -> e.refused
                        is Event.Conversed -> Ceremony.refusalIn(e.step)
                        else -> null
                    },
                )
            }
            if (e != null && Ceremony.took(id, p, e)) {
                // a meeting's own, taken by it
            } else {
                when (e) {
                    is Event.Payload -> {
                        val from = Gateway.hex(e.from)
                        val who = if (from == peerKey) peerName else from.take(16)
                        val tell = Gateway.front.incoming(id, from, who, String(e.bytes))
                        // **told in the shade where no screen shows it**
                        // [author, 2026-11-17]: it came over the session
                        // kept, and not for a party muted or blocked
                        // [author, 2026-11-23]
                        Gateway.appContext?.let { if (tell && !Arrived.onScreen()) Arrived.post(it, who, from) }
                    }
                    // **a sender this identity cannot attribute** [author,
                    // 2026-11-19]: shown under the alias it carries, marked
                    // unverified, and never answered from here
                    is Event.Unverified -> {
                        val from = Gateway.hex(e.from)
                        val who = "${e.alias} (unverified)"
                        val tell = Gateway.front.incoming(id, from, who, String(e.bytes), unverified = true)
                        Gateway.appContext?.let { if (tell && !Arrived.onScreen()) Arrived.post(it, who, from) }
                    }
                    is Event.Connection -> {
                        // every identity's connection is recorded for itself
                        Model.connectionOf(id, e.v1)
                        if (focused) show(p, e.v1)
                    }
                    // **the nouns the kernel changed**: the shell's copy is
                    // refreshed for exactly those, and the screens watching
                    // them redraw
                    is Event.Changed -> {
                        Model.refresh(e.nouns.toSet())
                        // **what a resource counts as newly waiting, told in
                        // the shade** where it may notify [author, 2026-11-23]
                        if (uniffi.rhtn_model.Noun.RESOURCES in e.nouns) {
                            Gateway.appContext?.let { ctx ->
                                Notify.notices(ctx, id, Gateway.call("resources", { emptyList() }) { p.resources() })
                            }
                        }
                        // **a move told by a node's own record is proved as it
                        // arrives** [author, 2026-11-17]: it came on a
                        // failover session, and the next attach round may be
                        // a long way off. Off this loop, since a proof dials
                        if (uniffi.rhtn_model.Noun.SUBNETS in e.nouns) Thread { proveMoves(p) }.start()
                    }
                    // between peers (design §6.3): what a locator, a deposit, a
                    // request or a return came to, in the kernel's words
                    is Event.Peer -> {
                        Diag.event("peer.event", "from" to Diag.id8(e.from), "what" to e.what)
                        Gateway.front.note("· peer: ${e.what}")
                    }
                    is Event.Answered -> Gateway.front.note("· a response was refused: ${e.refused}")
                    is Event.Conversed -> Gateway.front.note("· ${e.what}")
                    null -> {}
                    else -> Gateway.front.note("· $e")
                }
            }
            Ceremony.tick(id, p, diag)
        }
    }

    /**
     * **An attach that got no answer at start is tried again, with
     * backoff** [author, 2026-11-04]. A session *lost* after it held is
     * the kernel's to try again, beside its judgement of the loss
     * (`Net::watch`); this chain is for the attach that never held, which
     * the kernel has no session to watch. One chain per identity; a chain
     * already running is not doubled.
     */
    internal fun reattachLater(id: String, p: Participant, scope: String?, attempt: Int) {
        if (reattaching.putIfAbsent(id, attempt) != null) return
        val wait = REATTACH_AFTER_S[minOf(attempt - 1, REATTACH_AFTER_S.size - 1).coerceAtLeast(0)]
        Diag.event("identity.reattach", "who" to id.take(8), "attempt" to attempt, "after_s" to wait)
        Thread({
            try {
                Thread.sleep(wait * 1000)
            } catch (_: InterruptedException) {
                reattaching.remove(id)
                return@Thread
            }
            reattaching.remove(id)
            attachTabs(id, p, scope, attempt)
        }, "fueros-reattach-${id.take(8)}").start()
    }

    /**
     * **A move not yet proved is dialled again** [author, 2026-11-16]: the
     * new address may not have been answering when the invitation came,
     * and the person is asked only once the node answers there as itself.
     * Every attach round asks, so the reattach backoff paces it.
     */
    internal fun proveMoves(p: Participant) {
        val waiting = Gateway.call("relocations", { emptyList() }) { p.relocations() }
            .filter { it.standing == uniffi.rhtn_model.Standing.UNPROVED }
        for (r in waiting) {
            Gateway.call("proveRelocation", { e -> Diag.warn("relocation", "why" to e.reason); null }) { p.proveRelocation(r.node) }
        }
    }

    /**
     * **Every tab the identity agreed to, attached** — the first attach the
     * session, each further one a tab on it; the connection shown is the
     * first's. Where one did not answer, [reattachLater] tries again.
     */
    internal fun attachTabs(id: String, p: Participant, scope: String?, attempt: Int) {
        proveMoves(p)
        val tabs = Gateway.call("agreed", { emptyList() }) { p.agreed() }
        if (tabs.isEmpty()) {
            Diag.event("identity.attach", "who" to id.take(8), "node" to "none")
            Model.connectionOf(id, Connection.Detached)
            return
        }
        var first: Connection? = null
        var missed = 0
        for (t in tabs) {
            val node = t.serving
            val hex = node.hex()
            val a = Gateway.call("attach", { e -> Diag.event("identity.attach", "who" to id.take(8), "node" to hex.take(8), "refused" to e.reason); null }) {
                p.attach(node, t.addresses, listOf())
            }
            Diag.event("identity.attach", "who" to id.take(8), "node" to hex.take(8), "attached" to (a != null), "attempt" to attempt)
            if (a == null) missed += 1
            if (first == null) first = if (a == null) Connection.Detached else Connection.Attached(node, a.primary)
        }
        Model.connectionOf(id, first ?: Connection.Detached)
        if (missed > 0) reattachLater(id, p, scope, attempt + 1)
    }

    /**
     * **An identity that is not in focus still receives, and still
     * attaches to a node it agreed to** — steps 3 and 4 of the browser
     * model [author, 2026-11-11]: a browser window out of focus keeps its
     * tabs open and its notifications arriving, and opens no tab it was
     * never told to. The node is the identity's own agreement, kept by the kernel,
     * written where it agreed ([serveFrom]); an identity with none —
     * one just added, or a rotation's new key before its patron — has no
     * node until it agrees to one, and is recorded detached rather than
     * handed the device's. The result is this identity's alone
     * ([Model.connectionOf]), and its loop runs on a thread of its own.
     *
     * **The node admits an identity or refuses it on its own terms**,
     * which is the author's ruling that a node serves an identity and not
     * a handset: a refusal is that identity detached, said to the
     * diagnostics and the model, and nothing else on the device changes.
     * Called by [bringUp] for every loaded identity but the focused one,
     * and by [createIdentity] and [beginRotation] for one added while the
     * kernel is up.
     */
    internal fun launchHeld(id: String, p: Participant, scope: String?) {
        // **every tab the identity agreed to** [author, 2026-11-11]: the
        // first attach is the session, each further one a tab on it, and
        // the connection shown is the first's
        attachTabs(id, p, scope, attempt = 0)
        Thread({ runLoop(p, id) }, "fueros-kernel-${id.take(8)}").start()
    }

    internal fun bringUp(shell: AndroidShell) {
        Gateway.shell = shell
        val provision = shell.read("provision")?.let {
            try { JSONObject(String(it)) } catch (_: Exception) { null }
        }
        synchronized(Gateway.lock) { provisioned = provision != null }
        Gateway.front.setProvisioned(provision != null)
        // the identity is this device's own, whether or not anybody has
        // been told about it yet
        // **`opt`, not `get`** [2026-11-04]: `getJSONArray` throws where the
        // key is absent, and the `?.let` below only ever guarded a null
        // provision. A provision naming a node and an address and nothing
        // else — which is what agreeing to an invitation writes
        // ([serveFrom]) — crashed the kernel thread on every start, so two
        // bench phones would not launch at all. A provision is a thing
        // other code writes, and every field of it is optional to this.
        val known = provision?.optJSONArray("known")
            ?.let { a -> (0 until a.length()).map { Gateway.unhex(a.getString(it)) } }
            ?: listOf()
        // **every identity this device holds, opened at once**
        // ([Identities]; author, 2026-11-07): a key rotation replaces the
        // key one subnet knows and leaves the others live, so a device
        // that has rotated holds a key still usable elsewhere.
        val app = Gateway.appContext ?: return
        var index = Identities.decode(shell.unseal(Identities.INDEX))
        // **the identity held before this model is brought into the index
        // and nothing is moved**: its scope is the top-level directory it
        // already writes, because relocating an archive would be a
        // destructive step at the one moment there is nothing to fall
        // back on ([Identities.LEGACY_SCOPE])
        val legacy = shell.unseal(Identities.LEGACY_SEEDS)?.takeIf { it.size == 64 }
        val adopt = Identities.migrated(legacy != null, legacy?.let { Identities.idOfSeeds(it) }, index)
        if (adopt is Identities.Migration.Adopt) {
            index = index + Identities.Held(adopt.id, Identities.LEGACY_SCOPE, null)
            if (!shell.seal(Identities.INDEX, Identities.encode(index))) {
                Gateway.front.setStatus("this device's storage would not keep its list of identities")
                return
            }
            Diag.event("identity.adopted")
        }
        if (index.isEmpty()) {
            Gateway.front.setStatus("no identity on this device")
            return
        }
        for (h in index) {
            Identities.openHeld(app, h, known)
        }
        // **one of them acts** ([Identities.active]): the saved choice
        // where it is still held, the first otherwise — a device holding
        // identities always has one, or a meeting would have nothing to
        // offer and no way to say why
        val saved = shell.unseal(Identities.ACTIVE)?.toString(Charsets.UTF_8)?.trim()
        // **a bench lever's choice is read here and not at the door**
        // [bench, 2026-11-11]: `chooseActive` refuses an identity that is
        // not open yet, and a lever read from the launching intent runs
        // before this loop has opened any — so the request joins the saved
        // choice, which [Identities.active] already falls back from where
        // the identity named is not held.
        Identities.activeId = Identities.active(index.filter { Gateway.loaded.containsKey(it.id) }, Identities.requestedActive ?: saved)
        Identities.requestedActive = null
        // **a departure a rotation owes is owed at every start**, since a
        // process that died between the Ceremony.adoption() and the departure left
        // the mark behind ([concludeRotation])
        Identities.concludeRotation()
        // **the identity this bring-up acts for, read once**: focus can
        // move while this runs — a person choosing on the nameplate while
        // the held identities' attaches wait out their timeouts — and a
        // second read named the newly focused identity over this one's
        // kernel, so a loop drove a meeting through a kernel holding no
        // ceremony and the window would not open [bench, 2026-11-16]
        val up = Identities.activeId
        val p = up?.let { Gateway.loaded[it] }
        if (up == null || p == null) {
            Gateway.front.setStatus("no identity on this device came up")
            return
        }
        show(p, Connection.Detached)
        // the shell's copy of the nouns, from a kernel that is up now
        Model.refreshAll()
        watchNetwork(p)
        // **a nominee arrives as an introduction** (`wire-format.md` §2.3):
        // material and a signed locator, taken as a person would take one
        // scanned, so the nominee can be offered to the counterparty's
        // prune by locator. The bench names nominees this way because a
        // phone at genesis has met nobody; material alone left the set
        // empty (run 58a6883-a5-2, 2026-10-24)
        provision?.optJSONArray("introductions")?.let { a ->
            for (k in 0 until a.length()) {
                val bytes = runCatching { Gateway.unhex(a.getString(k)) }.getOrNull() ?: continue
                Gateway.call("takeIntroduction", { e -> Gateway.front.note("· an introduction was refused: ${e.reason}") }) {
                    val who = p.takeIntroduction(bytes)
                    Gateway.front.note("· introduced: ${Gateway.hex(who).take(16)}…")
                }
            }
        }

        val material = p.material().joinToString("") { "%02x".format(it) }

        // **An unprovisioned device is the ordinary device** [author,
        // 2026-10-11]: it holds its own identity, knows nobody, is
        // attached to nothing, and meets whoever is in front of it — the
        // event loop below is what drives a ceremony and it runs either
        // way. A provision is the bench's convenience (a peer to message,
        // nominees, a node, a stream) and never the way in. Until this
        // the shell returned here, and no phone ever met without a
        // script having written a blob first.
        if (provision == null) {
            // the material is this device's public identity, shown
            // because nothing else exposes it: no flavour logs it and the
            // Report bundle carries identities as eight characters only
            Gateway.front.note("· this device's public key material:")
            Gateway.front.note("    $material")
            Gateway.front.note("· nobody known, nothing attached: a meeting crosses the bearer alone.")
        }

        // the retention this device declares (design §7.5.1), where the
        // provision names one; the kernel's own default otherwise
        val retention = provision?.optLong("retention_years", 0L) ?: 0L
        if (retention > 0L) {
            Gateway.call("set_retention_years", { e -> Gateway.front.note("· retention refused: ${e.reason}") }) {
                p.setRetentionYears(retention.toULong())
            }
        }
        // **the provision need not name anybody.** A device that has met
        // nobody is the ordinary case, and the invitation code is what
        // introduces the first counterparty ([counterparty]); a named peer
        // here is only a convenience for messaging before any meeting.
        val to = provision?.optString("peer", "")?.takeIf { it.isNotEmpty() }
            ?.let { runCatching { Gateway.unhex(it) }.getOrNull() }
        peer = to
        peerName = provision?.optString("peer_name", "the other person") ?: "the other person"
        nominees = provision?.optJSONArray("nominees")
            ?.let { a -> (0 until a.length()).map { Gateway.unhex(a.getString(it)) } }
            ?: listOf()
        peerKey = to?.let { Gateway.hex(it) }
        peerKey?.let { Gateway.front.peerKnown(up, it, peerName) }
        // **a provision may name no node, and then this device stays
        // detached** (design §6.4): a pair can Ceremony.meet() with nothing reachable
        // but each other, and what they form is the witnessless shape —
        // two identities at their genesis, no witnesses, no responses
        // (`wire-format.md` §3.2 subtype 1). The event loop below still
        // runs, because it is what drives the ceremony; only the attach
        // is skipped [author, 2026-10-07].
        val nodeHex = provision?.optString("node", "") ?: ""
        if (nodeHex.isEmpty()) {
            if (provision != null) {
                Gateway.front.note("· no node in the provision: this device stays detached,")
                Gateway.front.note("  and a meeting crosses the bearer alone (design §6.4).")
            }
            show(p, Connection.Detached)
        } else {
            val node = Gateway.unhex(nodeHex)
            val a = Gateway.call("attach", { e -> Gateway.front.setStatus("attach refused: ${e.reason}"); null }) {
                // a device that has met nobody attaches with no peers to
                // follow; the first one is learned at the meeting
                p.attach(node, listOf(provision?.optString("addr", "") ?: ""), listOfNotNull(to))
            }
            // **a node named but unreachable is the detached case, not a
            // reason to stop.** This returned, and the event loop below
            // never ran — which is the author's own rule broken twelve
            // lines above, where a provision naming no node skips the
            // attach and runs the loop anyway [author, 2026-10-07].
            //
            // It cost seven attempts at A6. The phones carried
            // `node=5046da37 addr=192.168.0.8:7447` from an earlier
            // `field-run`, and a nodeless run has nothing listening
            // there, so every one of them left the kernel with no loop:
            // no poll, no staleness watchdog, no heartbeat, and a
            // conversation that received every byte it needed and acted
            // on none of it (2026-10-24).
            if (a == null) {
                Gateway.front.note("· the node named in the provision did not answer:")
                Gateway.front.note("  this device stays detached, and a meeting crosses")
                Gateway.front.note("  the bearer alone (design §6.4).")
                show(p, Connection.Detached)
            } else {
                show(p, Connection.Attached(node, a.primary))
            }
        }
        // **the tabs the focused identity agreed to** [author, 2026-11-11]:
        // each attached beside the provision's node, a second node while
        // one is held being a tab rather than a replacement
        run {
            val scope = index.firstOrNull { it.id == up }?.scope
            var missed = 0
            proveMoves(p)
            for (t in Gateway.call("agreed", { emptyList() }) { p.agreed() }) {
                val hex = t.serving.hex()
                if (hex == nodeHex) continue
                val a = Gateway.call("attach", { e -> Gateway.front.note("· tab ${hex.take(8)}… refused: ${e.reason}"); null }) {
                    p.attach(t.serving, t.addresses, listOfNotNull(to))
                }
                if (a == null) missed += 1
                Diag.event("identity.attach", "who" to up.take(8), "node" to hex.take(8), "attached" to (a != null), "tab" to true)
            }
            // **and tried again where one did not answer** [bench, 2026-11-13:
            // the focused identity's attach timed out at start and nothing
            // retried it — the held identities' path did]
            if (missed > 0) reattachLater(up, p, scope, 1)
        }
        // **every other identity attaches and receives for itself** — step
        // 3 of the browser model [author, 2026-11-11]: the one in focus
        // stays on this thread, as it always did, and each of the rest
        // gets its own ([launchHeld])
        // **every identity's conversations, back from its own scope, and
        // written there after each change** [author, 2026-11-12]
        // **sealed through the kernel, appended per message, read when
        // opened** [author, 2026-11-14] (`light-client-requirements.md`
        // §9, APP-015): one store per identity over its own scope and its
        // own kernel's sealing; the 12th's one file migrated once
        Gateway.appContext?.let { a ->
            val stores = HashMap<String, Threads>()
            fun storeOf(by: String): Threads? {
                stores[by]?.let { return it }
                val h = index.firstOrNull { it.id == by } ?: return null
                val p = Gateway.loaded[by] ?: return null
                val shell = AndroidShell(a, h.scope)
                return Threads(shell.files(), sealerOf(p), by).also { stores[by] = it }
            }
            for (h in index) {
                val s = storeOf(h.id) ?: continue
                migrateThreads(AndroidShell(a, h.scope), s)
                Gateway.front.restore(h.id, s.index())
                Gateway.front.restorePeople(h.id, s.people())
            }
            Gateway.front.appender = { by, peer, rec -> storeOf(by)?.append(peer, rec) }
            Gateway.front.indexer = { by -> storeOf(by)?.writeIndex(Gateway.front.entries(by)) }
            Gateway.front.loader = { by, peer, before, pending -> storeOf(by)?.page(peer, before, Threads.PAGE, pending) }
            Gateway.front.peopler = { by -> storeOf(by)?.writePeople(Gateway.front.people(by)) }
        }
        for ((hid, hp) in Gateway.loaded) {
            if (hid != up) launchHeld(hid, hp, index.firstOrNull { it.id == hid }?.scope)
        }
        runLoop(p, up)
    }

    /**
     * Start the kernel, once for the life of the process.
     *
     * **A second call does nothing**, which is the whole point: a recreated
     * screen finds the kernel it left rather than starting another against
     * the same state files.
     */
    fun start(context: Context) {
        val app = context.applicationContext
        synchronized(Gateway.lock) {
            if (started) return
            started = true
            Gateway.appContext = app
        }
        Thread({ bringUp(AndroidShell(app)) }, "fueros-kernel").start()
        // **the standing the platform gives a process that keeps a
        // connection** (SES-020): asked for after the kernel is already
        // starting and never waited on, so a refusal changes nothing here
        KernelService.request(app)
        // the main thread's watchdog, in a build that has somewhere to say it
        if (Diag.enabled()) Stall.start()
    }
}