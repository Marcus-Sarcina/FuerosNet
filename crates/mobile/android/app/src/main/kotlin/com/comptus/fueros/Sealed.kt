package com.comptus.fueros

import android.content.Context

/**
 * **A package's storage sealed against an image of its machine**
 * (`infra-client-requirements.md` §9.4, `light-client-requirements.md`
 * §10) [author, 2026-11-22], reopened without the operator being asked.
 *
 * The operator chose the seal when they installed the package, knowing
 * that package is unavailable after its machine restarts or comes back
 * from a backup until a device of theirs reaches the node. **So whenever
 * this app reaches a node it runs, it reads which packages are closed and
 * reopens each one**, with the key derived here from the signing seed and
 * the resource. Nothing on any screen names it unless it fails.
 */
object Sealed {

    /**
     * **The closed ones in a `sealed` answer**: one line per sealed
     * package, its resource and `open` or `closed`. Pure, for a JVM test.
     */
    fun closed(said: String): List<String> =
        said.lines()
            .map { it.trim().split(" ") }
            .filter { it.size == 2 && it[1] == "closed" && it[0].length == 64 }
            .map { it[0] }

    /**
     * **Reopen every closed package on the node at `at`.** Null where
     * nothing was closed or every one reopened; otherwise what failed, for
     * the caller to show. Blocking: off the main thread.
     */
    fun reopen(context: Context, by: String, at: String, token: String): String? {
        val said = Provision.act(at, token, "sealed", emptyList()).getOrNull() ?: return null
        val failed = mutableListOf<String>()
        for (resource in closed(said)) {
            val key = Gateway.packageSealKey(by, resource)
            if (key == null) {
                failed.add("${resource.take(16)}: this device could not derive its key")
                continue
            }
            val done = Provision.act(at, token, "reopen", listOf("resource" to resource, "seal" to key))
            if (done.isSuccess) {
                Diag.event("sealed.reopened", "at" to at, "resource" to resource.take(16))
            } else {
                val why = done.exceptionOrNull()?.message ?: "no answer"
                Diag.warn("sealed.reopened", "at" to at, "resource" to resource.take(16), "why" to why)
                failed.add("${resource.take(16)}: $why")
            }
        }
        return if (failed.isEmpty()) null else "a sealed package on $at did not reopen:\n" + failed.joinToString("\n")
    }
}
