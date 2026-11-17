package com.comptus.fueros

import android.content.Context

/**
 * **The program on a launched machine that limits what its node may ask
 * of it** (`infra-client-requirements.md` §9.4), kept current without the
 * operator ever being asked [author, 2026-11-17].
 *
 * A machine keeps the runner it was launched with, and this app carries a
 * newer one as it gains acts. The operator has no reason to know either:
 * **before any act that asks the machine, a machine behind this app is
 * given this app's runner first**, signed with the operator's own key,
 * which this client holds and the node does not. Nothing on any screen
 * names it.
 */
object Runner {

    /**
     * **Whether to bring this app's runner first**: where the machine's is
     * older than this app's or not known, and never to one that predates
     * refreshing (version 1), which cannot take it. Pure, for a JVM test.
     */
    fun behind(machine: Int, mine: Int): Boolean = machine != 1 && machine < mine

    /** What a machine whose runner predates refreshing is told, where an
     *  act needs more than it can do: a fresh node is the remedy. */
    const val EARLIER = "this machine was launched by an earlier Fueros that cannot do this: " +
        "launch a new one and invite its members there instead"

    /**
     * **Bring the machine at `at` this app's runner, where it is behind**,
     * and record what it then runs. Null where nothing was needed or the
     * machine took it; otherwise why not, for the caller to say only if
     * the act it was for fails as well — [EARLIER] for a machine that
     * cannot take one. Blocking: off the main thread.
     */
    fun bring(context: Context, by: String, at: String, token: String): String? {
        val machine = OperatorStore.runnerOf(context, by, at)
        if (machine == 1) return EARLIER
        if (!behind(machine, Launch.RUNNER_VERSION)) return null
        val key = Gateway.hostRunnerPrivateKey(by)
            ?: return "this device could not derive the key for that machine"
        val signed = Gateway.hostRunnerRefresh(by)
            ?: return "this device could not sign its copy for that machine"
        val said = Provision.act(
            at,
            token,
            "runner-refresh",
            listOf("key" to key, "runner" to signed.runner, "signature" to signed.signature),
        )
        val now = said.getOrNull()?.let { OperatorStore.runnerSaid(it) }
        if (now != null) {
            OperatorStore.recordRunner(context, by, at, now)
            Diag.event("runner.brought", "at" to at, "from" to machine, "to" to now)
            return null
        }
        val why = said.exceptionOrNull()?.message ?: said.getOrNull()?.trim() ?: "no answer"
        Diag.warn("runner.brought", "at" to at, "from" to machine, "why" to why)
        return why
    }
}
