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
 * **The sentences here are the author's own** [author, 2026-11-05],
 * written to this table's obligations and kept as written — a doubled word
 * and a singular "message" were the only edits. They name the parties as
 * `{patron}` and `{subnet}`, which [fill] replaces, because what is fixed
 * by kind is the sentence and not the people in it.
 *
 * **Nothing is owed as of 2026-11-05.** A brokered resource's retained
 * vendor session was a topic here until the author withdrew the
 * disclosure: ending a session protects the resource from the person, not
 * the person from anything, so a member who keeps access they were not cut
 * off from has nothing to act on and telling them would announce an
 * opportunity rather than correct a mistake (PRD-08, withdrawn).
 *
 * [Says.Owed] stays in the type for the next gap, and nothing here invents
 * a sentence to cover one: an owed topic would show the person nothing,
 * and the test would say so rather than a screen saying something nobody
 * wrote.
 */
object Disclosure {

    /**
     * **What a disclosure is about.** The transaction kinds, and the other
     * moments at which this client owes somebody words — a backup, a
     * restore, a provisioning choice. The acceptance
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

        /**
         * **An identity path that does not link the new credential to
         * prior history** — `light-client-requirements.md` §6's own words,
         * and the reason this is not called a rotation [author,
         * 2026-11-05]. The subnet adopts a new key of yours as design
         * §9.0's *"genuinely-new user"*, vouched for afresh, so the
         * inheritance a `Recovery` would have carried does not come with
         * it. **Prior standing is abandoned in that sense alone**: all
         * three of reissue, rotation and restore keep the local status,
         * permissions and state a member holds.
         */
        UNLINKED_PATH,

        /**
         * **An identity path that does publish the link** (§6), which is
         * what the author calls a **key rotation**: the new key is adopted
         * as the member the old one was, the old key signs the `Recovery`
         * that carries the inheritance, and what is given up is
         * unlinkability rather than standing.
         *
         * **Not restoring a backup** [author, 2026-11-05], which is
         * [RESTORE]: that reloads data to the *same* key and changes who
         * you are nowhere. These two were one word apart and two
         * operations apart, which is why both say so here.
         */
        LINKED_PATH,

        /** Holding one patron only, and what a second would protect. */
        SECOND_PATRON,

        /** Making a backup, and what losing the device costs. */
        BACKUP,

        /**
         * **Data reloaded to the same key** from a backup — the same
         * identity, carrying on [author, 2026-11-05]. Distinct from
         * [RECOVERY], which replaces the key a subnet knows; nothing about
         * a restore changes who you are anywhere.
         */
        RESTORE,

        /** Provisioning: what the ordering of providers and zones means. */
        PROVISIONING,

        /** What an operator's configuration exposes those below them to. */
        OPERATOR_EXPOSURE,

        /** A record nobody witnessed, said as a fact and not a fault. */
        WITNESSLESS,
    }

    /**
     * One obligation and the words that meet it, as `Meet.Item` is.
     *
     * [text] may name the parties as `{patron}` and `{subnet}`, which
     * [fill] replaces. **What goes in is a display name already decided**
     * — an alias the party published, or its keyhash where none was
     * (PRD-14: never a name this device was not given).
     */
    data class Item(val row: String, val text: String)

    /**
     * The words with the parties named. A hole nobody filled is left as
     * it is rather than blanked: a sentence with a visible `{patron}` in
     * it is a bug somebody reports, where one with a gap reads as
     * finished.
     */
    fun fill(text: String, patron: String? = null, subnet: String? = null): String {
        var out = text
        patron?.let { out = out.replace("{patron}", it) }
        subnet?.let { out = out.replace("{subnet}", it) }
        return out
    }

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

        // ---- the words themselves, [author, 2026-11-05] ---------------

        Topic.ADOPTION_UPWARD -> Says.Here(
            listOf(
                Item(
                    "PRD-02",
                    "You are accepting {patron} as your patron in the {subnet} subnet. " +
                    "{patron} will be the route for messages between you and {subnet}. " +
                    "{patron}'s policies and those of their patron will apply to any " +
                    "resources you use or host in {subnet} until such a time as you depart " +
                    "from their patronage.",
                ),
            ),
        )
        Topic.UNLINKED_PATH -> Says.Here(
            listOf(
                Item(
                    "PRD-03",
                    "You are changing the core identifier connecting you to {subnet}. Your " +
                    "previous identifier will still reach you in any other subnets you " +
                    "participate in. On {subnet} it loses its reachability, and any standing " +
                    "which depends on it goes with it. Your new identifier will carry only " +
                    "such privileges and relationships as you have established directly " +
                    "through {patron}, which are granted only within the trust horizon on " +
                    "{subnet}.",
                ),
            ),
        )
        Topic.LINKED_PATH -> Says.Here(
            listOf(
                Item(
                    "PRD-03",
                    "You are changing the core identifier connecting you to {subnet} and " +
                    "publishing the fact of your new identity. Your recovered identity will " +
                    "be traceable to your previous identity. Your previous identifier will " +
                    "still be active in any other subnets you participate in. On {subnet} it " +
                    "loses its reachability, and what it held there is carried to the new " +
                    "identifier rather than lost. Your new " +
                    "identifier will carry only such privileges and relationships as you " +
                    "have established directly through {patron}.",
                ),
            ),
        )
        Topic.SECOND_PATRON -> Says.Here(
            listOf(
                Item(
                    "PRD-04",
                    "You are currently connected to FuerosNet through a single patron. This " +
                    "means that your reachability and view of the network flow through a " +
                    "single person and his infrastructure. Consider a second adoption to " +
                    "strengthen your connection to the network as a whole.",
                ),
            ),
        )
        Topic.BACKUP -> Says.Here(
            listOf(
                Item(
                    "PRD-07",
                    "You should consider making a backup of the data stored on your device. " +
                    "Currently, losing a device means losing all of your transaction " +
                    "history. Your identity can be recovered from the seed phrase and your " +
                    "contacts can be recovered from a peer, but only a backup can protect " +
                    "any files and transaction records you have accumulated.",
                ),
            ),
        )
        Topic.RESTORE -> Says.Here(
            listOf(
                Item(
                    "PRD-09",
                    "The archive you are restoring is complete only for the period of time " +
                    "covered and does not preclude the existence of additional transactions " +
                    "not recorded here.",
                ),
            ),
        )
        Topic.PROVISIONING -> Says.Here(
            listOf(
                Item(
                    "PRD-11",
                    "Cloud providers are recommended to encourage a diversity of hosts " +
                    "within your local subnet. The ordering below does not reflect the " +
                    "distribution of all FuerosNet nodes in the world. You can also host " +
                    "your own node physically or use a cloud provider not listed by using " +
                    "the manual enrollment process.",
                ),
            ),
        )
        // **the node already says this, and the client frames the page
        // that says it** [2026-11-05]: `rhtn_daemon::operator` renders
        // "what this node's configuration exposes the identities below it
        // to", with how many sit below, whether their payload is relayed
        // here and what is hosted here, and serves it on the operator's
        // own page. Asked what configuration PRD-06 meant, the answer was
        // that it is written already.
        Topic.OPERATOR_EXPOSURE -> Says.Elsewhere(
            "PRD-06",
            "rhtn_daemon::operator: \"what this node's configuration exposes the " +
                "identities below it to\", on the page the client frames",
        )
    }

    /**
     * **The words a screen renders for a topic**, with the parties named:
     * what this table holds, and nothing where the words live elsewhere or
     * are owed.
     *
     * A screen calls this rather than casting [of]'s result, so a topic
     * that moves between [Says.Here] and [Says.Elsewhere] changes no
     * screen and throws nowhere. Empty means *this table has nothing to
     * render* — not that nothing is owed, which is [owed]'s question.
     */
    fun words(topic: Topic, patron: String? = null, subnet: String? = null): List<String> =
        when (val s = of(topic)) {
            is Says.Here -> s.items.map { fill(it.text, patron, subnet) }
            is Says.Elsewhere -> emptyList()
            is Says.Owed -> emptyList()
        }

    /** Every topic, for a test that must cover all of them. */
    fun topics(): List<Topic> = Topic.entries.toList()

    /** The topics whose words nobody has written yet, with what they owe. */
    fun owed(): List<Says.Owed> = topics().mapNotNull { of(it) as? Says.Owed }
}
