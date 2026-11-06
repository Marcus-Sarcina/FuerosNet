package com.comptus.fueros

/**
 * **What the end screen says about who witnessed the record** — the pure
 * part, so a test can hold it, as [Chrome], [Engagement] and [Backup] are.
 *
 * PRD-19 wants three cases told apart: one witnessed by both sides'
 * survivors, one by the counterparty's alone, and one by nobody. The list
 * of nominees beside this says who each was and what they answered; what
 * this says is **how many attested and how the `nominated_by` split
 * fell**, which a list does not tell a person without counting.
 *
 * **None of the three is a fault, and the words must not imply one**
 * (PRD-19). A witness that was not reachable, a nominee who declined, a
 * horizon with nobody in it — none is the person's doing, and a record
 * with no witness *finalises* and says so permanently (design §8.1). So
 * these sentences state what happened and never that something went
 * wrong: there is no "failed", no "only", no "unfortunately".
 */
object Witnessing {

    /**
     * **How many attested, and whose they were.** `theirs` is the
     * counterparty's display name, already decided — an alias they
     * published or their keyhash, never a name this device invented
     * (PRD-14, and [Rotation.name] does the choosing).
     *
     * Returns null where nobody attested: that case is the witnessless
     * one, which the screen says in its own words rather than as a count
     * of zero.
     */
    fun split(mine: Int, others: Int, theirs: String): String? {
        val all = mine + others
        if (all == 0) return null
        val count = "$all witness${if (all == 1) "" else "es"} attest" +
            (if (all == 1) "s" else "") + " this meeting"
        return when {
            // **both sides' survivors**: the split is the thing to say,
            // because it is what an evaluator reading the record later
            // weighs (design §8.1.2)
            mine > 0 && others > 0 ->
                "$count: ${nominees(mine)} of yours and ${nominees(others)} of $theirs's."
            // **the counterparty's alone.** Said as a fact about who was
            // reachable, with no suggestion that this side fell short
            mine == 0 ->
                "$count, all nominated by $theirs. None of yours is among them — " +
                    "yours were not reachable, which the record notes and does not hold " +
                    "against anyone."
            // **and this side's alone**, the same way round
            else ->
                "$count, all nominated by you. None of $theirs's is among them — " +
                    "theirs were not reachable, which the record notes and does not hold " +
                    "against anyone."
        }
    }

    private fun nominees(n: Int): String = "$n nominee${if (n == 1) "" else "s"}"
}
