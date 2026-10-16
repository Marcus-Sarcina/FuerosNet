package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * What the Catalog screen says of what it holds (`wire-format.md` §6.4)
 * and of what a resource answered (§11.1).
 */
class CatalogTest {
    private fun offered(truncated: Boolean = false, stale: Boolean = false) =
        Kernel.Offered(
            node = "ab12cd34",
            resource = "ef".repeat(32),
            owner = "99887766",
            serviceType = "rhtn-forum",
            instance = "the parish board",
            truncated = truncated,
            stale = stale,
        )

    @Test
    fun an_entry_names_its_type_instance_owner_and_the_node_that_served_it() {
        assertEquals(
            "rhtn-forum  the parish board  by 99887766  at ab12cd34",
            Catalog.line(offered()),
        )
    }

    @Test
    fun a_truncated_or_stale_portion_is_said_and_never_smoothed_over() {
        assertTrue(Catalog.line(offered(truncated = true)).endsWith("— more than this held"))
        assertTrue(Catalog.line(offered(stale = true)).endsWith("— not reached last sweep"))
        assertTrue(
            Catalog.line(offered(truncated = true, stale = true))
                .endsWith("— more than this held, not reached last sweep"),
        )
        assertEquals("nothing held yet", Catalog.roll(emptyList()))
    }

    @Test
    fun each_of_the_six_statuses_is_said_in_words_and_a_body_is_quoted_once() {
        assertEquals("delivered", Catalog.answer(Kernel.Answered(0UL, null)))
        assertEquals(
            "delivered — HTTP/1.1 200 OK",
            Catalog.answer(Kernel.Answered(0UL, "HTTP/1.1 200 OK\r\n\r\nbody".toByteArray())),
        )
        assertEquals("refused", Catalog.answer(Kernel.Answered(1UL, null)))
        assertEquals("the resource is unavailable", Catalog.answer(Kernel.Answered(2UL, null)))
        assertEquals("the request was malformed", Catalog.answer(Kernel.Answered(3UL, null)))
        assertEquals(
            "no subtree acknowledgement covers you",
            Catalog.answer(Kernel.Answered(4UL, null)),
        )
        assertEquals("no role of yours matches", Catalog.answer(Kernel.Answered(5UL, null)))
        assertEquals("status 9", Catalog.answer(Kernel.Answered(9UL, null)))
    }
}
