package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **The launch pickers' intersection, and what a node is sized for**
 * (`Robot/future-features.md` entry 2 [author, 2026-11-02]; the sizing
 * [author, 2026-11-05]).
 */
class SizingTest {

    private data class Size(val slug: String, val mb: Long, val regions: List<String>)
    private data class Region(val slug: String)

    private val sizes = listOf(
        Size("s-1vcpu-512mb-10gb", 512, listOf("nyc1", "ams3")),
        Size("s-1vcpu-1gb", 1024, listOf("nyc1", "ams3", "syd1")),
        Size("s-1vcpu-2gb", 2048, listOf("nyc1", "syd1")),
        Size("s-2vcpu-4gb", 4096, listOf("nyc1")),
        // a size whose region list did not parse: constrains nothing
        Size("s-mystery", 2048, listOf()),
    )
    private val regions = listOf(Region("nyc1"), Region("ams3"), Region("syd1"), Region("lon1"))

    @Test
    fun the_floor_and_the_band_are_what_the_estimate_says() {
        // 512 MB is below the floor: no room for three bundles and the
        // node's own work
        assertFalse(Sizing.adequate(512))
        assertTrue(Sizing.adequate(1024))
        // **both 1 GB and 2 GB are recommended** [author, 2026-11-05]:
        // the working set fits either and the difference is headroom
        assertTrue("1 GB is recommended", Sizing.recommended(1024))
        assertTrue("2 GB is recommended", Sizing.recommended(2048))
        // and 4 GB is adequate without being recommended — more than this
        // project can justify advising
        assertTrue(Sizing.adequate(4096))
        assertFalse("4 GB is not marked", Sizing.recommended(4096))
        assertFalse("nor is 512 MB", Sizing.recommended(512))
    }

    @Test
    fun both_recommended_sizes_are_marked_and_nothing_else_is() {
        val marked = Sizing.picked(sizes) { it.mb }.map { it.slug }
        assertTrue("1 GB marked", marked.contains("s-1vcpu-1gb"))
        assertTrue("2 GB marked", marked.contains("s-1vcpu-2gb"))
        assertFalse("512 MB not marked", marked.contains("s-1vcpu-512mb-10gb"))
        assertFalse("4 GB not marked", marked.contains("s-2vcpu-4gb"))
    }

    @Test
    fun a_region_narrows_the_sizes_to_those_offered_there() {
        // ams3 has the 512 MB and the 1 GB, not the 2 GB or the 4 GB
        val there = Sizing.sizesIn("ams3", sizes) { it.regions }.map { it.slug }
        assertEquals(listOf("s-1vcpu-512mb-10gb", "s-1vcpu-1gb", "s-mystery"), there)
        // **a size listing no regions constrains nothing**: an adapter that
        // could not parse the list must not make every region look wrong
        assertTrue(there.contains("s-mystery"))
    }

    @Test
    fun a_size_narrows_the_regions_to_those_offering_it() {
        val where = Sizing.regionsFor(listOf("nyc1", "syd1"), regions) { it.slug }.map { it.slug }
        assertEquals(listOf("nyc1", "syd1"), where)
        // and an unparsed list leaves every region standing
        assertEquals(regions.size, Sizing.regionsFor(listOf(), regions) { it.slug }.size)
    }

    @Test
    fun regions_that_cannot_host_a_node_are_last_and_not_hidden() {
        val ordered = Sizing.regionsByRecommended(
            regions, sizes, { it.slug }, { it.mb }, { it.regions },
        ).map { it.slug }
        // lon1 offers nothing at all, so it goes last — and is still there,
        // because an operator may know better than this screen
        assertEquals(listOf("nyc1", "ams3", "syd1", "lon1"), ordered)
        assertTrue("nothing is dropped", ordered.size == regions.size)
        // ams3 qualifies on the floor rather than the band's top: it offers
        // 1 GB and not 2, which is a region a node runs in
        assertTrue(
            Sizing.meetsRecommended("ams3", sizes, { it.mb }, { it.regions }),
        )
        assertFalse(
            Sizing.meetsRecommended("lon1", sizes, { it.mb }, { it.regions }),
        )
    }

    @Test
    fun the_ordering_and_the_absence_both_say_why() {
        // `light-client-requirements.md` §9: a refusal renders its reason,
        // and a silent reordering is a refusal in disguise
        assertTrue(Sizing.whyBehind().contains("listed last"))
        assertTrue(Sizing.whyBehind().contains("not refused"))
        val why = Sizing.whyAbsent("s-2vcpu-4gb", "ams3")
        assertTrue("names both: $why", why.contains("s-2vcpu-4gb") && why.contains("ams3"))
        assertTrue("says the provider said so: $why", why.contains("provider's own list"))
    }

    @Test
    fun the_note_says_what_the_sizing_rests_on() {
        val said = Sizing.note()
        // the firmest input is the protocol's fanout, not a measurement
        assertTrue("names the ten-member ceiling: $said", said.contains("ten members"))
        assertTrue("and the eight it is sized for: $said", said.contains("eight"))
        assertTrue("and the three bundles: $said", said.contains("three application bundles"))
        // and it does not promise that more memory buys reach
        assertTrue("$said", said.contains("headroom, not reach"))
    }
}
