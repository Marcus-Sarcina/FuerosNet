package com.comptus.fueros

import android.content.Context
import android.content.SharedPreferences

/**
 * **The operator's own records, one file per identity**: the instances
 * an identity launched or enrolled, the token each was given, the
 * invitations it sent.
 *
 * **And the records written before there was one file per identity**
 * [found 2026-11-16]. Until 2026-11-14 they were kept in one file,
 * `operator`, for whichever identity acted; the move to `operator-<id>`
 * read the new file and nothing moved into it, so a phone that had
 * launched nodes showed none, while the droplets ran on. [prefs] adopts
 * them the first time an identity opens its file: each old instance
 * whose subnet is one this identity holds, or which names no subnet,
 * moves across with its token, and leaves the old file. What another
 * identity's subnet holds stays for that identity.
 */
object OperatorStore {

    /** The one file every identity's records were kept in before. */
    const val LEGACY: String = "operator"

    /**
     * **One instance this operator administers**, as its record keeps
     * it: where it is, and the subnet it serves by the label it was
     * enrolled under (`light-client-requirements.md` §10). The list is
     * the shell's own state — an operator in several subnets runs one
     * instance in each, and nothing here infers an instance from a
     * position.
     */
    data class Instance(
        val at: String,
        /** The subnet it serves, hex, or null for the one under this
         *  operator's own key. Null too where it is [unassigned]. */
        val subnet: String?,
        val label: String,
        /** The provider it was launched at and its id there, where this
         *  client launched it (H1); blank where it was enrolled by hand. */
        val provider: String = "",
        val remoteId: String = "",
        /** The runner version this machine was given, where this client
         *  knows it: from its launch here, or from a refresh since; 1 for a
         *  machine launched here before the version was recorded, 0 where
         *  nothing says. */
        val runner: Int = 0,
        /**
         * **Serving no subnet** [author, 2026-11-21]: enrolled while its
         * operator rooted none and named none, so neither of its records
         * is signed. It stays so until its operator assigns it to a
         * subnet, from the Operator screen or as the answer to the
         * question every adoption it makes as patron asks while it holds
         * one [author, 2026-11-21] (`infra-client-requirements.md` §4.4);
         * nothing assigns it by where its operator happens to be.
         */
        val unassigned: Boolean = false,
        /** **What its records will name**, as its enrolment was told: the
         *  address published (blank where none was, and the anchor entry
         *  names [at]), and the provider's ASN and zone where the launch
         *  knew them. Kept so that the records an assignment signs later
         *  say what enrolment's would have [author, 2026-11-06]. */
        val endpoint: String = "",
        val asn: Long? = null,
        val zone: String = "",
    )

    /** The record's marker, in its seventh field, for an [Instance.unassigned]. */
    private const val SERVES_NONE = "none"

    /**
     * **An instance's record, read**: tab-separated, in the order
     * [Instance] declares its fields. Pure, for a JVM test.
     */
    fun parse(line: String): Instance {
        val parts = line.split('\t')
        return Instance(
            parts[0],
            parts.getOrNull(1)?.ifEmpty { null },
            parts.getOrElse(2) { parts[0] },
            parts.getOrElse(3) { "" },
            parts.getOrElse(4) { "" },
            runnerIn(line),
            parts.getOrNull(6) == SERVES_NONE,
            parts.getOrElse(7) { "" },
            parts.getOrNull(8)?.toLongOrNull(),
            parts.getOrElse(9) { "" },
        )
    }

    /** **An instance's record, written**: what [parse] reads back. */
    fun line(i: Instance): String = listOf(
        i.at,
        i.subnet ?: "",
        i.label,
        i.provider,
        i.remoteId,
        if (i.runner > 0) i.runner.toString() else "",
        if (i.unassigned) SERVES_NONE else "",
        i.endpoint,
        i.asn?.toString() ?: "",
        i.zone,
    ).joinToString("\t")

    /** The instances this identity administers. */
    fun instances(context: Context, by: String): List<Instance> =
        prefs(context, by).getString("instances", "").orEmpty()
            .lines()
            .filter { it.isNotBlank() }
            .map { parse(it) }

    /** Keep `i`, in place of any record at the same address. */
    fun remember(context: Context, by: String, i: Instance) {
        val kept = instances(context, by).filter { it.at != i.at } + i
        prefs(context, by).edit()
            .putString("instances", kept.joinToString("\n") { line(it) })
            .apply()
    }

    /** Drop the record at `at`. */
    fun forget(context: Context, by: String, at: String) {
        val kept = instances(context, by).filter { it.at != at }
        prefs(context, by).edit()
            .putString("instances", kept.joinToString("\n") { line(it) })
            .apply()
    }

    /**
     * **The instances an assignment is offered for** [author, 2026-11-21]:
     * every one serving no subnet, by the record's own flag, with an
     * address to reach it at. What the Operator screen offers to assign
     * and what an adoption as patron asks about, until none is left.
     * Pure, for a JVM test.
     */
    fun unassigned(instances: List<Instance>): List<Instance> =
        instances.filter { it.unassigned && it.at.isNotEmpty() }

    /**
     * **A node as an adoption carries it**: where members dial it, and the
     * keyhash it answers as, hex [author, 2026-11-22] — the member has no
     * other way to learn whose it is where it is further up the chain than
     * the patron.
     */
    data class Carried(val at: String, val node: String)

    /**
     * **The node an adoption about to be made carries to the member**, as
     * members dial it and whose it is, or null for none [author,
     * 2026-11-21]: *"auto-invite via the bluetooth channel during an
     * adoption into a subnet where a node already exists."* The kernel
     * carries it beside the intent and the member holds it as an
     * invitation once the adoption is recorded.
     *
     * **Only as patron, and its own node first**, answering as this
     * identity (`me`): the instance chosen at [Meet.Choice.SERVE_FROM],
     * else the newest record serving the subnet the adoption is into —
     * [Meet.Scope.OWN]'s being the one under this operator's own key, and
     * a record naming no subnet serving the one that key roots (`root`),
     * that line being its. An instance serving no subnet is nobody's to be
     * invited to.
     *
     * **Else the node this identity is itself served from there**
     * (`served`) [author, 2026-11-22]: a member is served from the nearest
     * infrastructure node on its patron chain (`light-client-requirements.md`
     * §4), so a patron with none of its own carries the one above it. Only
     * into a subnet already held: a new one has no node above it. Pure,
     * for a JVM test.
     */
    fun meetingNode(
        adopt: Meet.Adopt,
        scope: Meet.Scope?,
        serving: Meet.Serving?,
        instances: List<Instance>,
        me: String,
        root: String? = null,
        served: Carried? = null,
    ): Carried? {
        if (adopt != Meet.Adopt.THEM_UNDER_ME || scope == null) return null
        serving?.at?.let { return Carried(Meet.Serving.dialled(it), me) }
        val subnet = scope.id.takeIf { it.length == 64 }?.lowercase()
        if (subnet == null && scope != Meet.Scope.OWN) return null
        val rooted = root?.lowercase()
        instances
            .lastOrNull {
                val serves = it.subnet?.lowercase()
                !it.unassigned && it.at.isNotEmpty() && (serves == subnet || (serves == null && subnet != null && subnet == rooted))
            }
            ?.let { return Carried(Meet.Serving.dialled(it.at), me) }
        if (subnet == null) return null
        return served?.takeIf { it.at.isNotEmpty() && it.node.length == 64 }
    }

    /**
     * **The subnets an unassigned instance can be assigned to**: each one
     * this key holds a position in, the kernel signing on any such line,
     * once apiece. Neither [Meet.Scope.OWN] nor [Meet.Scope.INDIVIDUAL] is
     * a subnet with an id, so neither is offered; empty where none is
     * held, and a subnet then comes from an adoption. Pure, for a JVM test.
     */
    fun assignable(held: List<Meet.Scope>): List<Meet.Scope> =
        held.filter { it.id.length == 64 }.distinctBy { it.id }

    /** **`i`'s record once it serves `to`**: the subnet named, labelled
     *  as the chooser labels it, and no longer unassigned. */
    fun assigned(i: Instance, to: Meet.Scope): Instance =
        i.copy(subnet = to.id, unassigned = false, label = to.label())

    /**
     * **Assign the unassigned instance `i` to the subnet `to`** [author,
     * 2026-11-21], wherever the operator chose it: [Gateway.assign] signs
     * both records on that subnet's line and hands them over, and only
     * where the instance took both is its record rewritten as [assigned].
     * Anything short of that leaves it unassigned. Off the main thread:
     * it is several round trips.
     */
    fun assign(context: Context, by: String, i: Instance, token: String, to: Meet.Scope): Gateway.Placed {
        val placed = Gateway.assign(by, i.at, token, to.id, i.endpoint.ifEmpty { null }, i.asn, i.zone.ifEmpty { null })
        if (placed.taken) remember(context, by, assigned(i, to))
        return placed
    }

    /** This identity's records, with any old ones that are its adopted. */
    fun prefs(context: Context, by: String): SharedPreferences {
        val mine = context.getSharedPreferences("operator-$by", Context.MODE_PRIVATE)
        runCatching { adopt(context, mine, Gateway.subnets(by).map { it.id.hex() }.toSet()) }
            .onFailure { Diag.warn("operator.adopt", "why" to (it.message ?: it.javaClass.simpleName)) }
        return mine
    }

    private fun adopt(context: Context, mine: SharedPreferences, subnets: Set<String>) {
        val legacy = context.getSharedPreferences(LEGACY, Context.MODE_PRIVATE)
        val held = legacy.getString("instances", null) ?: return
        val (taken, left) = split(held.lines(), subnets)
        if (taken.isEmpty()) return
        val have = mine.getString("instances", "").orEmpty().lines().filter { it.isNotBlank() }
        val haveAt = have.map { it.substringBefore('\t') }.toSet()
        val merged = have + taken.filter { it.substringBefore('\t') !in haveAt }
        val edit = mine.edit().putString("instances", merged.joinToString("\n"))
        val old = legacy.edit()
        for (line in taken) {
            val at = line.substringBefore('\t')
            legacy.getString("token:$at", null)?.let { t ->
                if (!mine.contains("token:$at")) edit.putString("token:$at", t)
                old.remove("token:$at")
            }
        }
        edit.apply()
        if (left.isEmpty()) old.remove("instances") else old.putString("instances", left.joinToString("\n"))
        old.apply()
        Diag.event("operator.adopt", "taken" to taken.size.toString(), "left" to left.size.toString())
    }

    /**
     * **Record the runner a machine now has** against its instance, in the
     * record's sixth field, so [Runner] knows when this client carries a
     * newer one.
     */
    fun recordRunner(context: Context, by: String, at: String, version: Int) {
        val mine = context.getSharedPreferences("operator-$by", Context.MODE_PRIVATE)
        val lines = mine.getString("instances", "").orEmpty().lines().filter { it.isNotBlank() }
        val updated = lines.map { line ->
            val parts = line.split('\t').toMutableList()
            if (parts.firstOrNull() != at) return@map line
            while (parts.size < 6) parts += ""
            parts[5] = version.toString()
            parts.joinToString("\t")
        }
        mine.edit().putString("instances", updated.joinToString("\n")).apply()
    }

    /** The version a runner's answer names (`runner <n> <digest>`), or null. */
    fun runnerSaid(text: String): Int? =
        Regex("""(?m)^runner (\d+) [0-9a-f]{64}$""").find(text)?.groupValues?.get(1)?.toIntOrNull()

    /**
     * **The runner a record says its machine has**: the sixth field; else
     * 1 for a machine this client launched before it recorded one, which
     * is the runner those were given; else 0, not known. Pure, for a JVM
     * test.
     */
    fun runnerIn(line: String): Int {
        val parts = line.split('\t')
        return parts.getOrNull(5)?.toIntOrNull() ?: if (parts.getOrElse(3) { "" }.isNotEmpty()) 1 else 0
    }

    /** The runner the record for the instance at `at` says, or 0. */
    fun runnerOf(context: Context, by: String, at: String): Int =
        prefs(context, by).getString("instances", "").orEmpty().lines()
            .firstOrNull { it.split('\t').firstOrNull() == at }
            ?.let { runnerIn(it) } ?: 0

    /**
     * **Which old records are this identity's**: those whose subnet (the
     * second tab-separated field) is one it holds, and those naming none.
     * Pure, so a JVM test holds it.
     */
    fun split(lines: List<String>, subnets: Set<String>): Pair<List<String>, List<String>> {
        val real = lines.filter { it.isNotBlank() }
        val taken = real.filter { line ->
            val s = line.split('\t').getOrNull(1).orEmpty()
            s.isEmpty() || s.lowercase() in subnets
        }
        return taken to (real - taken.toSet())
    }
}
