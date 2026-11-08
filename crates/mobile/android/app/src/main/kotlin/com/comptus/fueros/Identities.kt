package com.comptus.fueros

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
        Act.MEETING, Act.OPERATOR -> true
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

    /** The one that acts, marked, as the selector draws the list. */
    fun listed(held: List<Held>, active: String?): List<String> =
        held.map { h -> (if (h.id == active) "● " else "  ") + label(h) }

    /** What the selector says above the list. */
    fun words(): String =
        "Every identity here reads and acts in its own subnets at the same time, " +
            "and a conversation stays with the identity it started in. A meeting or " +
            "an action on your own node uses the one marked here."

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
}
