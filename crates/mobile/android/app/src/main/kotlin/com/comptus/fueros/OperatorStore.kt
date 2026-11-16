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
