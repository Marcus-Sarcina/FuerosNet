package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The system bars are **added** to a screen's own padding, never
 * substituted for it.
 *
 * That substitution was the whole defect: a screen setting its padding and
 * `fitsSystemWindows` on one view had the framework overwrite the padding
 * with the insets, so its margins went and the bottom inset landed inside
 * the scrolling content, where it scrolls away. The foot of every
 * scrolling screen sat under the navigation bar, the thread's send box
 * among them. A view cannot be held on the JVM; this arithmetic can, and
 * it is the half that was wrong.
 */
class ScreenTest {

    private val none = Screen.Pad(0, 0, 0, 0)

    @Test
    fun the_bars_are_added_to_what_the_screen_already_carries() {
        val own = Screen.Pad(0, 48, 0, 48)
        val bars = Screen.Pad(0, 0, 0, 126)
        assertEquals(Screen.Pad(0, 48, 0, 174), Screen.padding(own, bars))
    }

    @Test
    fun a_screen_with_no_padding_of_its_own_is_the_bars_alone() {
        assertEquals(Screen.Pad(0, 83, 0, 126), Screen.padding(none, Screen.Pad(0, 83, 0, 126)))
    }

    @Test
    fun no_bars_leaves_the_screens_own_padding_exactly() {
        val own = Screen.Pad(8, 24, 8, 16)
        assertEquals(own, Screen.padding(own, none))
    }

    @Test
    fun a_cutout_or_a_landscape_bar_is_added_at_the_side_too() {
        val own = Screen.Pad(8, 48, 8, 48)
        val bars = Screen.Pad(132, 0, 44, 0)
        assertEquals(Screen.Pad(140, 48, 52, 48), Screen.padding(own, bars))
    }

    /** **Never a substitution**, which is what the framework did: whatever
     *  the bars take, the screen's own padding survives inside it. */
    @Test
    fun the_screens_own_padding_is_never_lost_whatever_the_bars_are() {
        val own = Screen.Pad(8, 48, 8, 48)
        for (b in listOf(0, 1, 63, 126, 200)) {
            val got = Screen.padding(own, Screen.Pad(b, b, b, b))
            assertEquals(own.left + b, got.left)
            assertEquals(own.top + b, got.top)
            assertEquals(own.right + b, got.right)
            assertEquals(own.bottom + b, got.bottom)
        }
    }

    /** The gutter is one number for every screen but the optical
     *  exchange, and it is the author's to set. */
    @Test
    fun the_gutter_is_one_number_and_is_not_negative() {
        assertEquals(0, Screen.GUTTER)
    }
}
