package com.comptus.fueros

import uniffi.rhtn_model.Cost
import uniffi.rhtn_model.Limit
import uniffi.rhtn_model.Line
import uniffi.rhtn_model.Loss
import uniffi.rhtn_model.LossReason

/**
 * **What the screen says before a person ends an authority relationship**
 * — the pure part, so a test can hold it (`light-client-requirements.md`
 * §6; the screens sheet's E3, PRD-02, UX-006).
 *
 * §6's requirement is *warn if the resulting topology change will revoke
 * access to resources the user currently uses*, and the kernel computes
 * that as of 2026-10-28. What is here is the other half: saying it in a
 * way somebody can act on, and **never saying more than was computed**.
 *
 * **The hard case is not the long list, it is the empty one.** A cost that
 * came to nothing and a cost that could not be worked out both arrive here
 * as an empty list of losses, and they mean opposite things: the first is
 * *leaving is free as far as this device can see*, the second is *this
 * device cannot tell you*. The screens sheet asks for a stale permission
 * list **identified as stale, never silently used**, and that distinction
 * is the whole of it — so [computed] decides which sentence the screen
 * leads with, and the limits are printed whether or not anything is lost.
 *
 * **The list is a floor in every case.** Whether a resource admits you is
 * the resource's own rule, evaluated where it is hosted, and no client
 * holds one; so [LIMIT_PREDICATE] is said every time, last, where it reads
 * as the standing condition it is rather than as news.
 *
 * The screen itself is [LeavingActivity]; nothing here touches a view.
 */
object Leaving {

    /**
     * How much of a keyhash is shown where one stands for a party.
     *
     * **The same width in the row and in the button**, because they name
     * the same party and the act is not undoable: two widths would have
     * the person comparing a prefix against a shorter prefix of itself.
     * No alias is shown here at all — a patron that published one is
     * named by it on People, and this screen is about the line rather
     * than the person.
     */
    const val SHORT = 12

    /**
     * **A party, named**: the alias this device was told, and the keyhash
     * with it [author, 2026-10-28].
     *
     * **Both, never one.** The keyhash alone is no way to recognise
     * somebody before an act that cannot be undone, which is what this
     * screen showed until the author ruled otherwise. The alias alone is
     * no way either: it is what a party signed about themselves, nothing
     * stops two of them signing the same, and it is this device's
     * received copy rather than a fact. So the name leads and the
     * keyhash settles it.
     */
    fun party(who: String, alias: String?): String =
        if (alias.isNullOrBlank()) {
            "${who.take(SHORT)}…"
        } else {
            "$alias, ${who.take(SHORT)}…"
        }

    /** One line this key holds, as the roll draws it, with the patron's
     *  alias where one is held. */
    fun line(l: Line, alias: String?): String = "${party(l.patron.hex(), alias)}   series ${l.series}"

    /**
     * What to say where this key is under no patron at all.
     *
     * **Not a roll**, as the other screens have: a line here is tapped to
     * ask what leaving it costs, so the screen builds them one at a time
     * and this is only the empty case. A `roll` joining them into one
     * block would be a path nothing takes.
     */
    const val NONE =
        "This key is under no patron, so there is nothing to leave. " +
            "A patron adopts you at a meeting; the Meet screen is where that starts."

    /**
     * **Whether the answer is an answer.** False where this device holds
     * no edge for the relationship in its own copy of the topology — its
     * own adoption reaches it by propagation, so a key that has not been
     * told the shape of its own line cannot subtract it. Everything the
     * screen says about cost is conditioned on this.
     */
    fun computed(c: Cost): Boolean = c.limits.none { it is Limit.NotInTable }

    /** `1 resource` / `4 resources`. */
    private fun many(n: Int, one: String, more: String): String =
        if (n == 1) "1 $one" else "$n $more"

    /** The verb that agrees with it, which a hardcoded one did not. */
    private fun verb(n: Int, one: String, more: String): String = if (n == 1) one else more

    /** How many leave the horizon, with the verb they take. */
    private fun went(n: Int): String =
        "${many(n, "person", "people")} ${verb(n, "leaves", "leave")} your horizon"

    /** The sentence the screen leads with. */
    fun headline(c: Cost): String = when {
        !computed(c) ->
            "What leaving costs cannot be worked out on this device."
        c.losing.isEmpty() && c.leaving.isEmpty() ->
            "Nothing this device can see goes with it."
        c.losing.isEmpty() ->
            "No resource this device knows of goes, but ${went(c.leaving.size)}."
        else ->
            "${many(c.losing.size, "resource", "resources")} " +
                "${verb(c.losing.size, "goes", "go")}, and ${went(c.leaving.size)}."
    }

    /** What is kept, where anything is and the answer is one. */
    fun kept(c: Cost): String? = when {
        !computed(c) || c.keeping == 0u -> null
        else -> "${many(c.keeping.toInt(), "resource", "resources")} " +
            "${verb(c.keeping.toInt(), "stays", "stay")} within reach."
    }

    /**
     * One resource that goes, and why it goes. **The two reasons are
     * different things to be told**: a host out of reach is nothing left
     * to ask, while a host still in view whose owner has gone is a door
     * that is still there and will not open.
     */
    /** One resource that goes, its host and owner named from `names`. */
    fun lost(r: Loss, names: Map<String, String> = emptyMap()): String {
        val why = if (r.reason == LossReason.HOST_LEAVES) {
            "the node serving it leaves your horizon: ${party(r.node.hex(), names[r.node.hex()])}"
        } else {
            "its owner leaves your horizon: ${party(r.owner.hex(), names[r.owner.hex()])} — " +
                "the node stays, and admission is theirs to gate"
        }
        val named = r.instance.ifBlank { r.resource.hex().take(SHORT) + "…" }
        return "$named   ${r.serviceType}\n    $why"
    }

    /**
     * **Why the figure above is a floor**, in the order that changes what
     * a person should believe: what cannot be computed first, then what is
     * known to be missing from the list, then the condition that always
     * holds.
     */
    fun limits(c: Cost): List<String> {
        val out = mutableListOf<String>()
        if (!computed(c)) out.add(LIMIT_UNCOMPUTED)
        for (l in c.limits) {
            if (l is Limit.SeveralLines) {
                out.add(
                    "This relationship runs on ${l.lines} lines in this device's copy, " +
                        "and one departure ends one of them. What is above is the cost " +
                        "of ending them all.",
                )
            }
        }
        val stale = c.limits.filterIsInstance<Limit.Stale>()
        if (stale.isNotEmpty()) {
            out.add(
                "${many(stale.size, "node's list", "nodes' lists")} could not be " +
                    "refreshed when this device last asked, so what is counted there " +
                    "is what was counted then.",
            )
        }
        val cut = c.limits.filterIsInstance<Limit.Truncated>()
        if (cut.isNotEmpty()) {
            out.add(
                "${many(cut.size, "node", "nodes")} " +
                    "${verb(cut.size, "holds", "hold")} more than " +
                    "${verb(cut.size, "it", "they")} will list at once, so " +
                    "${verb(cut.size, "it serves", "they serve")} things this device " +
                    "has never seen.",
            )
        }
        val unswept = c.limits.filterIsInstance<Limit.Unswept>()
        if (unswept.isNotEmpty()) {
            out.add(
                "${many(unswept.size, "node", "nodes")} within reach " +
                    "${verb(unswept.size, "has", "have")} never been asked what " +
                    "${verb(unswept.size, "it serves", "they serve")}, so whatever " +
                    "that is, it is not counted above.",
            )
        }
        if (c.limits.any { it is Limit.PredicateNotHeld }) out.add(LIMIT_PREDICATE)
        return out
    }

    /** The whole warning, as the screen shows it. */
    /** The whole warning; `names` are the aliases held, by keyhash. */
    fun words(c: Cost, names: Map<String, String> = emptyMap()): String {
        val parts = mutableListOf(headline(c))
        kept(c)?.let { parts.add(it) }
        if (c.losing.isNotEmpty()) {
            parts.add("")
            parts.add(c.losing.joinToString("\n") { lost(it, names) })
        }
        if (c.leaving.isNotEmpty() && computed(c)) {
            parts.add("")
            parts.add(
                "Out of your horizon: " +
                    c.leaving.joinToString(", ") { k -> names[k.hex()] ?: "${k.hex().take(SHORT)}…" },
            )
        }
        val why = limits(c)
        if (why.isNotEmpty()) {
            parts.add("")
            parts.add(why.joinToString("\n") { "· $it" })
        }
        return parts.joinToString("\n")
    }

    /**
     * **What the button says, and what the second tap means.** Leaving is
     * a right rather than a request (design §6.2) — the old patron does
     * not sign and cannot refuse — so nothing here gates it on the cost
     * having been read. It is two taps because it is not undoable, not
     * because anybody must approve.
     */
    fun button(who: String, alias: String?, armed: Boolean): String {
        // **the name alone on the button**, the keyhash being on the row
        // above and in the warning: a button is read at a glance and a
        // prefix of a hash is not read at all
        val name = alias ?: "${who.take(SHORT)}…"
        return if (armed) "Tap again to leave $name" else "Leave $name"
    }

    /** Said every time, because it is true every time. */
    const val LIMIT_PREDICATE =
        "Whether a resource admits you is its own rule, held where it is hosted and " +
            "not on this device: something still within reach may refuse you once the " +
            "line is gone. What is above is the least that goes, not the whole of it."

    /** Said where the topology this device holds has no edge to subtract. */
    const val LIMIT_UNCOMPUTED =
        "Nobody has told this device the shape of this relationship, so there is " +
            "nothing here to subtract and no figure to give. Attach to a serving node " +
            "and let it reach you before reading anything into an empty list."
}
