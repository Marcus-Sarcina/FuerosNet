package com.comptus.fueros

import android.view.View
import android.view.WindowInsets

/**
 * **What every screen does about the system bars, in one place**
 * [2026-10-27].
 *
 * The defect this closes: a screen that set its padding and
 * `fitsSystemWindows` on the same view had that padding **replaced** by the
 * window insets rather than added to, so its own margins went, and the
 * bottom inset landed inside a `ScrollView` where it scrolls away — which
 * put the foot of every scrolling screen under the system navigation bar.
 * The thread's send box sat behind the gesture pill.
 *
 * `MeetActivity` had already met this and solved it by hand, with the
 * reason beside it: *Android 15 draws edge to edge and does not honour
 * `fitsSystemWindows` on a plain layout*. So the mechanism is not in doubt
 * and this is that mechanism, shared: **the insets are added to what a
 * view already carries, never substituted for it**, and they are added to
 * the frame rather than to the scrolling content, so nothing is ever drawn
 * under a bar in the first place.
 *
 * The arithmetic is kept apart from the view so a test on the JVM can say
 * it, which is the half that was wrong.
 */
object Screen {

    /**
     * **The side gutter every screen but the optical exchange carries**
     * (`Robot/light-client-screens.md`, the gutter rule).
     *
     * The ruling is *none on the optical exchange, very narrow everywhere
     * else* [author, 2026-10-27]. **This is the drawing's own number**,
     * not a guess at what narrow means: the artboards set 14 against a
     * 390-wide phone, so 14dp is that at any density. It stood at zero
     * while nothing was drawn to the design, which was what the framework
     * had been doing by accident, and read as text flush to the glass the
     * moment a screen was dark. One edit here moves every screen.
     */
    const val GUTTER_DP = 14

    /** The gutter in this screen's pixels. */
    fun gutter(context: android.content.Context): Int =
        (GUTTER_DP * context.resources.displayMetrics.density).toInt()

    /** A view's four paddings, or the four insets, in raw pixels. */
    data class Pad(val left: Int, val top: Int, val right: Int, val bottom: Int)

    /**
     * **The padding a frame carries once the bars are accounted for:
     * added, never substituted.** The whole of the defect was a
     * substitution, so this is the rule stated as arithmetic.
     */
    fun padding(own: Pad, bars: Pad): Pad =
        Pad(own.left + bars.left, own.top + bars.top, own.right + bars.right, own.bottom + bars.bottom)

    /**
     * Hold `frame` clear of the system bars for as long as it lives: its
     * own padding, plus whatever the bars take, re-applied whenever they
     * change — a keyboard, a rotation, a gesture bar that comes and goes.
     *
     * Called on the view the screen's content sits in: the `ScrollView`
     * where one is the content view, the root otherwise. The insets are
     * passed on rather than consumed, so a child that wants them still
     * sees them.
     */
    fun insetting(frame: View) {
        val own = Pad(frame.paddingLeft, frame.paddingTop, frame.paddingRight, frame.paddingBottom)
        frame.setOnApplyWindowInsetsListener { v, insets ->
            val b = insets.getInsets(WindowInsets.Type.systemBars())
            val p = padding(own, Pad(b.left, b.top, b.right, b.bottom))
            v.setPadding(p.left, p.top, p.right, p.bottom)
            insets
        }
        // **in case the bars were dispatched before this was set**: a
        // listener installed after the first pass would otherwise wait for
        // a rotation to take effect
        frame.requestApplyInsets()
    }
}
