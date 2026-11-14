package com.comptus.fueros

import android.content.Context
import org.json.JSONObject
import uniffi.rhtn_ffi.Answer
import uniffi.rhtn_ffi.Construction
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

/**
 * **The identities this device holds, and which of them may do what** —
 * the pure part, so a test can hold it, as [Rotation] and [Wipe] are.
 *
 * ## Why there is more than one
 *
 * **A key rotation replaces the key one subnet knows and leaves the others
 * untouched** (design §9.0, §13.6), so a device that has rotated holds a
 * key that is still live elsewhere. The author's ruling [2026-11-07]:
 * *"if we support key rotation, we wind up having to support multiple
 * identities on a device"*, and the rotated-away key *"stays useable to
 * join new subnets or to interact with any existing subnets which were
 * not rotated"*. Only on the subnet it rotated does it lose its
 * reachability and the standing that rested on it.
 *
 * ## What each may do
 *
 * **Concurrent for viewing and for acting within subnets; one at a time
 * for a meeting or an operator action** [author, 2026-11-07], chosen by
 * press and hold on the nameplate at the foot of the home screen. A
 * direct exchange continues in the identity it started in and is not
 * reassigned later. [Act] is that division, and [needsTheActive] is the
 * whole of the rule; the reason for it is the author's and is not
 * restated here.
 *
 * ## Where each one's state lives
 *
 * Every identity has its own storage, because two kernels writing one
 * archive would each overwrite the other's. The **scope** is the
 * directory under the shell's own ([Wipe.KERNEL_DIR]) that an identity's
 * files live in.
 *
 * **The identity this device already held keeps the top-level directory**
 * ([LEGACY_SCOPE]), and nothing is moved. A migration that relocated an
 * archive would be a destructive step at the one moment the device has
 * nothing to fall back on; leaving it where it is costs a null in this
 * model and no risk at all. New identities get a directory named for
 * themselves.
 */
object Identities {

    /** The sealed name the index lives under, beside the identities. */
    const val INDEX = "identities"

    /** The sealed name holding which identity acts. */
    const val ACTIVE = "identity.active"

    /**
     * What a single-identity device sealed its seeds under, before this
     * model: `Kernel` and `FirstRunActivity` both still write it for the
     * first identity, and [migrated] is what brings it into the index.
     */
    const val LEGACY_SEEDS = "seeds"

    /**
     * The scope of the identity that already held the top-level
     * directory: **no directory of its own**, so nothing had to move.
     */
    val LEGACY_SCOPE: String? = null

    /**
     * **One row and the identity that holds it** [author, 2026-11-07]:
     * *"Concurrent for viewing or acting within subnets"*, so a list that
     * merges several identities' rows has to say whose each one is.
     *
     * **Not decoration — it is what an action routes by.** A screen that
     * acted on a merged row through whichever identity happened to be
     * active would act as the wrong key, and nothing would fail: the act
     * would succeed, signed by somebody the person did not choose. So the
     * holder travels with the row and the screen acts through it, and only
     * a meeting and an operator action take the active identity
     * ([needsTheActive]).
     */
    data class Owned<T>(
        /** The id of the identity that holds [row]. */
        val by: String,
        /** The row, as that identity's kernel reported it. */
        val row: T,
    )

    /** One identity the device holds. */
    data class Held(
        /** Its keyhash, lower-case hex — what names it everywhere. */
        val id: String,
        /** The directory its state lives in, or null for [LEGACY_SCOPE]. */
        val scope: String?,
        /** The name its person gave it, where they gave one. */
        val alias: String?,
    )

    /** What an identity is being asked to do. */
    enum class Act {
        /** Reading what a subnet holds. Every identity, at once. */
        VIEWING,

        /** Acting inside a subnet it belongs to. Every identity, at once. */
        WITHIN_SUBNET,

        /** A direct exchange, which belongs to the identity it began in. */
        CONVERSATION,

        /** A meeting. The active identity alone. */
        MEETING,

        /** An operator action. The active identity alone. */
        OPERATOR,

        /** Attaching to a node — opening a tab. The active identity alone
         *  [author, 2026-11-11], and the node it agrees to is remembered as
         *  that identity's own, never the device's. */
        ATTACH,

        /** Starting a direct exchange where none is under way — a new tab.
         *  The active identity alone; continuing one is [CONVERSATION]. */
        NEW_THREAD,
    }

    /**
     * **Whether the act takes the active identity rather than any held
     * one** [author, 2026-11-07].
     *
     * A conversation is neither: it is bound to the identity it started
     * in ([CONVERSATION]), which is a third answer and not the active
     * one — switching which identity acts must not move a conversation
     * already under way.
     */
    fun needsTheActive(act: Act): Boolean = when (act) {
        Act.MEETING, Act.OPERATOR, Act.ATTACH, Act.NEW_THREAD -> true
        Act.VIEWING, Act.WITHIN_SUBNET, Act.CONVERSATION -> false
    }

    /** The scope an identity's state lives in, which is its id. */
    fun scopeFor(id: String): String = id

    /**
     * **Whether `id` is a name this model will accept**: 64 lower-case hex
     * digits, which is also what makes it safe as a directory name — the
     * scope is this string, and anything else could name a path.
     */
    fun names(id: String): Boolean =
        id.length == 64 && id.all { it in '0'..'9' || it in 'a'..'f' }

    /**
     * **Where each identity's colour is kept** [author, 2026-11-12]: a
     * device file, one `id rrggbb` line per identity, beside the index
     * rather than in it, so the index's own form does not move.
     */
    const val COLOURS = "identity.colours"

    /** The colours file read: id to ARGB. A line that is not one is skipped. */
    fun colours(bytes: ByteArray?): Map<String, Int> {
        val text = bytes?.toString(Charsets.UTF_8) ?: return emptyMap()
        val out = LinkedHashMap<String, Int>()
        for (line in text.lines()) {
            val id = line.substringBefore(' ')
            val hex = line.substringAfter(' ', "").trim()
            if (!names(id) || hex.length != 6 || !hex.all { it in '0'..'9' || it in 'a'..'f' }) continue
            out[id] = (0xFF000000L or hex.toLong(16)).toInt()
        }
        return out
    }

    /** The colours file written. */
    fun encodeColours(colours: Map<String, Int>): ByteArray =
        colours.entries.joinToString("") { (id, c) -> "$id ${"%06x".format(c and 0xFFFFFF)}\n" }.toByteArray()

    /**
     * **Every identity held, coloured**: the stored colour where there is
     * one, else the first of [Palette.IDENTITY] no identity on the device
     * uses yet, in the order held — so a new identity takes an unused
     * colour, and the same one each time until it is chosen otherwise.
     * Returns a map the caller writes back when it differs from [stored].
     */
    fun assigned(stored: Map<String, Int>, held: List<Held>): Map<String, Int> {
        val out = LinkedHashMap<String, Int>()
        for (h in held) stored[h.id]?.let { out[h.id] = it }
        for (h in held) {
            if (h.id in out) continue
            val used = out.values.toSet()
            out[h.id] = Palette.IDENTITY.map { it.second }.firstOrNull { it !in used }
                ?: Palette.IDENTITY[out.size % Palette.IDENTITY.size].second
        }
        return out
    }

    /** An identity's colour, or the rail's own where it is not held here. */
    fun colourOf(colours: Map<String, Int>, id: String?): Int =
        id?.let { colours[it] } ?: Palette.RAIL_ORB

    /** The palette's name for a colour, for the choice on Options. */
    fun colourName(argb: Int): String = Palette.IDENTITY.firstOrNull { it.second == argb }?.first ?: "Colour"

    /**
     * **The index, as one line each**: the id, the scope or `-`, and the
     * alias as the rest of the line.
     *
     * An alias may hold spaces and may not hold a newline, which
     * [Identity.aliasRefusal] does not forbid and this does: a name with
     * one in it would write a second record.
     */
    fun encode(held: List<Held>): ByteArray =
        held.filter { names(it.id) && it.alias?.contains('\n') != true }
            .joinToString("") { h ->
                "${h.id} ${h.scope ?: "-"} ${h.alias ?: ""}\n"
            }
            .toByteArray()

    /** The index back, dropping any line this model would not have written. */
    fun decode(bytes: ByteArray?): List<Held> {
        val text = bytes?.toString(Charsets.UTF_8) ?: return listOf()
        val out = mutableListOf<Held>()
        for (line in text.lines()) {
            if (line.isBlank()) continue
            val id = line.substringBefore(' ')
            if (!names(id)) continue
            val rest = line.substringAfter(' ', "")
            val scope = rest.substringBefore(' ')
            val alias = rest.substringAfter(' ', "").ifEmpty { null }
            // a duplicate id is one identity, however the file got two
            if (out.any { it.id == id }) continue
            out += Held(id, if (scope == "-") null else scope, alias)
        }
        return out
    }

    /** What a start has to do about the identity held before this model. */
    sealed class Migration {
        /** The index already holds it, or there is nothing to hold. */
        object None : Migration()

        /** Add the legacy identity to the index under this id. */
        data class Adopt(val id: String) : Migration()
    }

    /**
     * **Bring an identity sealed before this model into the index**, by
     * its id, keeping [LEGACY_SCOPE] so its archive stays where it is.
     *
     * **Idempotent, because a start may die in the middle of it**: the
     * decision is taken from what is on disk each time, so redoing it
     * changes nothing and skipping it once loses nothing.
     */
    fun migrated(hasLegacySeeds: Boolean, legacyId: String?, index: List<Held>): Migration =
        when {
            !hasLegacySeeds || legacyId == null || !names(legacyId) -> Migration.None
            index.any { it.id == legacyId } -> Migration.None
            else -> Migration.Adopt(legacyId)
        }

    /**
     * **Which identity acts**, given what is held and what was last
     * chosen.
     *
     * The saved choice where it is still held; the first held otherwise,
     * which is the case after the chosen one is taken off the device —
     * **a device holding identities must always have one that acts**, or
     * a meeting would have nothing to offer and no way to say why.
     * Nothing where nothing is held.
     */
    fun active(held: List<Held>, saved: String?): String? =
        held.firstOrNull { it.id == saved }?.id ?: held.firstOrNull()?.id

    /**
     * **How an identity is named in the list** — its alias, or the first
     * eight of its id where it has none, which is the same rendering
     * PRD-14 fixes for a party: never a name this device was not given.
     */
    fun label(h: Held): String = h.alias?.takeIf { it.isNotBlank() } ?: h.id.take(8)

    /**
     * **The door to another identity is a row of the list, not a screen
     * of its own** [author, 2026-11-09]: *"a nice way to present this
     * visually would be to have <New Identity> as a list item … which
     * would also mean the drop down never had only one item and wouldn't
     * necessitate adding a new screen/button."* Before this, a device
     * holding one identity answered the press and hold with a notice and
     * nothing opened, and nothing anywhere reached [Kernel.createIdentity]
     * past the first run.
     */
    const val ANOTHER = "<New Identity>"

    /**
     * The one that acts, marked, as the selector draws the list, and
     * [ANOTHER] last — so the list never holds one item and a choice
     * between identities is never displaced by the door.
     */
    fun listed(held: List<Held>, active: String?): List<String> =
        held.map { h -> (if (h.id == active) "● " else "") + label(h) } + ANOTHER

    /** Whether the row chosen at [which] is [ANOTHER] rather than one held. */
    fun adding(held: List<Held>, which: Int): Boolean = which == held.size

    /**
     * **Why this identity cannot be taken off this device on its own**, or
     * null where it can [author, 2026-11-10: a single-identity delete, on
     * the Options screen].
     *
     * Three refusals, each for a different reason:
     *
     * - **It is not held.** Nothing to do, and saying so beats a silent
     *   success.
     * - **It is the only one.** A device holding identities always has one
     *   that acts ([active]), and emptying the index by this door would
     *   leave a device that is neither wiped nor usable. Taking the last
     *   one off is the device's own door ([Wipe]), which clears the
     *   at-rest key as well and is confirmed as the destructive act it is.
     * - **Its storage is the device's own** ([LEGACY_SCOPE]). The identity
     *   held before this model writes the top level, where the index, the
     *   active choice and the provision also live, so removing it is not a
     *   directory to delete — and inventing a list of its files is how a
     *   half-removal happens. It can be rotated, and the device can be
     *   wiped; it cannot be singled out.
     */
    fun removable(held: List<Held>, id: String): String? {
        val it = held.firstOrNull { h -> h.id == id }
            ?: return "that identity is not on this device"
        if (held.size <= 1) {
            return "this is the only identity on this device; wiping the device is the other door"
        }
        if (it.scope == LEGACY_SCOPE) {
            return "this identity's storage is the device's own and cannot be taken away on its own"
        }
        return null
    }

    /** The index without `id`, and the scope whose storage goes with it. */
    fun without(held: List<Held>, id: String): Pair<List<Held>, String?> =
        held.filter { it.id != id } to held.firstOrNull { it.id == id }?.scope

    /**
     * **What removing one identity takes**, said once where the person
     * decides. The other identities are untouched, which is the whole
     * difference from the device's wipe.
     */
    fun removes(label: String): String =
        "Remove $label from this device: its key, everything it signed or was " +
            "shown, and every capture sealed to it. The other identities here are " +
            "untouched. Nothing is sent anywhere and nobody is told, and the seed " +
            "phrase is the only way back."

    /**
     * **What an act refused for want of an identity says.** Only
     * [Act.MEETING] and [Act.OPERATOR] can be refused this way, and only
     * on a device holding none — a device holding any always has one that
     * acts ([active]).
     */
    fun refusal(act: Act): String? = when (act) {
        Act.MEETING -> "No identity on this device to meet as."
        Act.OPERATOR -> "No identity on this device to act as."
        else -> null
    }

    /**
     * **Which identity acts** — a meeting's and an operator action's
     * ([Identities.needsTheActive]), chosen by press and hold on the
     * nameplate. Null only where the device holds none.
     */
    @Volatile internal var activeId: String? = null

    @Volatile internal var requestedActive: String? = null

    /**
     * **The departure a concluded rotation owes** [author, 2026-11-07]:
     * *"the rotation completes, the new key is adopted by patron P, and
     * then the prior key departs P"* — in that order, so the person is
     * never briefly outside the subnet they are rotating inside (design
     * §9.1).
     *
     * **The mark is the trigger and forgetting it is the completion**, the
     * same shape the rest of the rotation uses: this runs at every start
     * and at the end of every meeting, does nothing where there is no
     * mark, and does nothing where the new identity holds no line yet —
     * which is the state between beginning a rotation and meeting the
     * patron. A process that dies in the middle leaves the mark, and the
     * next start owes the same act.
     *
     * **Nothing is guessed about which subnet.** The patron is the one
     * the new identity's own line names; a new identity's archive is new
     * ([beginRotation]), so the line it holds is the Ceremony.adoption() the
     * rotation just made.
     */
    internal fun concludeRotation() {
        val app = Kernel.appContext ?: return
        for (h in identities(app)) {
            val own = AndroidShell(app, h.scope)
            val priorId = own.unseal(Rotation.MARK_SUCCEEDS)
                ?.toString(Charsets.UTF_8)?.trim()
                ?.takeIf { Identities.names(it) }
                ?: continue
            val fresh = Kernel.loaded[h.id] ?: continue
            // the rotation has not reached its Ceremony.adoption() yet: nothing owed
            val patron = Kernel.call("lines", { null }) { fresh.lines().firstOrNull()?.patron }
                ?: continue
            val old = Kernel.loaded[priorId] ?: run {
                // the key being succeeded is not open here — taken off the
                // device, or never held. Nothing can sign its departure,
                // and the mark is spent rather than retried forever
                own.forget(Rotation.MARK_SUCCEEDS)
                Kernel.front.note("· the key this rotation succeeded is not on this device, so its departure was not signed")
                return@run null
            } ?: continue
            val said = Kernel.call("depart", { e -> e.reason }) {
                old.depart(patron, null)
                null
            }
            if (said == null) {
                own.forget(Rotation.MARK_SUCCEEDS)
                Diag.event("rotation.departed", "who" to priorId.take(8))
                Kernel.front.note("· the key you rotated away from has left that subnet")
            } else {
                // **left for the next start**: the mark stays, so the
                // departure is owed rather than lost
                Kernel.front.note("· that departure was refused and is still owed: $said")
            }
        }
    }

    /**
     * **Begin a key rotation, in a way a dying process cannot spoil**
     * (design §9.0, §9.1; PRD-03).
     *
     * The two seed pairs are sealed as a mark *before* anything is opened,
     * and [bringUp] obeys a mark over the sealed seeds — so the rotation
     * either has not begun or will be finished by some later start, and
     * there is no state in which the archive on disk belongs to a key
     * nothing names. The mark is cleared only after the new seeds are the
     * sealed ones.
     *
     * **Both halves or neither**: `wire-format.md` §4.1 admits no variant
     * of a `Recovery` carrying one, so a mark without its prior key is
     * dropped rather than half-performed.
     *
     * False where custody refused either, and nothing was begun. The
     * caller's copies are this function's to wipe.
     */
    fun beginRotation(context: Context, newSeeds: ByteArray, priorSeeds: ByteArray): Boolean {
        require(newSeeds.size == 64) { "the kernel's seeds are 64 bytes" }
        require(priorSeeds.size == 64) { "a prior identity is 64 bytes of seed" }
        val app = context.applicationContext
        // **the new key is a new identity, with an archive of its own**
        // [author, 2026-11-07]: *"just start a new archive for the new
        // identity … The old archive should show the old ID leaving the
        // subnet that rotated, but should otherwise remain unchanged."*
        // So the marks are sealed in the **new** identity's scope, not the
        // rotating one's: [openOn] then performs the rotation on empty
        // storage, and the key being succeeded opens from its own
        // directory untouched.
        val newId = idOfSeeds(newSeeds) ?: return false
        val priorId = idOfSeeds(priorSeeds) ?: return false
        if (newId == priorId) return false
        val scope = Identities.scopeFor(newId)
        val own = AndroidShell(app, scope)
        // the prior key first: a mark whose new seeds landed and whose
        // prior did not is the half-mark [openOn] has to drop, so make it
        // the harder half to be missing
        if (!own.seal(Rotation.MARK_PRIOR, priorSeeds)) return false
        if (!own.seal(Rotation.MARK_NEW, newSeeds)) {
            own.forget(Rotation.MARK_PRIOR)
            return false
        }
        // **which key it succeeds, kept for the departure that follows**
        // [author, 2026-11-07]: the old key leaves the rotated subnet once
        // the new one is adopted into its place, and the marks that hold
        // the prior seeds are forgotten the moment the rotation completes
        own.seal(Rotation.MARK_SUCCEEDS, priorId.toByteArray())
        // **and the index names it**, so a start opens it beside the one
        // it succeeds rather than instead of it
        val device = AndroidShell(app)
        val index = Identities.decode(device.unseal(Identities.INDEX))
        if (index.none { it.id == newId } &&
            !device.seal(Identities.INDEX, Identities.encode(index + Identities.Held(newId, scope, null)))
        ) {
            own.forget(Rotation.MARK_NEW)
            own.forget(Rotation.MARK_PRIOR)
            own.forget(Rotation.MARK_SUCCEEDS)
            return false
        }
        newSeeds.fill(0)
        priorSeeds.fill(0)
        Diag.event("rotation.begun")
        if (Attachments.started) {
            openHeld(app, Identities.Held(newId, scope, null), listOf())
            Kernel.loaded[newId]?.let { Attachments.launchHeld(newId, it, scope) }
            chooseActive(context, newId)
        } else {
            Attachments.start(context)
        }
        return true
    }

    /**
     * **The seeds this device holds**, for a rotation to sign its
     * `Recovery` with (design §9.0: *"the old key is in hand wherever the
     * rotation carries its inheritance, since it signs the `Recovery`"*).
     *
     * **The one caller is [beginRotation]'s screen**, and the copy is the
     * caller's to wipe. Nothing else has business reading these: a seed
     * crosses no wire (design §23.3), and this returns it only so the
     * device can sign with the key it already has rather than asking a
     * person to type a phrase it holds.
     */
    fun heldSeedsForRotation(context: Context): ByteArray? {
        // **the identity being rotated is the one that acts** ([Identities]:
        // a rotation is a meeting, and a meeting takes the active
        // identity), so its seeds come from its own scope and not the
        // top-level directory a single-identity device used
        val app = context.applicationContext
        val id = activeId
        val scope = if (id == null) {
            Identities.LEGACY_SCOPE
        } else {
            val held = identities(app).firstOrNull { it.id == id } ?: return null
            held.scope
        }
        return AndroidShell(app, scope).unseal(Identities.LEGACY_SEEDS)?.takeIf { it.size == 64 }
    }

    /**
     * **This device's archive and store, enveloped** (design §13.7.1,
     * `rhtn_client::backup`): what a device loss takes away, under a key
     * derived from the identity, so the blob is unreadable wherever it is
     * kept until the identity is in hand again.
     *
     * The identity is **not** in it: that is the one thing a backup must
     * not carry, and the seed phrase is how it comes back.
     */
    fun exportBackup(by: String): ByteArray? {
        val p = Kernel.loaded[by] ?: return null
        return Kernel.call("exportBackup", { null }) { p.exportBackup() }
    }

    /**
     * **Open a backup into this client** (`rhtn_client::backup`;
     * `light-client-requirements.md` §2): the blob opens under this
     * identity, the import scans it, and what came back is reported —
     * records taken, and captures the scan discarded as past the window
     * their *subject* declared (design §13.7.1).
     *
     * Refused where this client already holds an archive, where the blob
     * belongs to another identity, or where it does not open; the kernel's
     * own words come back rather than a guess at them.
     *
     * **Waits for the kernel**, as the first run calls this moments after
     * starting it.
     */
    fun restoreBackup(blob: ByteArray): Result<uniffi.rhtn_ffi.Restored> {
        val until = System.currentTimeMillis() + 30_000
        var p = activeId?.let { Kernel.loaded[it] }
        while (p == null && System.currentTimeMillis() < until) {
            Thread.sleep(200)
            p = activeId?.let { Kernel.loaded[it] }
        }
        val held = p ?: return Result.failure(IllegalStateException("the kernel did not start"))
        return runCatching { held.restoreBackup(blob) }
    }

    /**
     * The device's own seeds, as the first run sealed them.
     *
     * **Under the shell's own custody and never around it**: these are the
     * whole of the identity, and they never cross the kernel's storage
     * seam, so the kernel's sealing does not reach them — they are
     * Keystore-wrapped here, exactly as the storage key itself is
     * (`light-client-requirements.md` §9).
     *
     * **Nothing is minted here since 2026-10-29** [author]: the first run
     * is a choice between a new identity and one restored from its
     * phrase, and [createIdentity] is where the seeds arrive. A kernel
     * started with none is a shell that skipped that screen, which is a
     * fault to say rather than paper over.
     */
    internal fun heldSeeds(shell: AndroidShell): ByteArray {
        val s = shell.unseal(Identities.LEGACY_SEEDS)
        check(s != null && s.size == 64) { "no identity on this device: the first run was not completed" }
        return s
    }

    /**
     * **Open one of the identities this device holds**, on storage of its
     * own ([AndroidShell]'s scope): two kernels over one directory would
     * each overwrite the other's archive.
     *
     * **A mark outranks the seeds** ([Rotation.start]), per identity: a
     * rotation that began and may not have finished is performed here,
     * because `Participant.rotating` opens the new identity on the
     * storage it is handed — so between beginning one and sealing its new
     * seeds the archive belongs to a key the seeds do not name, and a
     * start that trusted the seeds would open the wrong identity on the
     * right archive.
     */
    internal fun openHeld(app: Context, h: Identities.Held, known: List<ByteArray>) {
        val own = AndroidShell(app, h.scope)
        val p = openOn(own, known) ?: return
        Kernel.loaded[h.id] = p
    }

    /** The keyhash a seed pair names, or null where it does not read. */
    internal fun idOfSeeds(seeds: ByteArray): String? =
        Kernel.call("keyhashOfSeeds", { null }) { uniffi.rhtn_ffi.keyhashOfSeeds(seeds) }
            ?.takeIf { Identities.names(it) }

    /**
     * The participant for the identity whose storage `own` is, or null
     * where it would not open. The rotation logic is per identity: see
     * [openHeld].
     */
    internal fun openOn(shell: AndroidShell, known: List<ByteArray>): Participant? {
        // **a mark outranks the seeds** ([Rotation.start]): a rotation that
        // began and may not have finished is performed here, because
        // `Participant.rotating` opens the new identity on the storage it
        // is handed — so between beginning one and sealing its new seeds
        // the archive belongs to a key the seeds do not name, and a start
        // that trusted the seeds would open the wrong identity on the
        // right archive
        val mark = shell.unseal(Rotation.MARK_NEW)
        val p = when (
            Rotation.start(
                shell.unseal(Identities.LEGACY_SEEDS)?.size == 64,
                mark?.size == 64,
            )
        ) {
            is Rotation.Start.Unstarted -> {
                // **one identity's seeds being gone is that identity's
                // fact and not the device's** [2026-11-07]: the device
                // may hold others, and [bringUp] says so once where none
                // of them came up
                Diag.warn("identity", "op" to "open", "error" to "no seeds under this scope")
                return null
            }
            is Rotation.Start.Rotate -> {
                // bound here rather than asserted: the predicate above read
                // the same value, and a `!!` would say this code knows
                // something the compiler cannot check
                val fresh = mark ?: return null
                val prior = shell.unseal(Rotation.MARK_PRIOR)
                if (prior == null || prior.size != 64) {
                    // **half a mark is no mark**: without the prior key
                    // there is no rotation to make (`wire-format.md` §4.1
                    // admits no variant carrying one half), so the mark is
                    // dropped and the device starts as it was
                    shell.forget(Rotation.MARK_NEW)
                    Kernel.front.note("· a rotation was begun without its prior key and was dropped")
                    Kernel.call("Participant.start", { e -> Kernel.front.setStatus("kernel refused: ${e.reason}"); null }) {
                        Participant.start(heldSeeds(shell), known, platformOf(shell))
                    } ?: return null
                } else {
                    val out = Kernel.call("Participant.rotating", { e ->
                        Kernel.front.setStatus("the rotation was refused: ${e.reason}"); null
                    }) {
                        Participant.rotating(fresh.copyOf(), prior.copyOf(), null, known, platformOf(shell))
                    }
                    prior.fill(0)
                    if (out == null) return null
                    // **the new seeds become the identity only now**, and
                    // the mark goes after them: cleared first and the
                    // process dying would leave an archive nothing names
                    if (!shell.seal(Identities.LEGACY_SEEDS, fresh)) {
                        Kernel.front.setStatus("the rotation held but its seeds could not be kept")
                        return null
                    }
                    shell.forget(Rotation.MARK_PRIOR)
                    shell.forget(Rotation.MARK_NEW)
                    fresh.fill(0)
                    Diag.event("rotation.done")
                    Kernel.front.note("· the rotation completed and this device holds its new key")
                    out
                }
            }
            is Rotation.Start.Ordinary ->
                Kernel.call("Participant.start", { e -> Kernel.front.setStatus("kernel refused: ${e.reason}"); null }) {
                    Participant.start(heldSeeds(shell), known, platformOf(shell))
                } ?: return null
        }
        return p
    }

    /** `dir` and everything under it gone, or false where some of it stays. */
    internal fun clear(dir: java.io.File): Boolean {
        if (!dir.exists()) return true
        dir.listFiles()?.forEach { if (it.isDirectory) clear(it) else it.delete() }
        return dir.deleteRecursively() || !dir.exists()
    }

    /**
     * **Take the identity off this device** ([Wipe]; PRD-03): the key,
     * the archive, the store, every sealed capture, the shell's own
     * preference stores, and last the Keystore key that wrapped them, so
     * a byte that survived a delete cannot be read.
     *
     * **The kernel handle goes first**, before anything is deleted: a
     * live `Participant` writes through the shell's custody, and a writer
     * running against a half-deleted directory could put a file back.
     * The caller kills the process afterwards for the same reason — the
     * one guarantee that nothing in flight outlives this.
     *
     * Returns what would **not** go, for [Wipe.left] to say. An empty
     * list is a device holding no identity.
     */
    fun wipe(context: Context): List<String> {
        Kernel.loaded.clear()
        activeId = null
        Ceremony.meet = null
        val app = context.applicationContext
        val left = mutableListOf<String>()
        if (!clear(java.io.File(app.filesDir, Wipe.KERNEL_DIR))) left += Wipe.KERNEL_DIR
        if (!clear(java.io.File(app.filesDir, Wipe.DIAG_DIR))) left += Wipe.DIAG_DIR
        for (store in Wipe.STORES) {
            val ok = runCatching {
                app.getSharedPreferences(store, Context.MODE_PRIVATE).edit().clear().commit()
            }.getOrDefault(false)
            // **and the file itself**, not only its contents: a cleared
            // store still names itself on disk
            runCatching { app.deleteSharedPreferences(store) }
            if (!ok) left += store
        }
        // **the wrapping key last of all**: while it exists, anything
        // recovered from the storage it wrapped is readable, and once it
        // is gone nothing is (`light-client-requirements.md` §6 gives the
        // same reasoning for a deleted archive window's capture keys)
        val alias = runCatching {
            val ks = java.security.KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            ks.deleteEntry(Wipe.AT_REST_ALIAS)
            !ks.containsAlias(Wipe.AT_REST_ALIAS)
        }.getOrDefault(false)
        if (!alias) left += Wipe.AT_REST_ALIAS
        Diag.event("identity.wiped")
        return left
    }

    /** Whether the first run has been completed on this device. */
    fun hasIdentity(context: Context): Boolean {
        val device = AndroidShell(context.applicationContext)
        // the index, or the identity a device held before there was one
        return Identities.decode(device.unseal(Identities.INDEX)).isNotEmpty() ||
            device.unseal(Identities.LEGACY_SEEDS)?.size == 64
    }

    /**
     * **The first run hands the seeds over and the kernel starts on them**
     * ([FirstRunActivity]): sealed under the shell's custody first, so a
     * process that dies between the two finds them at its next start.
     * False where the storage refused them, and nothing started.
     */
    fun createIdentity(context: Context, seeds: ByteArray): Boolean {
        require(seeds.size == 64) { "the kernel's seeds are 64 bytes" }
        val app = context.applicationContext
        val device = AndroidShell(app)
        val id = idOfSeeds(seeds) ?: return false
        val index = Identities.decode(device.unseal(Identities.INDEX))
        // **adding, not replacing** [author, 2026-11-07]: *"'Add an
        // identity' … is just another trigger for 'Start a new
        // identity'"*, so this door is reached from the first run and
        // again later, and an identity already held stays held.
        if (index.any { it.id == id }) return true
        // **its own storage, named for itself**: two kernels over one
        // directory would each overwrite the other's archive. The
        // top-level directory belongs to whatever identity a device held
        // before this model ([Identities.LEGACY_SCOPE]) and is never
        // taken by a new one.
        val scope = Identities.scopeFor(id)
        if (!AndroidShell(app, scope).seal(Identities.LEGACY_SEEDS, seeds)) return false
        // **the seeds land before the index names them**, so a process
        // that dies between leaves an identity on disk that nothing opens
        // rather than an index naming storage that is not there
        if (!device.seal(Identities.INDEX, Identities.encode(index + Identities.Held(id, scope, null)))) {
            return false
        }
        Diag.event("identity.added")
        if (Attachments.started) {
            // already running: open it beside the others rather than
            // waiting for a restart, and let it act since a person who
            // just made an identity means to use it
            openHeld(app, Identities.Held(id, scope, null), listOf())
            Kernel.loaded[id]?.let { Attachments.launchHeld(id, it, scope) }
            chooseActive(context, id)
        } else {
            Attachments.start(context)
        }
        return true
    }

    fun chooseActive(context: Context, id: String): Boolean {
        if (!Kernel.loaded.containsKey(id)) return false
        activeId = id
        AndroidShell(context.applicationContext).seal(Identities.ACTIVE, id.toByteArray())
        Diag.event("identity.active", "who" to id.take(8))
        activeId?.let { Kernel.loaded[it] }?.let { Attachments.show(it, Connection.Detached) }
        Model.refreshAll()
        return true
    }

    /** Ask the next bring-up to make `id` the one that acts. */
    fun requestActive(id: String) {
        requestedActive = id.trim().takeIf { Identities.names(it) }
    }

    /**
     * **Choose the identity that acts** — a meeting's and an operator
     * action's ([Identities.needsTheActive]), from the nameplate's own
     * list [author, 2026-11-07]. Nothing where `id` is not held: a
     * selector offering what the device does not hold is a bug, and
     * switching to it would leave the device unable to Ceremony.meet().
     */
    /**
     * **Take one identity off this device, leaving the others** [author,
     * 2026-11-10]. The refusals are [Identities.removable]'s and are
     * returned as they are; null means it went.
     *
     * **The index is rewritten before the storage is deleted**, which is
     * the opposite order from [createIdentity] and for the same reason: a
     * process that dies between leaves storage nothing opens rather than
     * an index naming storage that is not there. Half-removed is the
     * failure to design against, and an orphaned directory is the
     * harmless half.
     *
     * **A rotation owed in that identity's scope stops it.** The marks say
     * a prior key is still to depart ([Rotation.MARK_PRIOR]); deleting the
     * identity that owes the departure would leave the old key standing in
     * a subnet it has been replaced in, with nothing on the device able to
     * say so.
     */
    fun removeIdentity(context: Context, id: String): String? {
        val app = context.applicationContext
        val device = AndroidShell(app)
        val index = Identities.decode(device.unseal(Identities.INDEX))
        Identities.removable(index, id)?.let { return it }
        val (kept, scope) = Identities.without(index, id)
        val own = AndroidShell(app, scope)
        if (own.unseal(Rotation.MARK_PRIOR) != null) {
            return "this identity owes a departure from a rotation; finish that first"
        }
        if (!device.seal(Identities.INDEX, Identities.encode(kept))) {
            return "this device's storage would not keep its list of identities"
        }
        // nothing opens it from here on: the index no longer names it
        Kernel.loaded.remove(id)
        val left = scope?.let { java.io.File(app.filesDir, "${Wipe.KERNEL_DIR}/$it").deleteRecursively() } ?: false
        if (activeId == id) {
            Identities.active(kept.filter { Kernel.loaded.containsKey(it.id) }, null)?.let { chooseActive(context, it) }
        }
        Diag.event("identity.removed", "who" to id.take(8), "storage" to left.toString())
        Model.refreshAll()
        return if (left) {
            null
        } else {
            "the identity is off this device's list; some of its storage would not go"
        }
    }

    /** The colour chosen for one identity on Options. */
    fun setIdentityColour(context: Context, id: String, argb: Int) {
        val shell = AndroidShell(context.applicationContext)
        val now = LinkedHashMap(identityColours(context))
        now[id] = argb
        runCatching { shell.write(Identities.COLOURS, Identities.encodeColours(now)) }
            .onFailure { Kernel.front.note("· the colour could not be kept: ${it.message}") }
    }

    /**
     * **Every identity held, with its colour** [author, 2026-11-12]: the
     * device's colours file, each identity without one given the first
     * unused and the file written back where that happened.
     */
    fun identityColours(context: Context): Map<String, Int> {
        val shell = AndroidShell(context.applicationContext)
        val stored = Identities.colours(runCatching { shell.read(Identities.COLOURS) }.getOrNull())
        val now = Identities.assigned(stored, identities(context))
        if (now != stored) runCatching { shell.write(Identities.COLOURS, Identities.encodeColours(now)) }
        return now
    }

    /**
     * **The identity in focus, named at a call** [restructure step 4,
     * 2026-11-14]: the person's choice of which identity acts, a UI
     * input and nothing the kernel reasons from. Every function that acts
     * for an identity takes it as its first argument; a screen that acts
     * for the focused one passes this, and a screen given its identity
     * passes that. Empty where the device holds none, which every
     * function answers as it answered a missing focus.
     */
    fun focused(): String = activeId ?: ""

    /** Which identity acts, or null where the device holds none. */
    fun activeIdentity(): String? = activeId

    /** Every identity this device holds, in the order the index names. */
    fun identities(context: Context): List<Identities.Held> =
        Identities.decode(AndroidShell(context.applicationContext).unseal(Identities.INDEX))
}