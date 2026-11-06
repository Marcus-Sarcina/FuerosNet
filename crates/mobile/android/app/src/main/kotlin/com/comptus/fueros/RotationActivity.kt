package com.comptus.fueros

import android.app.Activity
import android.content.Intent
import android.graphics.Typeface
import android.os.Bundle
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **Changing the key a subnet knows you by** (design §9.0;
 * `wire-format.md` §4.6; `light-client-requirements.md` §2; PRD-03).
 *
 * **Rotation is adoption** — design §9.0's own title, and the thing this
 * screen first got wrong. *"Adoption and key rotation are one operation,
 * not two… rotation is simply the case where the presented archive
 * belongs to a different key."* So a rotation is not a setting and not a
 * new identity made alone: **it happens at a meeting with the patron**,
 * who adopts the new key, and it is re-anchored by physical presence
 * [author, 2026-11-05] — the new key inheriting the local status,
 * permissions and state the old one held in that subnet.
 *
 * **Which is why this screen sends you to a meeting.** A new identity made
 * on its own re-anchors nothing: it is a stranger to the subnet, and the
 * first draft of this screen offered exactly that under the word rotation.
 *
 * **And none of this is a series reissue** [author, 2026-11-05]. A reissue
 * keeps the same key and restarts the sequence, so the history can branch
 * per subnet; a rotation replaces the key the subnet knows. They are
 * different operations and only one of them changes who you are there.
 *
 * Two paths, and the whole of what this screen owes is that a person can
 * tell them apart before taking either.
 *
 * **Unlinked.** An adoption of a new key carrying no `Recovery`, so
 * nothing ties it to the old one and nothing the old key stood on comes
 * with it — design §9.0's *"genuinely-new user"* case. *"Your new
 * identifier will carry only such privileges and relationships as you
 * have established directly through"* the patron who adopts it [author,
 * 2026-11-05].
 *
 * **Linked.** A recovery (`wire-format.md` §4.1), which carries both
 * halves — the old key's own signature and a prior counterparty's
 * recognition — so standing comes across and **the link is published
 * within the horizon**. What is given up is unlinkability: anyone who
 * sees the recovery sees that these two keys are one person.
 *
 * **Neither is undone.** `wire-format.md` §4.6 has a series never
 * re-entered, so the old line is sealed as the old key's last act and
 * there is no path back to it.
 *
 * ## What this screen does not do, and why
 *
 * **The linked path is performed here, and survives a dying process**
 * [2026-11-05]. `Participant::rotating` opens the new identity on *this
 * device's own storage* and saves, so the archive on disk becomes the new
 * identity's the moment it runs. A mark is therefore sealed *before*
 * anything opens and cleared only after the new seeds are the sealed ones,
 * and [Rotation.start] has every start obey a mark over the seeds — so a
 * process dying mid-rotation leaves one a later start finishes rather than
 * a key and an archive that do not match.
 *
 * **The unlinked path is not a door here, and is not a rotation.**
 * `wire-format.md` §4.1 admits no `Recovery` carrying one half, so a
 * prior key is required and an unlinked path is not this operation at all:
 * it is starting over with a new identity, which on a device that already
 * holds one needs the old archive cleared, or the fresh key opens on the
 * old identity's records. That is the most destructive act in the app and
 * is owed its own care rather than a button here.
 */
class RotationActivity : Activity() {

    private lateinit var body: LinearLayout

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val g = Screen.gutter(this)
        body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(g, g, g, g)
        }
        val root = ScrollView(this).apply { addView(body) }
        Screen.insetting(root)
        setContentView(root)
        draw()
    }

    private fun draw() {
        body.removeAllViews()
        heading("Changing your key")
        val lines = Kernel.lines()
        if (lines.isEmpty()) {
            para(
                "This key stands under no patron, so there is nothing to carry across " +
                    "and nothing to abandon. Making a new identity costs you nothing you hold.",
            )
            return
        }
        para(
            "This key stands under ${lines.size} " +
                "patron${if (lines.size == 1) "" else "s"}. What a rotation costs depends " +
                "on which of the two you take, and neither can be undone: a series is " +
                "never re-entered.",
        )

        // **each line, named**, because the author's sentences name the
        // parties and a warning about "your subnets" names nobody
        for (line in lines) {
            val patron = Rotation.name(line.patron.hex(), Kernel.aliasOf(line.patron.hex()))
            val subnet = Rotation.name(
                line.anchor.hex(),
                Kernel.subnets().firstOrNull { it.id.hex() == line.anchor.hex() }?.alias,
            )
            heading("$subnet, under $patron")
            for (said in Disclosure.words(Disclosure.Topic.UNLINKED_PATH, patron = patron, subnet = subnet)) {
                para(said)
            }
            sub("If you link instead:")
            for (said in Disclosure.words(Disclosure.Topic.LINKED_PATH, patron = patron, subnet = subnet)) {
                para(said)
            }
        }

        heading("Which one")
        para(
            "Unlinked: a new identity, nothing tying it to this one, and nothing this " +
                "key stands on coming with it. You are adopted afresh.",
        )
        para(
            "Linked: a recovery, which carries your standing and publishes within your " +
                "horizon that the two keys are the same person. You give up being two " +
                "people rather than one.",
        )
        para(
            "Either way it happens in person, with the patron who adopts the new key: " +
                "rotation is adoption, and what re-anchors you is the meeting rather " +
                "than anything this screen can do alone.",
        )
        button("Meet the patron") {
            startActivity(Intent(this, MeetActivity::class.java))
        }
        sub(
            "Carrying your standing across needs the old key present to sign with, " +
                "and this device holds it — so nothing is typed. The old line is " +
                "sealed as that key's last act, and a series is never re-entered.",
        )
        said?.let { para(it) }
        button("Rotate now, carrying your standing") { rotate() }
    }

    /** What came of beginning one, across a redraw. */
    private var said: String? = null

    /**
     * **Begin the rotation** ([Kernel.beginRotation]).
     *
     * A new identity is minted here and the one this device holds signs
     * for it. **The phrase is not asked for**: design §9.0 has the old key
     * in hand wherever a rotation carries its inheritance, and this device
     * has it — asking a person to type a phrase it already holds would
     * prove nothing and put the phrase through a keyboard.
     *
     * **The mark is written before anything opens**, so a process dying
     * here leaves a rotation some later start finishes rather than a key
     * and an archive that do not match ([Rotation.start]).
     */
    private fun rotate() {
        val prior = Kernel.heldSeedsForRotation(this)
        if (prior == null) {
            said = "This device holds no key to rotate from."
            draw()
            return
        }
        val master = ByteArray(Identity.ENTROPY_BYTES).also {
            java.security.SecureRandom().nextBytes(it)
        }
        val fresh = Identity.seeds(master)
        master.fill(0)
        said = if (Kernel.beginRotation(this, fresh, prior)) {
            "The rotation has begun. It completes on this start, and any later one " +
                "if this is interrupted."
        } else {
            "This device's own storage refused it, and nothing was begun."
        }
        draw()
    }

    private fun heading(t: String) = body.addView(
        TextView(this).apply {
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Palette.now.ink)
            setPadding(0, 24, 0, 8)
            text = t
        },
    )

    private fun para(t: String) = body.addView(
        TextView(this).apply {
            textSize = 15f
            setTextColor(Palette.now.ink)
            setPadding(0, 0, 0, 12)
            text = t
        },
    )

    private fun sub(t: String) = body.addView(
        TextView(this).apply {
            textSize = 13f
            setTextColor(Palette.now.sub)
            setPadding(0, 0, 0, 12)
            text = t
        },
    )

    private fun button(label: String, onClick: () -> Unit) = body.addView(
        Button(this).apply {
            text = label
            setOnClickListener { onClick() }
        },
    )
}
