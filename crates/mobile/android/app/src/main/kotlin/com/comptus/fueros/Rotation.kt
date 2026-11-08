package com.comptus.fueros

/**
 * **The pure part of changing a key** (PRD-03), so a test can hold it, as
 * [Chrome], [Engagement] and [Backup] are.
 *
 * There is little to decide and one thing easy to get wrong: what to call
 * a party in a warning. PRD-14 forbids showing a name this device was not
 * given, and a warning naming nobody is a warning about nothing, so this
 * is where the two meet.
 */
object Rotation {

    /** What custody holds a rotation's new seeds under. */
    const val MARK_NEW = "rotation.new"

    /** And the prior identity's, which the rotation needs to sign with. */
    const val MARK_PRIOR = "rotation.prior"

    /**
     * **Which key the new identity succeeds**, kept beside the marks
     * [author, 2026-11-07]: the old key leaves the subnet that rotated
     * once the new one is adopted into its place, and the marks holding
     * the prior seeds are forgotten the moment the rotation completes —
     * so the pairing has to outlive them to be acted on.
     */
    const val MARK_SUCCEEDS = "rotation.succeeds"

    /** What a start does, given what custody holds. */
    sealed class Start {
        /** No rotation outstanding: the sealed seeds are the identity. */
        object Ordinary : Start()

        /**
         * A rotation was begun and may not have finished. **Perform it**,
         * and only when it returns does its new identity become the one
         * the sealed seeds name.
         */
        object Rotate : Start()

        /** Nothing to start from: the first run has not happened. */
        object Unstarted : Start()
    }

    /**
     * **The mark is the authority while it exists**, which is the whole of
     * what makes a rotation survive a dying process [2026-11-05].
     *
     * `Participant.rotating` opens the new identity on this device's own
     * storage and saves, so between beginning a rotation and sealing its
     * new seeds there is a window where the archive on disk belongs to a
     * key the sealed seeds do not name. A start that trusted the seeds in
     * that window would open the wrong identity on the right archive.
     *
     * So: **a mark outranks the seeds.** It is written before the rotation
     * runs and deleted after the new seeds are sealed, so the window where
     * the archive is new and nothing says so does not exist. Re-entering
     * [Start.Rotate] after a crash is the correct behaviour rather than a
     * hazard: the rotation is over the same two keys and reaches the same
     * place.
     */
    fun start(hasSeeds: Boolean, hasMark: Boolean): Start = when {
        hasMark -> Start.Rotate
        hasSeeds -> Start.Ordinary
        else -> Start.Unstarted
    }

    /** How many characters of a keyhash stand in for a name. */
    const val SHORT = 8

    /**
     * **What to call a party in a sentence**: the alias they published,
     * where this device holds one, and their keyhash otherwise.
     *
     * **Never a name this device invented** (PRD-14,
     * `light-client-requirements.md` §4.3): an alias is a party's own
     * published claim, and where none has arrived the keyhash is what
     * this device honestly has. A blank, or an invented "your patron",
     * would make the author's sentences name nobody.
     */
    fun name(keyhash: String, alias: String?): String {
        val said = alias?.trim().orEmpty()
        return if (said.isEmpty()) "${keyhash.take(SHORT)}…" else said
    }
}
