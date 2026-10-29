package com.comptus.fueros

/**
 * **What the shell's content says where it says a number** — the pure
 * part, so a test can hold it. [Chrome] decides the chrome and the rail;
 * this is the wording of the places between them.
 */
object Shell {

    /**
     * **A patron's downline in one subnet: the direct lines they issued
     * there and no deeper figure** [author, 2026-10-28]. A patron signed
     * every adoption it issued, so the count is exact, where a subtree
     * count would be only as far as the propagated copy reaches. Null is a
     * standing not read yet, which is said rather than shown as none.
     */
    fun downline(s: Kernel.Standing?): String = when {
        s == null -> "Your standing here has not been read yet."
        s.subordinates == 0 -> "No line of yours is issued here."
        s.subordinates == 1 -> "Your downline here: 1 direct line."
        else -> "Your downline here: ${s.subordinates} direct lines."
    }
}
