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
    fun badge(n: Int): String? = when {
        n <= 0 -> null
        n > 99 -> "99+"
        else -> n.toString()
    }

    /** The foot: this person's own name, and what the bar says it does. */
    fun foot(alias: String?, me: String): Pair<String, String> =
        (alias?.takeIf { it.isNotBlank() } ?: me.take(12) + "…") to "home"
}
