package com.comptus.fueros

/**
 * **What this client owes somebody in words, by the kind of thing it is
 * about** — the pure part, so a test can hold it, as [Chrome] is.
 *
 * [author, 2026-11-05]: *"The disclosure page is parameterized boilerplate
 * by transaction type… what a meeting, peering, patron relationship etc
 * implies is fixed across all occurrences."* If the words are fixed per
 * kind then no occurrence needs testing: one test says every kind has its
 * words, and one says the right words are chosen for the kind. That is
 * what this table is for, and `Robot/disclosure-tests.md` has the whole
 * set it discharges.
 *
 * **It is a register of provenance before it is a store of text.** Most of
 * these words already exist and are already parameterized — `Meet.brief`
 * returns items keyed by the row they satisfy, `Leaving.words` is written
 * per cost, `Chrome` decides what a rung says. Copying them here would
 * give the shell two copies to drift apart, so a topic whose words have a
 * home says where that home is, and a topic with none says what it owes
 * and which row asks for it. Nothing here invents a sentence: an [Owed]
 * topic shows the person nothing today, exactly as it does now, and the
 * test fails loudly rather than the screen saying something nobody wrote.
 *
 * **Why the gaps are not filled in here** [author, 2026-11-05]: the
 * missing sentences are the same text that is to be *"translated into a
 * more approachable language and then hidden behind expandable
 * headings"*, so writing them now would be composing them twice. They are
 * the author's to write, each listed with the obligation it has to meet.
 */
object Disclosure {

    /**
     * **What a disclosure is about.** The transaction kinds, and the other
     * moments at which this client owes somebody words — a backup, a
     * restore, a first brokered use, a provisioning choice. The acceptance
     * rows name all of these; nothing here is a screen, because two
     * screens can be about one kind and one screen about two.
     */
    enum class Topic {
        /** A presence ceremony: what becomes durable and who may read it. */
        MEETING,

        /** Peering, where a meeting carries it. */
        PEERING,

        /** An adoption: that it moves authority, and what that costs. */
        ADOPTION,

        /** Taking a new patron whose policies reach upward. */
        ADOPTION_UPWARD,

        /** Leaving a patron: what access goes with it. */
        DEPARTURE,

        /** A plain rotation, which abandons what the old key stood on. */
        ROTATION,

        /** A recovery, which publishes the link between the two keys. */
        RECOVERY,

        /** Holding one patron only, and what a second would protect. */
        SECOND_PATRON,

        /** Making a backup, and what losing the device costs. */
        BACKUP,

        /** A restored archive, and what its completeness rests on. */
        RESTORE,

        /** First use of a brokered resource, and whose session it is. */
        BROKERED,

        /** Provisioning: what the ordering of providers and zones means. */
        PROVISIONING,

        /** What an operator's configuration exposes those below them to. */
        OPERATOR_EXPOSURE,

        /** A record nobody witnessed, said as a fact and not a fault. */
        WITNESSLESS,
    }

    /** One obligation and the words that meet it, as `Meet.Item` is. */
    data class Item(val row: String, val text: String)

    /** Where a topic's words are, or that they are owed. */
    sealed class Says {
        /** Words this table holds, fixed by the topic. */
        data class Here(val items: List<Item>) : Says()

        /**
         * Words another module already holds and already parameterizes.
         * [where] names it so a test can go and ask it rather than
         * holding a second copy that drifts.
         */
        data class Elsewhere(val row: String, val where: String) : Says()

        /**
         * **Words nobody has written.** [row] is the acceptance row that
         * asks for them and [obligation] is what it asks, in its own
         * words, so what is owed is a sentence and not a specification.
         */
        data class Owed(val row: String, val obligation: String) : Says()
    }

    /**
     * **The table.** Total over [Topic] by construction — a `when` with no
     * else — which is the first half of what the tests check; the second
     * is that nothing comes back [Says.Owed].
     */
    fun of(topic: Topic): Says = when (topic) {
        Topic.MEETING -> Says.Elsewhere(
            "UX-001",
            "Meet.brief: what becomes durable and who may read it, and the fields withheld",
        )
        Topic.PEERING -> Says.Elsewhere(
            "UX-001",
            "Meet.brief, where peeringAsked() holds",
        )
        Topic.ADOPTION -> Says.Elsewhere(
            "PRD-01",
            "MeetActivity: \"An adoption moves authority. What it costs is listed before " +
                "either of you agrees.\"",
        )
        Topic.DEPARTURE -> Says.Elsewhere(
            "PRD-02",
            "Leaving.words: the headline, what is kept, what is lost and why, per cost",
        )
        Topic.WITNESSLESS -> Says.Elsewhere(
            "PRD-19",
            "MeetActivity: \"No witness attests this meeting… the record still finalises, " +
                "and says so permanently\"",
        )

        // ---- owed, and each one a sentence the author is to write ------

        Topic.ADOPTION_UPWARD -> Says.Owed(
            "PRD-02",
            "that this patron's policies reach upward, before the resources whose " +
                "predicates do so are listed",
        )
        Topic.ROTATION -> Says.Owed(
            "PRD-03",
            "that a plain rotation abandons the standing the prior identity held",
        )
        Topic.RECOVERY -> Says.Owed(
            "PRD-03",
            "that a recovery publishes the link to the prior identity within the horizon, " +
                "so the two are no longer unlinkable",
        )
        Topic.SECOND_PATRON -> Says.Owed(
            "PRD-04",
            "that a second independent adoption protects against a single patron — " +
                "unobtrusively, and before any eclipse condition holds",
        )
        Topic.BACKUP -> Says.Owed(
            "PRD-07",
            "what losing the device costs, on the screen that makes the backup",
        )
        Topic.RESTORE -> Says.Owed(
            "PRD-09",
            "that the restored history is complete only as far as its holder claims, and " +
                "was not verified complete",
        )
        Topic.BROKERED -> Says.Owed(
            "PRD-08",
            "that the vendor's session outlives membership — shown once, before any " +
                "traffic is routed",
        )
        Topic.PROVISIONING -> Says.Owed(
            "PRD-11",
            "that the ordering of providers and zones speaks to this operator's own " +
                "horizon and not to the network's",
        )
        Topic.OPERATOR_EXPOSURE -> Says.Owed(
            "PRD-06",
            "what the operator's configuration exposes the identities below them to",
        )
    }

    /** Every topic, for a test that must cover all of them. */
    fun topics(): List<Topic> = Topic.entries.toList()

    /** The topics whose words nobody has written yet, with what they owe. */
    fun owed(): List<Says.Owed> = topics().mapNotNull { of(it) as? Says.Owed }
}
