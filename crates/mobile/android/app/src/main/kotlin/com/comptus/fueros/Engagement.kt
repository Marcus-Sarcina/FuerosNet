package com.comptus.fueros

/**
 * **What to do after one attempt at engaging this side's nominees** — the
 * pure part, so a test can hold it, as [Chrome] and [Leaving] are.
 *
 * [author, 2026-11-05] ruled this out of [Kernel]: the retry and the words
 * were inside a private function that needed a live `Participant`, so
 * PRD-17's second and third clauses — *"the unreachable one is retried
 * while its binding is fetched"* and *"the person is told how many of the
 * nominated are watching"* — could not be reached by a unit test at all.
 * Nothing about the policy needed a kernel; only asking how many were
 * reached did.
 *
 * **What is still not here**: that the engagement happens *before the code
 * is shown* (PRD-17's first clause) is the order [Kernel] calls things in,
 * not a decision this makes, and the trace owed to each nominee is
 * `OpticalExchange`'s, already held by
 * `a_whole_exchange_traces_every_part_it_moved_and_no_content`.
 */
object Engagement {

    /** How many turns the engagement is given while the bindings land. */
    const val TRIES = 12

    /** The wait between them, in milliseconds. */
    const val WAIT_MS = 250L

    /** What follows one attempt. */
    sealed class Next {
        /**
         * Nothing more and nothing said. Either nobody was nominated — a
         * meeting with no witnesses to engage owes no sentence about them
         * — or the caller has already been told why it stopped.
         */
        object Stop : Next()

        /** Say this to the person, and stop. */
        data class Say(val note: String) : Next()

        /**
         * **Wait and ask again**, which is the retry PRD-17 requires: a
         * nominee whose session material is not held yet is being fetched,
         * and a transient absence is not an unavailable witness
         * (`light-client-requirements.md` §1.1).
         */
        object Again : Next()
    }

    /**
     * The decision, given what the kernel reported and which attempt this
     * was.
     *
     * **Every nominee reached ends it**, and the person is told how many
     * are watching. **The last attempt ends it too**, saying how many were
     * reached and that the rest will not appear — stated as a fact about
     * reachability and not as a fault of theirs. Anything else waits.
     */
    fun next(engaged: UInt, nominated: UInt, attempt: Int, tries: Int = TRIES): Next = when {
        engaged >= nominated && nominated > 0u ->
            Next.Say("$engaged of $nominated nominees are engaged and watching.")
        engaged >= nominated -> Next.Stop
        attempt >= tries - 1 -> Next.Say(
            "$engaged of $nominated nominees are engaged; " +
                "the rest were not reachable and will not appear.",
        )
        else -> Next.Again
    }
}
