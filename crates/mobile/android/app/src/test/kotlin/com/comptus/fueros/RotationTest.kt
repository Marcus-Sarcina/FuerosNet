package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **PRD-03: the two paths warn differently** — the unlinked one that prior
 * standing is abandoned, the linked one that the link will be published
 * within the horizon.
 *
 * The words are the author's and live in [Disclosure]; what is tested here
 * is that each path carries its own, that they are not each other's, and
 * that the parties they name are named honestly.
 *
 * **The row is deliberately not claimed by these tests**, and the marker
 * is spelled nowhere below — the catalogue finds markers by pattern, so a
 * sentence denying one would create one. Its `when` has a reviewer *start*
 * a plain rotation and then a recovery, and there is no door to start a
 * recovery: `Participant::recovering` opens the new identity on this
 * device's own storage, so it needs a mark the next start resumes from.
 * The warnings are what this proves; the flow is what the row still waits
 * on, and closing it on the warnings alone would be closing it on half.
 */
class RotationTest {

    private fun rotation(patron: String = "Pat", subnet: String = "Ward") =
        Disclosure.words(Disclosure.Topic.UNLINKED_PATH, patron = patron, subnet = subnet)
            .joinToString(" ")

    private fun recovery(patron: String = "Pat", subnet: String = "Ward") =
        Disclosure.words(Disclosure.Topic.LINKED_PATH, patron = patron, subnet = subnet)
            .joinToString(" ")

    @Test
    fun a_plain_rotation_warns_that_prior_standing_is_abandoned() {
        val said = rotation()
        assertTrue("there are words", said.isNotEmpty())
        // what it costs: the new key carries only what is established
        // through the patron who adopts it, and nothing of the old key's
        assertTrue("says what the new identifier carries: $said", said.contains("only such"))
        assertTrue("names the patron it is established through: $said", said.contains("Pat"))
        // and what it does not cost: other subnets are untouched, which is
        // the half a person would otherwise fear
        assertTrue("says the old one still reaches elsewhere: $said", said.contains("other subnets"))
        // **and it does not claim a link**, which is the other path
        assertTrue("an unlinked rotation must not speak of publishing: $said", !said.contains("publish"))
        assertTrue("$said", !said.contains("traceable"))
    }

    @Test
    fun a_recovery_states_that_the_link_will_be_published() {
        val said = recovery()
        assertTrue("says the fact of the new identity is published: $said", said.contains("publishing"))
        assertTrue("and that the two are tied: $said", said.contains("traceable"))
        assertTrue("names which subnet: $said", said.contains("Ward"))
    }

    @Test
    fun the_two_paths_do_not_say_the_same_thing() {
        // a person choosing between them must be able to tell them apart,
        // which is the whole of what PRD-03 asks of this screen
        assertTrue("the two warnings are the same text", rotation() != recovery())
        assertTrue(
            "only the recovery speaks of publishing the link",
            recovery().contains("publishing") && !rotation().contains("publishing"),
        )
    }

    @Test
    fun a_party_is_named_by_what_they_published_or_by_their_keyhash() {
        // **never a name this device invented** (PRD-14): an alias is the
        // party's own published claim, and absent one the keyhash is what
        // this device honestly holds
        assertEquals("Allotments", Rotation.name("ab".repeat(32), "Allotments"))
        assertEquals("Allotments", Rotation.name("ab".repeat(32), "  Allotments  "))
        assertEquals("abababab…", Rotation.name("ab".repeat(32), null))
        assertEquals("abababab…", Rotation.name("ab".repeat(32), "   "))
    }

    @Test
    fun the_sentences_name_the_parties_rather_than_leaving_holes() {
        // a warning with `{patron}` in it is a bug somebody reports; one
        // with a gap reads as finished, which is worse
        for (said in listOf(rotation(), recovery())) {
            assertTrue("a hole was left unfilled: $said", !said.contains("{patron}"))
            assertTrue("a hole was left unfilled: $said", !said.contains("{subnet}"))
        }
    }
    // ---- the resume, which is what makes a rotation survivable ---------

    /**
     * **A mark outranks the sealed seeds**, which is the whole invariant.
     *
     * `Participant.rotating` opens the new identity on this device's own
     * storage, so between beginning a rotation and sealing its new seeds
     * the archive belongs to a key the seeds do not name. A start obeying
     * the seeds there would open the wrong identity on the right archive.
     */
    @Test
    fun a_start_obeys_the_mark_over_the_seeds() {
        assertTrue(
            "a mark means finish the rotation, whatever the seeds say",
            Rotation.start(hasSeeds = true, hasMark = true) is Rotation.Start.Rotate,
        )
        // and even with no seeds at all: the mark carries its own
        assertTrue(
            Rotation.start(hasSeeds = false, hasMark = true) is Rotation.Start.Rotate,
        )
    }

    @Test
    fun without_a_mark_the_sealed_seeds_are_the_identity() {
        assertTrue(
            Rotation.start(hasSeeds = true, hasMark = false) is Rotation.Start.Ordinary,
        )
    }

    @Test
    fun with_neither_there_is_nothing_to_start() {
        // a device whose first run never completed, which must not be
        // reported as a refused kernel
        assertTrue(
            Rotation.start(hasSeeds = false, hasMark = false) is Rotation.Start.Unstarted,
        )
    }

    @Test
    fun re_entering_the_rotation_after_a_crash_is_the_same_decision() {
        // **idempotent by construction**: the inputs a start reads do not
        // change until the new seeds are sealed, so a process that dies
        // mid-rotation reaches the same branch next time rather than a
        // different one
        val first = Rotation.start(hasSeeds = true, hasMark = true)
        val again = Rotation.start(hasSeeds = true, hasMark = true)
        assertEquals(first::class, again::class)
        // and once the seeds are the new ones and the mark is gone, it is
        // an ordinary start — the only transition out
        assertTrue(Rotation.start(hasSeeds = true, hasMark = false) is Rotation.Start.Ordinary)
    }

    @Test
    fun the_mark_is_kept_under_names_of_its_own() {
        // not under "seeds", which is the identity in force: a rotation
        // that overwrote it before finishing would be the hazard this
        // whole mechanism exists to avoid
        assertTrue(Rotation.MARK_NEW != "seeds")
        assertTrue(Rotation.MARK_PRIOR != "seeds")
        assertTrue(Rotation.MARK_NEW != Rotation.MARK_PRIOR)
    }

}
