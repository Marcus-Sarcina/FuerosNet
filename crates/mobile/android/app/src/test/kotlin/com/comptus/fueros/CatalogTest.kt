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

    private fun said(status: ULong, page: Kernel.Page? = null, unreadable: String? = null) =
        Catalog.answer(Kernel.Answered(status, page, unreadable))

    /**
     * **The diagnostic line, which is not the page** [2026-10-28]. It
     * used to quote the first line of the raw HTTP message, which was
     * this shell reading one; the kernel reads it now, so what is
     * quoted here is what the kernel found.
     */
    @Test
    fun each_of_the_six_statuses_is_said_in_words_and_what_was_read_is_quoted() {
        assertEquals("delivered", said(0UL))
        assertEquals(
            "delivered — HTTP 200, text/html, 4 bytes",
            said(
                0UL,
                Kernel.Page(200, "text/html", "utf-8", Kernel.Shape.PAGE, "body".toByteArray()),
            ),
        )
        assertEquals(
            "a type nobody declared is said to be nothing, never guessed at",
            "delivered — HTTP 200, nothing declared, 0 bytes",
            said(0UL, Kernel.Page(200, null, null, Kernel.Shape.OTHER, ByteArray(0))),
        )
        assertEquals(
            "delivered, and unreadable: content-length twice",
            said(0UL, unreadable = "content-length twice"),
        )
        assertEquals("refused", said(1UL))
        assertEquals("the resource is unavailable", said(2UL))
        assertEquals("the request was malformed", said(3UL))
        assertEquals("no subtree acknowledgement covers you", said(4UL))
        assertEquals("no role of yours matches", said(5UL))
        assertEquals("status 9", said(9UL))
    }

    /** And it says the same words about a status as the screen does, so
     *  the two never disagree about one. */
    @Test
    fun the_status_words_come_from_one_place() {
        for (s in 0..6) {
            assertEquals(ResourcePage.delivery(s.toULong()), said(s.toULong()))
        }
    }
}
