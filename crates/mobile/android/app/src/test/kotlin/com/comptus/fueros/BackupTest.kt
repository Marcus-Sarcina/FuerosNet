package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **PRD-10's prompt and PRD-07's words**, now that the shell has a door to
 * a backup at all ([BackupActivity]).
 *
 * The lifecycle is the whole of what PRD-10 asks, and it is a function of
 * one thing, so these are tests of a function and not of a screen.
 */
class BackupTest {

    // acceptance: PRD-10
    @Test
    fun the_prompt_runs_until_a_backup_exists_and_returns_if_it_stops_existing() {
        // **until**: nothing written, so ask
        assertTrue("a device with no backup is asked", Backup.prompting(null, present = false))
        // **absent while it exists**
        assertFalse(
            "a device holding one is not nagged",
            Backup.prompting("content://docs/1", present = true),
        )
        // **and back after deletion**, which is the case a flag could not
        // reach: a boolean saying *backed up* never comes back down, so
        // what is kept is where it went and the file is asked
        assertTrue(
            "a backup that is gone is a backup to make again",
            Backup.prompting("content://docs/1", present = false),
        )
    }

    // acceptance: PRD-10
    @Test
    fun nothing_counts_and_nothing_remembers_having_asked() {
        // **the same inputs give the same answer, always**: there is no
        // counter to advance and no memory to consult, which is how
        // PRD-10's "no count or memory" is met rather than promised
        repeat(50) {
            assertTrue(Backup.prompting(null, present = false))
            assertFalse(Backup.prompting("content://docs/1", present = true))
        }
        // and the decision takes nothing else: a URI with no file behind
        // it is the same as no URI, however many times it is asked
        assertEquals(
            Backup.prompting(null, present = false),
            Backup.prompting("content://docs/gone", present = false),
        )
    }

    /**
     * **PRD-07 is deliberately not claimed here**, though this is its
     * middle clause — and the marker is spelled nowhere in this comment,
     * because the catalogue finds markers by pattern and a sentence saying
     * a row is *not* marked would mark it. The row has a reviewer *import* a backup containing captures
     * past their window, and the shell has no door to do it: `restore_backup`
     * is refused once a client holds an archive, so importing belongs on
     * first run — the third door [FirstRunActivity] says it owes. The
     * encryption and the import scan are both proven in
     * `client/tests/backup.rs`; what is missing is a person's way in, and
     * a marker here would close a row on two clauses out of three.
     */
    @Test
    fun the_screen_says_what_losing_the_device_costs() {
        val said = Backup.words().joinToString(" ")
        assertTrue("the words come from the table", said.isNotEmpty())
        // the author's sentence, by its substance rather than its letter:
        // what is lost, and what each of the three routes recovers
        assertTrue("says what losing a device costs: $said", said.contains("losing a device"))
        assertTrue("names the transaction history: $said", said.contains("transaction history"))
        assertTrue("names the seed phrase as the identity's route: $said", said.contains("seed phrase"))
        assertTrue("and that only a backup covers the rest: $said", said.contains("only a backup"))
    }

    @Test
    fun what_is_said_once_written_keeps_the_two_halves_apart() {
        val said = Backup.written("content://docs/1")
        assertTrue("says where it went: $said", said.contains("content://docs/1"))
        // **design §13.7.1's point, said to the person**: the blob is
        // under a key derived from the identity, so a backup kept beside a
        // written-down phrase is one file holding both halves
        assertTrue("says the phrase opens it: $said", said.contains("seed phrase"))
        assertTrue("and to keep them apart: $said", said.contains("apart"))
    }

    // acceptance: PRD-07
    @Test
    fun an_import_says_what_came_back_and_what_the_scan_deleted() {
        // **nothing past its window**: said plainly, so a person is not
        // left wondering whether something went quietly
        val clean = Backup.imported(records = 42uL, discarded = 0uL)
        assertTrue("says how much came back: $clean", clean.contains("42 records came back"))
        assertTrue("and that nothing was dropped: $clean", clean.contains("Nothing in it was past"))

        // **something past its window**: the count, whose window it was,
        // and that it was deleted rather than kept (design §13.7.1)
        val scanned = Backup.imported(records = 7uL, discarded = 3uL)
        assertTrue("says how much came back: $scanned", scanned.contains("7 records came back"))
        assertTrue("says how many went: $scanned", scanned.contains("3 sealed captures"))
        assertTrue(
            "says whose window decided it: $scanned",
            scanned.contains("window its subject declared"),
        )
        assertTrue("and that they were deleted: $scanned", scanned.contains("deleted on import"))

        // the singular reads as English, since one is the common case
        val one = Backup.imported(records = 1uL, discarded = 1uL)
        assertTrue("$one", one.contains("1 record came back"))
        assertTrue("$one", one.contains("1 sealed capture was"))
    }

    // acceptance: PRD-09
    @Test
    fun a_restored_archive_is_presented_as_recovered_on_trust() {
        val said = Disclosure.words(Disclosure.Topic.RESTORE).joinToString(" ")
        assertTrue("the words come from the table", said.isNotEmpty())
        // the author's sentence, by its substance: complete only for what
        // it covers, and not a claim that nothing else existed
        assertTrue("says it is complete only for a period: $said", said.contains("complete only"))
        assertTrue(
            "and does not claim there was nothing more: $said",
            said.contains("does not preclude"),
        )
        // **and never as verified**, which is what PRD-09 forbids: the
        // history is what its holder claims, and this device checked none
        // of it
        assertTrue("it calls the history verified: $said", !said.contains("verified"))
    }

    @Test
    fun the_file_is_named_for_the_day_it_was_written() {
        // a person with several backups should be able to tell which is
        // which without opening any of them
        assertEquals(
            "fueros-backup-2026-11-05.rhtn",
            Backup.fileName(
                java.text.SimpleDateFormat("yyyy-MM-dd", java.util.Locale.US)
                    .parse("2026-11-05")!!
                    .time,
            ),
        )
    }
}
