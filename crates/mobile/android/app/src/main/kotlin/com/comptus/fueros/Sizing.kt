package com.comptus.fueros

/**
 * **What a node needs, and which of a provider's sizes meet it** — the
 * pure part, so a test can hold it, as [Chrome] and [Witnessing] are.
 *
 * ## Where the figures come from, and what they are not
 *
 * **One measured floor, and everything above it reasoned** [2026-11-05].
 * The bench node at `142.93.86.90`, eighteen hours up, serving two members
 * on a 512 MB single-core machine: `rhtnd` held **12.7 MB** resident, the
 * machine including the operating system and container engine used
 * **121 MB**, and state on disk was 6.1 MB.
 *
 * **That node is idle in every way that would cost it** [author,
 * 2026-11-05]: no message traffic in its horizon or out to the network, no
 * resources hosted — its hosting file says *"none yet"* — and no resource
 * call ever served. So 12.7 MB is a **floor**: a process, two sessions and
 * its own loops. It is not a basis for extrapolating a loaded node, and
 * the 0.01% of one core it spends says nothing about load either.
 *
 * **The queue's disk cost is block-dominated, which the bytes hide.** Its
 * 770 files — residue from the bench ceremonies — are 3.1 MB of content in
 * 5.2 MB of blocks. A per-subordinate cap counted in bytes therefore
 * under-states the disk a queue of small messages occupies, by roughly the
 * ratio above at this file size.
 *
 * **The bundles are estimated from comparable Rust services and not from
 * anything here, because they do not exist yet.** The figure below is the
 * weakest input and the largest term; treat it as the thing the first
 * loaded node will correct.
 *
 * **The assumptions are the author's** [author, 2026-11-05]: the five
 * application bundles implemented in Rust at this project's depth, a
 * typical node hosting three of them, 80% of maximum downline, and
 * messaging at the frequency of an active Slack or Discord user.
 *
 * **The downline is bounded by the protocol, which decides the scale**:
 * design §3.2 gives every node at most **f = 10** subordinates — *"ten
 * values, ten subordinates, one occupant each"* — so a node serving 80% of
 * its maximum serves **eight members**, not thousands. That is the firmest
 * input here, and it is why a small machine is the right answer.
 *
 * ## The arithmetic, with its confidence marked
 *
 * | | | |
 * |---|---|---|
 * | `rhtnd`, idle, two members | 12.7 MB | **measured** |
 * | `rhtnd` at eight members under traffic | ~25 MB | estimated: sessions and pools per member, from an idle floor |
 * | three hosted bundles | 90–150 MB | **guessed**: 30–50 MB each, from comparable Rust services, the bundles being unbuilt |
 * | the operating system and engine | ~100 MB | **measured** |
 * | working set | 215–275 MB | the sum, dominated by the guess |
 *
 * So **1 GB is the floor** and **2 GB the recommendation**, the difference
 * buying room for a bundle compacting or a backup being enveloped without
 * touching swap. **One core is enough**: the node relays ciphertext it
 * cannot read, so per-message cryptography is the clients' work, and what
 * it spends is TLS, QUIC and the bundles' request handling.
 *
 * **Disk is the queue, capped per subordinate** (`queue_cap`, design
 * §14.1.6). Eight members at a 256 MB cap each is 2 GB of content and
 * more of blocks; with three bundles' storage blocks and the operating
 * system, **25 GB is comfortable** and 50 GB is right where an operator
 * sets generous caps.
 *
 * **Stated as capability and not as a provider's slug**, so it holds for
 * whichever provider the operator picks: what is recommended is memory,
 * and a size meets it or does not.
 */
object Sizing {

    /** Below this a node has no room for three bundles and its own work. */
    const val MINIMUM_MB = 1024L

    /**
     * The top of the recommended band. **Both the 1 GB and the 2 GB
     * machines are recommended and both are marked** [author,
     * 2026-11-05]: the working set fits either, and the difference is
     * headroom rather than reach — so an operator choosing between them is
     * choosing how much slack to buy, not whether the node will work.
     */
    const val RECOMMENDED_MAX_MB = 2048L

    /** How a size is described where one is named to a person. */
    fun described(memoryMb: Long, vcpus: Int, monthly: Double): String =
        "${memoryMb / 1024.0} GB, $vcpus core${if (vcpus == 1) "" else "s"}, " +
            "about ${"%.0f".format(monthly)} a month"

    /**
     * **Whether a size is one this project would put a node on.** Memory
     * is the binding constraint; a provider offering more cores at less
     * memory is not a better answer.
     */
    fun adequate(memoryMb: Long): Boolean = memoryMb >= MINIMUM_MB

    /**
     * **Whether a size is in the recommended band**, which is what the
     * picker marks: at or above the floor and no larger than needed.
     *
     * A band rather than a threshold, so **both** the 1 GB and the 2 GB
     * machines are marked, and a 4 GB one is left unmarked — adequate, and
     * more than this project can justify recommending. Expressed as a
     * range rather than two exact values because a provider may call its
     * gigabyte 1000 MB.
     */
    fun recommended(memoryMb: Long): Boolean =
        memoryMb >= MINIMUM_MB && memoryMb <= RECOMMENDED_MAX_MB

    /**
     * **Every size to mark**, which is the band and not one answer: an
     * operator picking between 1 GB and 2 GB is choosing headroom, and the
     * screen should not pretend one of them is the only right machine.
     * Empty where the provider offers neither, which is a fact about the
     * provider rather than a reason to mark something else.
     */
    fun <T> picked(sizes: List<T>, memoryOf: (T) -> Long): List<T> =
        sizes.filter { recommended(memoryOf(it)) }

    /**
     * **A note for the person**, saying what the mark means and what it
     * rests on. Short, because the launch screen is already long.
     */
    fun note(): String =
        "A node serves at most ten members and this one is sized for eight, " +
            "hosting three application bundles. ${RECOMMENDED_MAX_MB / 1024} GB is " +
            "recommended, and so is ${MINIMUM_MB / 1024} GB; below that there is no room for the " +
            "bundles and the node's own work. More memory buys headroom, not reach."
    // ---- which sizes a region offers, and which regions a size does ----
    //
    // `Robot/future-features.md` entry 2 [author, 2026-11-02]: *"the
    // 'fetch regions and sizes' control populates every size and every
    // region, but many combinations result in a rejected request for that
    // size not being available on that region."*  Both adapters already
    // parse which regions each size is offered in and it already reaches
    // the shell, where it was held and not used — so the whole of the fix
    // is in the drawing.

    /** The sizes a region offers, in the order given. */
    fun <S> sizesIn(region: String, sizes: List<S>, regionsOf: (S) -> List<String>): List<S> =
        sizes.filter { regionsOf(it).isEmpty() || region in regionsOf(it) }

    /**
     * The regions a size is offered in, in the order given.
     *
     * **A size listing no regions constrains nothing** rather than
     * everything: an adapter that could not parse the list must not make
     * every region look wrong.
     */
    fun <R> regionsFor(
        sizeRegions: List<String>,
        regions: List<R>,
        slugOf: (R) -> String,
    ): List<R> = if (sizeRegions.isEmpty()) regions else regions.filter { slugOf(it) in sizeRegions }

    /**
     * **Regions where the recommendation can actually be met, first; then
     * the least crowded of those** [author, 2026-11-05, 2026-11-06].
     *
     * A region that cannot offer a size this project would put a node on
     * is still shown — it may suit an operator who knows better — but it
     * is shown after the ones that can, and [whyBehind] says what the
     * ordering means. Ties keep the provider's own order, which is the
     * same rule the concentration ordering keeps.
     *
     * **Two keys, and capability is the first of them** [2026-11-06]. The
     * second is how many parties in this operator's own horizon already
     * run in that zone of this provider (PRD-11;
     * `light-client-requirements.md` §6) — the author's reason being that
     * *"if we disperse by ASN + region, then we are remediating war,
     * disaster, power grid, physical network backbone, and local
     * government risk as well"*. Capability comes first because a zone
     * that cannot host a node is not a dispersal: the question *should it
     * run here* only arises where it can.
     *
     * [crowding] is zero by default, which is the ordering as it was
     * before the pair was carried — and what a client holding no peering
     * in any zone of this provider genuinely knows.
     */
    fun <R, S> regionsByRecommended(
        regions: List<R>,
        sizes: List<S>,
        slugOf: (R) -> String,
        memoryOf: (S) -> Long,
        regionsOf: (S) -> List<String>,
        crowding: (R) -> Int = { 0 },
    ): List<R> = regions.withIndex().sortedWith(
        compareBy(
            { (_, r) -> if (meetsRecommended(slugOf(r), sizes, memoryOf, regionsOf)) 0 else 1 },
            { (_, r) -> crowding(r) },
            { (i, _) -> i },
        ),
    ).map { it.value }

    /**
     * **Whether a region can host a node at all**: the author's *"minimum
     * recommended instance being available"*, which is the floor rather
     * than the band's top — a region offering 1 GB and not 2 GB is a
     * region a node runs in.
     *
     * **A size with no region list does not count here, though it counts
     * in [sizesIn]**, and the asymmetry is deliberate [2026-11-05]. Hiding
     * a size is destructive, so an unparsed list must hide nothing;
     * ordering is advisory, so an unparsed list must not vouch for a
     * region either. Letting it vouch would mean one size an adapter could
     * not read made every region look able — which is how this ordering
     * would quietly stop meaning anything. The test that found this had
     * asserted the opposite.
     */
    fun <S> meetsRecommended(
        region: String,
        sizes: List<S>,
        memoryOf: (S) -> Long,
        regionsOf: (S) -> List<String>,
    ): Boolean = sizes.any { adequate(memoryOf(it)) && region in regionsOf(it) }

    /**
     * **Why a region sits at the bottom**, said rather than left to be
     * inferred: `light-client-requirements.md` §9 has a refusal render its
     * reason, and a reordering nobody explains is a refusal in disguise.
     */
    fun whyBehind(): String =
        "Regions that cannot offer a recommended size are listed last. They are " +
            "not refused — a smaller machine may suit you — but a node with three " +
            "bundles on it will be tight. Among the rest, the regions where fewest " +
            "of the people in your horizon already run come first, and any that are " +
            "crowded say how many."

    /**
     * **Why a combination is absent**, which is a different fact from a
     * size not existing (entry 2): an operator comparing prices will want
     * to know which they are looking at.
     */
    fun whyAbsent(size: String, region: String): String =
        "$size is not offered in $region. The provider's own list says so; a " +
            "creation there would be refused. Pick another region, or another size."

}
