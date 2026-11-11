package com.comptus.fueros

/**
 * **Taking an identity off this device** — the pure part, so a test can
 * hold it, as [Backup] and [Rotation] are.
 *
 * **The most destructive act in the app, and it is a door of its own**
 * [author, 2026-11-07]. Asked whether this and starting a new identity
 * should be one function, the answer was no: *"'Launch a new identity'
 * and 'Wipe the current identity' are necessary functions"* and they are
 * two. Fusing them would make the only way to start fresh a destruction
 * and the only way to destroy a fresh start, and a person confirming one
 * button could not say which of the two they had agreed to. So this
 * clears, returns the device to [FirstRunActivity], and the two doors
 * already there — *start a new identity*, *restore from a seed phrase* —
 * take it from that point.
 *
 * **It is not a rotation, and the two must not be confused** (design
 * §9.0's vocabulary). A key rotation replaces the key a subnet knows you
 * by and keeps you in that subnet — it happens at a meeting, the patron
 * adopts the new key, and the local status comes across
 * ([RotationActivity]). This keeps nothing and tells nobody: the device
 * is left holding no identity, and whatever the wiped key stood on stands
 * in other people's archives without this device in the picture.
 *
 * **It does not assume one identity per device** [author, 2026-11-07]:
 * *"if we support key rotation, we wind up having to support multiple
 * identities on a device, so 'Add an identity' … is just another trigger
 * for 'Start a new identity'/Key rotation."*  So the words say *this*
 * identity and not *your* identity, and nothing here is named for the
 * singular case.
 *
 * ## What has to go, and why the list is checked rather than remembered
 *
 * Everything the kernel holds goes through the shell's custody into one
 * directory ([KERNEL_DIR]), the sealed names beside the archive and the
 * store. The shell keeps its own state in preference stores of its own
 * ([STORES]), and a field-test build writes diagnostic lines to
 * [DIAG_DIR]. **A half-wiped device is worse than an unwiped one** — it
 * reads as clean and still holds sealed captures — so the list is
 * asserted against the sources by `WipeTest`, which fails when a store
 * is added anywhere in the shell and not added here.
 *
 * **And the at-rest key goes last of all.** Everything [AndroidShell.seal]
 * wrote is wrapped by one Keystore key ([AT_REST_ALIAS]); deleting it
 * makes any byte that survived the file deletion undecryptable, which is
 * the same reasoning `light-client-requirements.md` §6 gives for deleting
 * a window of the archive taking its capture keys with it.
 */
object Wipe {

    /** The directory the kernel's own state and every sealed name live in. */
    const val KERNEL_DIR = "kernel"

    /** Where a field-test build writes its diagnostic lines. */
    const val DIAG_DIR = "diag"

    /**
     * The Keystore alias wrapping everything the shell sealed.
     *
     * **Held here as well as in [AndroidShell]**, and `WipeTest` asserts
     * the two agree: this object must stay loadable without Android, and
     * a wipe that deleted the wrong alias would leave the identity's seeds
     * readable while reporting success.
     */
    const val AT_REST_ALIAS = "fueros.kernel.at-rest"

    /**
     * Every preference store the shell writes, which all go.
     *
     * **Checked against the sources, not remembered**: `WipeTest` reads
     * every `getSharedPreferences` name the shell names anywhere and
     * fails where one is missing from this list.
     */
    val STORES = listOf("backup", "operator", "permissions", Giving.NOTIFY_STORE)

    /**
     * **What it takes and what it cannot give back**, before anything is
     * confirmed.
     *
     * The second paragraph is PRD-03's unlinked half, in the shape that
     * entry asks for: the new key **arrives as a genuinely-new user,
     * vouched for afresh** — not that the person loses a status they
     * hold. The distinction is the whole of it. Nothing here is lost by
     * being wiped; what is lost is the evidence this device held that the
     * standing was earned, and the standing itself lives in other
     * people's archives either way (design §16.2.1).
     */
    fun words(): List<String> = listOf(
        "This takes every identity off this device: their keys, everything " +
            "they have signed or been shown, and every capture sealed to them. " +
            "Nothing is sent anywhere and nobody is told.",
        "It cannot be undone from here. The seed phrase written down when each " +
            "identity was made is the only way back to it, with a backup file for " +
            "the history; without both, this device is the last place it existed.",
        "A new identity started afterwards arrives as a genuinely-new user, " +
            "vouched for afresh. It does not inherit what this one earned, and it " +
            "is not the same person to anyone who met this one.",
    )

    /**
     * **The second step**, which names the act rather than asking again.
     * A destructive control that asks "are you sure?" has told the person
     * nothing they did not already know; this says what the tap does.
     */
    fun confirm(): String =
        "Wipe this device. The phrases and a backup are the only way back."

    /** What it says once it has, on the way to the first-run screen. */
    fun done(): String =
        "This device holds no identity. Start a new one or restore one from a phrase."

    /**
     * **What a wipe that could not finish says** — named rather than
     * swallowed: a device still holding part of an identity must not
     * report a clean one, since the person's next decision rests on it
     * (`light-client-requirements.md` §9: a refusal renders its reason).
     */
    fun left(remaining: List<String>): String? =
        if (remaining.isEmpty()) {
            null
        } else {
            "Some of it would not go: ${remaining.joinToString(", ")}. The rest is " +
                "gone and the identity's key is not usable, but this device is not " +
                "clean — uninstall the app to be sure."
        }
}
