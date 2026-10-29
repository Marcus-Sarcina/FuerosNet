package com.comptus.fueros

import android.app.Activity
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.text.InputType
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **People: who this client holds, and the one way to add somebody it has
 * never met** (`light-client-requirements.md` §1.4, §4.3; the first of the
 * four functions the client review of 2026-10-11 found the app could not
 * perform at all — there was no screen and no kernel call either).
 *
 * **What an intake carries is a defined object, and the kernel decides
 * which.** An `Introduction` (`wire-format.md` §2.3) is somebody's key
 * material with a locator of theirs; an `EndpointRecord` (§7.6) is where a
 * node answers, signed by that node. A person scanning a code should not
 * have to know the difference, so both are offered to the kernel and
 * whichever parses is what was taken.
 *
 * **Why a node's record is here at all.** A locator names an anchor and a
 * path, never an address (design §12.3), so an introduction alone cannot
 * be acted on: §12.3 Case 1 has the holder look the anchor up "in his
 * local table for an address to start from — his contacts", and until some
 * record of where that anchor answers arrives, there is nobody to ask.
 * This screen is where one arrives.
 *
 * **The carriage is this shell's own**, as the ceremony's codes are: base45
 * in a QR ([Optical]), or the same text pasted. The protocol neither names
 * nor depends on it.
 */
class PeopleActivity : Activity() {

    private lateinit var narration: TextView
    private lateinit var roll: TextView
    private lateinit var shown: android.widget.ImageView
    private var camera: QrCamera? = null
    private var preview: SurfaceView? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        narration = TextView(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.sub)
            text = "Scan a code somebody showed you, or paste one."
        }
        roll = TextView(this).apply {
            textSize = 14f
            typeface = Typeface.MONOSPACE
        }
        val pasted = EditText(this).apply {
            hint = "a code, as text"
            textSize = 14f
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            typeface = Typeface.MONOSPACE
        }
        val take = Button(this).apply {
            text = "Take it"
            setOnClickListener {
                val text = pasted.text.toString().trim().uppercase()
                val bytes = Optical.bytes(text)
                if (bytes == null) {
                    narration.text = "that is not one of these codes"
                } else {
                    narration.text = Kernel.takeIntake(bytes)
                    pasted.setText("")
                    draw()
                }
            }
        }
        val scan = Button(this).apply {
            text = "Scan a code"
            setOnClickListener { scan() }
        }
        val mine = Button(this).apply {
            text = "Show my code"
            setOnClickListener { showMine() }
        }
        shown = android.widget.ImageView(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                700,
            )
            visibility = android.view.View.GONE
        }
        preview = SurfaceView(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                600,
            )
        }

        // **the way to E3** [2026-10-28]: a patron is somebody in the
        // horizon this screen shows, and ending that relationship is the
        // one act about a person that changes what this device can reach.
        // It is a row here rather than a seventh destination on Home,
        // which is what the connection line did for the attach screen.
        val authority = TextView(this).apply {
            text = "Authority — who you are under, and what leaving costs"
            textSize = 15f
            setPadding(0, 20, 0, 24)
            isClickable = true
            setOnClickListener {
                startActivity(
                    android.content.Intent(this@PeopleActivity, LeavingActivity::class.java),
                )
            }
        }

        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(Screen.gutter(context), 48, Screen.gutter(context), 48)
            addView(authority)
            addView(narration)
            addView(scan)
            addView(preview)
            addView(pasted)
            addView(take)
            addView(mine)
            addView(shown)
            addView(
                TextView(this@PeopleActivity).apply {
                    textSize = 12f
                    setTextColor(Palette.now.sub)
                    text = "\nHeld:"
                },
            )
            addView(roll)
        }
        // **the bars are the frame's, not the scrolling content's**
        // ([`Screen`]): an inset applied inside a `ScrollView`
        // scrolls away, which is how the foot of this screen came to
        // sit under the navigation bar
        val frame = ScrollView(this).apply { addView(body) }
        Screen.insetting(frame)
        setContentView(frame)
        draw()
    }

    /** One code, then the camera closes: an intake is a deliberate act. */
    private fun scan() {
        val holder: SurfaceHolder = preview?.holder ?: return
        val surface: Surface? = holder.surface?.takeIf { it.isValid }
        camera?.close()
        val cam = QrCamera(this)
        camera = cam
        narration.text = "looking…"
        val refused = cam.readOne(
            facing = QrCamera.Facing.REAR,
            preview = surface,
            which = "intake",
            failed = { why -> runOnUiThread { narration.text = why } },
        ) { bytes ->
            runOnUiThread {
                narration.text = Kernel.takeIntake(bytes)
                draw()
            }
            cam.close()
        }
        if (refused != null) narration.text = refused
    }

    /**
     * **This device's own code**, for the other person to scan. Drawn only
     * when the kernel can make one: a card names the anchor of this
     * client's own locator and where that anchor answers (§2.4), and a
     * client with neither has nothing anybody could act on.
     */
    private fun showMine() {
        val card = Kernel.ownCard()
        if (card == null) {
            narration.text =
                "no code to show yet: this client needs a position and an address for its anchor"
            shown.visibility = android.view.View.GONE
            return
        }
        val m = Optical.matrix(card)
        val px = IntArray(m.width * m.height)
        for (y in 0 until m.height) {
            for (x in 0 until m.width) {
                px[y * m.width + x] = if (m.get(x, y)) Color.BLACK else Color.WHITE
            }
        }
        shown.setImageBitmap(
            android.graphics.Bitmap.createBitmap(
                px,
                m.width,
                m.height,
                android.graphics.Bitmap.Config.ARGB_8888,
            ),
        )
        shown.visibility = android.view.View.VISIBLE
        narration.text = "your code — ${card.size} bytes, theirs to scan"
    }

    private fun draw() {
        roll.text = People.roll(Kernel.people())
    }

    /**
     * **Ask for what each contact published, on opening** (§4.3): holding
     * a locator is what lets this device fetch a profile, so the screen
     * that shows names is where the asking belongs. Off the main thread,
     * since each fetch is a round trip to wherever that locator leads;
     * the roll is redrawn as answers land.
     */
    override fun onResume() {
        super.onResume()
        draw()
        Thread {
            Kernel.requestProfiles()
            runOnUiThread { if (!isFinishing) draw() }
        }.start()
    }

    override fun onStop() {
        super.onStop()
        camera?.close()
        camera = null
    }
}
