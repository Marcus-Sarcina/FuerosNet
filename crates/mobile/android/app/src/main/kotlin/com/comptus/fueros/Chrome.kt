package com.comptus.fueros

/**
 * **Where you are in the shell, and what the shell shows around you** —
 * the pure part, so a test can hold it (`Robot/light-client-screens.md`,
 * *the shell*).
 *
 * The app has been one Activity per screen with no chrome of its own:
 * thirteen screens, each drawing its own title or none, the only way back
 * the system gesture. The design is one shell instead — **a rail down the
 * left and a bar along the foot that persist**, with the content between
 * them changing, and a few places that take the whole screen and hide
 * both.
 *
 * **This is the part that decides, and it touches no view.** Which places
 * keep the chrome, what the rail holds and in what order, what the foot
 * says, and what *back* means from where you are. [ShellActivity] draws
 * what this returns.
 */
object Chrome {

    /** Where the shell can be. */
    sealed class Place {
        /** The landing state: every subnet's notices, newest first. */
        object Home : Place()

        /** One subnet: its resources and its board. */
        data class Subnet(val id: String) : Place()

        /** Direct messages, which is also the contacts list. */
        object Direct : Place()

        /** Meeting somebody, which owns the screen and the camera. */
        object Meet : Place()

        /** This person's own options, reached from the foot. */
        object Options : Place()

        /** One subnet's options, reached from its name. */
        data class SubnetOptions(val id: String) : Place()

        /** One thread, direct or on a board. */
        data class Thread(val id: String, val inSubnet: String?) : Place()

        /** A resource's own page, in its frame. */
        data class Resource(val node: String, val resource: String) : Place()
    }

    /**
     * **Whether the rail and the foot are drawn.**
     *
     * A thread hides them [author, 2026-10-27] — *selecting a DM thread
     * or a discussion thread will hide the side and bottom navigation*
     * — and a resource takes the screen as a thread does. Options hides
     * them for a reason of its own: it is reached *from* the foot, and
     * the foot is what it would otherwise cover. A meeting owns the
     * screen because the phone is turned round to face somebody.
     */
    fun chrome(place: Place): Boolean = when (place) {
        is Place.Home, is Place.Subnet, is Place.Direct -> true
        is Place.Meet, is Place.Options, is Place.SubnetOptions -> false
        is Place.Thread, is Place.Resource -> false
    }

    /**
     * **What a place with no chrome puts back in its title bar**: a back
     * control, because the way out went with the rail.
     */
    fun back(place: Place): Boolean = !chrome(place)

    /** One thing the rail can hold. */
    sealed class Rung {
        /** Direct messages, pinned at the top. */
        data class Direct(val unseen: Int) : Rung()

        /** Meeting somebody, pinned under it. */
        object Meet : Rung()

        /** The line between what is pinned and what scrolls. */
        object Divider : Rung()

        /** One joined subnet. */
        data class Subnet(
            val id: String,
            /** Its name where its root published one, else null and the
             *  keyhash stands in. */
            val alias: String?,
            val unseen: Int,
            val crown: Crown,
            /** The identity that holds this subnet, whose colour the orb takes. */
            val by: String = "",
        ) : Rung()
    }

    /**
     * **What this person is in a subnet, drawn as the crown the orb sits
     * in** [author, 2026-10-27]: a thin silver circlet where they have at
     * least one subordinate, a golden circlet where they run a node, a
     * full pointed crown where they are the root.
     *
     * **Only the highest that applies is drawn**, since a root usually
     * has subordinates too: root before node before subordinates.
     */
    enum class Crown { NONE, SILVER, GOLD, FULL }

    /**
     * What a subnet's standing comes to, given the three facts about it.
     * **One place, because three booleans read in two places would
     * eventually disagree about which wins.**
     */
    fun crown(root: Boolean, operator: Boolean, hasSubordinate: Boolean): Crown = when {
        root -> Crown.FULL
        operator -> Crown.GOLD
        hasSubordinate -> Crown.SILVER
        else -> Crown.NONE
    }

    /**
     * The rail, in order: direct messages and meeting pinned, a divider,
     * then the subnets as the kernel gave them.
     */
    fun rail(unseenDirect: Int, subnets: List<Rung.Subnet>): List<Rung> =
        buildList {
            add(Rung.Direct(unseenDirect))
            add(Rung.Meet)
            if (subnets.isNotEmpty()) add(Rung.Divider)
            addAll(subnets)
        }

    /**
     * **What an orb says when it has no picture**: two letters of the
     * name, or four hex characters of the keyhash where no name has
     * reached this device — never a name that was not received
     * (`light-client-requirements.md` §4.3).
     */
    fun mark(alias: String?, id: String): String {
        val name = alias?.trim().orEmpty()
        if (name.isEmpty()) return id.take(4)
        val words = name.split(' ').filter { it.isNotBlank() }
        return if (words.size >= 2) {
            "${words[0].first().uppercaseChar()}${words[1].first().uppercaseChar()}"
        } else {
            name.take(2).replaceFirstChar { it.uppercaseChar() }
        }
    }

    /** What the rail's own accessibility label says about one rung. */
    fun said(rung: Rung): String = when (rung) {
        is Rung.Direct -> "Direct messages" + count(rung.unseen)
        is Rung.Meet -> "Meet somebody"
        is Rung.Divider -> ""
        is Rung.Subnet -> {
            val who = rung.alias ?: "subnet ${rung.id.take(8)}"
            val standing = when (rung.crown) {
                Crown.FULL -> ", you are its root"
                Crown.GOLD -> ", you run a node here"
                Crown.SILVER -> ", you have a subordinate here"
                Crown.NONE -> ""
            }
            who + standing + count(rung.unseen)
        }
    }

    private fun count(n: Int): String = when {
        n <= 0 -> ""
        n == 1 -> ", 1 unseen"
        else -> ", $n unseen"
    }

    /**
     * **An unseen count as the orb draws it.** Nothing above a threshold
     * is a number anybody reads, and a four-digit badge would be wider
     * than the orb it sits on.
     */
    /**
     * **Whether to suggest a second patron** (PRD-04,
     * `light-client-requirements.md` §6): a person standing under exactly
     * one patron reaches the network through one person and their
     * infrastructure, and a second adoption is what changes that.
     *
     * **Not timed against an eclipse** [author, 2026-11-05]: *"It is a
     * theoretical attack, not an intended feature of the network."* So
     * this asks only how many patrons are held — suggested at one, absent
     * at none, because somebody with no patron has nothing to add a second
     * to, and absent at two or more, because it is then done.
     */
    fun suggestingSecondPatron(patrons: Int): Boolean = patrons == 1

    /** **One launcher on a subnet's page** [author, 2026-11-23]: a resource
     *  its serving node offered, which the card opens, what it asks that
     *  waits, which the card wears, and the icon it serves, which badges it
     *  — a PNG, JPEG or WebP the kernel took, or null and its mark stands in. */
    data class Launcher(
        val node: String,
        val resource: String,
        val name: String,
        val unseen: Int,
        val icon: ByteArray? = null,
    )

    /**
     * **What a subnet launches**: the resources swept from the node serving
     * its holder there, one card each, named as the node named them. The
     * person never browses for them; connecting sweeps, and so does
     * opening the subnet [author, 2026-11-23].
     */
    fun launchers(offered: List<uniffi.rhtn_model.Resource>, node: String): List<Launcher> =
        offered
            .filter { it.node.hex() == node }
            .map { Launcher(it.node.hex(), it.resource.hex(), it.instance, it.waiting.toInt(), it.icon) }
            .sortedBy { it.name.lowercase() }

    /**
     * **A subnet's orb count, rolled up** [author, 2026-11-23]: a move of its
     * node to answer, what waits for the person at each of its resources,
     * and what is unread on its board, which is this client's own count.
     */
    fun unseen(moved: Boolean, launchers: List<Launcher>, board: Int = 0): Int =
        (if (moved) 1 else 0) + launchers.sumOf { it.unseen } + board

    /** What names one identity's board in one subnet. */
    fun boardKey(by: String, subnet: String): String = "$by:$subnet"

    /** How many posts of a board the person has not been shown. */
    fun boardUnread(threads: List<uniffi.rhtn_model.BoardThread>): Int =
        threads.sumOf { it.unread.toInt() }

    /** **A reach as a post says it** [author, 2026-10-27]: `+2/-2`. */
    fun reach(up: Int, down: Int): String = "+$up/-$down"

    /** The reach a new thread is offered before the writer changes it [author, 2026-10-27]. */
    const val DEFAULT_REACH = 2

    /** The furthest a reach goes either way: the path bound (`wire-format.md` §10.3). */
    const val MAX_REACH = 24

    /** A reach moved a level, kept within what a post may carry. */
    fun step(level: Int, by: Int): Int = (level + by).coerceIn(0, MAX_REACH)

    /** How long ago `then` was, in seconds, said shortly. */
    fun ago(now: Long, then: Long): String {
        val s = (now - then).coerceAtLeast(0)
        return when {
            s < 90 -> "just now"
            s < 5400 -> "${(s + 59) / 60} min ago"
            s < 129_600 -> "${s / 3600} h ago"
            else -> "${s / 86_400} d ago"
        }
    }

    /** A thread's row on its subnet's page: its title, and what stands under it. */
    fun threadRow(t: uniffi.rhtn_model.BoardThread, beganBy: String?, now: Long): Pair<String, String> {
        val title = (t.title ?: "A thread begun out of your reach") +
            if (t.unread > 0u) " (${t.unread} unread)" else ""
        val posts = if (t.posts == 1u) "1 post" else "${t.posts} posts"
        val by = beganBy?.let { "began by $it, " } ?: ""
        return title to "$by$posts, ${ago(now, t.latest.toLong())}"
    }

    /** Who a post is by: the name held for them, else the one the post carried, else their key. */
    fun writer(held: String?, carried: String?, key: String): String =
        held ?: carried ?: "${key.take(8)}…"

    fun badge(n: Int): String? = when {
        n <= 0 -> null
        n > 99 -> "99+"
        else -> n.toString()
    }

    /** The foot: this person's own name, and what the bar says it does. */
    /**
     * **A person's colour** [author, 2026-11-23: *"in a characteristic
     * color"*]: the palette's entry chosen for them, else the one their
     * key gives them — its first byte, over the palette — so a party keeps
     * one colour without anybody choosing it.
     */
    fun personColour(chosen: String?, key: String): Pair<String, Int> =
        Palette.IDENTITY.firstOrNull { it.first == chosen }
            ?: Palette.IDENTITY[(key.take(2).toIntOrNull(16) ?: 0) % Palette.IDENTITY.size]

    /** **How much of a person's colour their bubbles wear** over the
     *  ground: a low-saturation version of it, under which ink still reads. */
    const val BUBBLE_SHARE = 0.28

    /**
     * **Where a person stands from here, as their card says it** [author,
     * 2026-11-23]: levels above with a plus, below with a minus, across as
     * nought; none where it cannot be stated.
     */
    fun standing(offset: Int?): String? = when {
        offset == null -> null
        offset > 0 -> "+$offset"
        else -> "$offset"
    }

    fun foot(alias: String?, me: String): Pair<String, String> =
        (alias?.takeIf { it.isNotBlank() } ?: me.take(12) + "…") to "home"
}
