package com.comptus.fueros

/**
 * **Whether to ask for a backup** — the pure part, so a test can hold it,
 * as [Chrome] and [Engagement] are.
 *
 * PRD-10 wants the prompt *"until the backup exists, absent while it
 * exists, and back after deletion, with no count or memory of having
 * prompted"*. So the decision is a function of one thing — whether a
 * backup this device knows of is still there — and **there is nowhere to
 * keep a count even if somebody wanted one**, which is how that clause is
 * met rather than remembered.
 *
 * **Why a place and not a flag.** A flag saying *backed up* could never
 * come back down: the file lives wherever the person put it, and an app
 * holding a boolean would go on believing a backup existed after they
 * deleted it — which is the state PRD-10 names as the one that must
 * prompt again. So what is kept is *where* it went, as a document URI this
 * device holds a lasting permission for, and whether it is still there is
 * asked of the file rather than of a memory.
 */
object Backup {

    /**
     * **Ask, or not.** `where` is the backup this device last wrote, as a
     * URI it kept; `present` is whether that URI still resolves.
     *
     * Nothing else is an argument, and nothing else is consulted: not how
     * many sessions have passed, not whether this was asked before, not
     * how old the backup is. A person who keeps a backup is not nagged and
     * a person who loses one is asked again.
     */
    fun prompting(where: String?, present: Boolean): Boolean = where == null || !present

    /**
     * **What the prompt says**, from [Disclosure] rather than from here:
     * the words are the author's and fixed by the kind of thing being
     * disclosed, and this is the screen that shows them.
     */
    fun words(): List<String> = Disclosure.words(Disclosure.Topic.BACKUP)

    /** The name offered when the document is created. */
    fun fileName(whenMs: Long): String {
        val day = java.text.SimpleDateFormat("yyyy-MM-dd", java.util.Locale.US)
            .format(java.util.Date(whenMs))
        return "fueros-backup-$day.rhtn"
    }

    /**
     * **What a person is told once it is written.** Where it went, and the
     * one thing about it they have to keep separately — design §13.7.1
     * puts the blob under a key derived from the identity, so the phrase
     * that restores the identity is what opens the backup, and a backup
     * kept beside a written-down phrase is one file holding both halves.
     */
    fun written(where: String): String =
        "Backup written to $where. It is encrypted under this identity, so the " +
            "seed phrase is what opens it — keep the two apart, because together " +
            "they are one thing and not two."
}
