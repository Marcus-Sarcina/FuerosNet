package com.comptus.fueros

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.view.Gravity
import android.widget.Button
import android.widget.CheckBox
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * Meet: the ceremony flow (screens sheet section D). One Activity walks the
 * whole sequence, because a ceremony is one continuous flow and not a set
 * of places you navigate between. The state lives in [Kernel]'s [Meet], so
 * a recreation mid-flow rejoins it.
 *
 * **What this build carries and what it cannot.** The flow, its front-
 * loaded brief, its hands-off phase and its review-before-sign gate are all
 * here and enforced by [Meet]. **D1 through D4 are wired to hardware**: the
 * bootstrap and anchor QRs on the rear and selfie cameras, the intent over
 * a Bluetooth LE bearer, the proximity tap over NFC (with the optical pass
 * D2 already made), and the guided capture on the selfie camera — every
 * radio and camera half compiling against the platform's API and **none of
 * it run on a device**, which the files behind each say at the top. What is
 * still not real is D6: signing needs the record artifacts the steps
 * produce, which a single process has no second party to complete, so a
 * dim **walkthrough** control stands in there — scaffolding, labelled as
 * standing in for a signal a real pair of devices would raise.
 */
class MeetActivity : Activity() {

    private lateinit var body: LinearLayout
    private lateinit var scroll: ScrollView
    private var camera: QrCamera? = null

    /** Hands-off steps started this screen's life: so a redraw rejoins a
     *  running step rather than starting it again. */
    private val started = java.util.Collections.synchronizedSet(mutableSetOf<String>())

    private val sink = object : Meet.Ui {
        override fun render() = runOnUiThread { redraw() }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        body = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        scroll = ScrollView(this).apply {
            addView(body)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.MATCH_PARENT,
            )
        }
        setContentView(
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                fitsSystemWindows = true
                setPadding(48, 48, 48, 48)
                addView(scroll)
            },
        )
    }

    override fun onStart() {
        super.onStart()
        Consent.host(this)
        // the NFC reader side needs a foreground activity (D3); this is it
        ProximityChannels.host(this)
        Kernel.meet()?.bind(sink) ?: redraw()
    }

    override fun onStop() {
        Consent.release(this)
        ProximityChannels.release(this)
        Kernel.meet()?.unbind(sink)
        // the camera goes back the moment this screen stops: holding one
        // behind a screen nobody is looking at is a camera nobody consented
        // to
        camera?.close()
        camera = null
        super.onStop()
    }

    private fun redraw() {
        body.removeAllViews()
        val m = Kernel.meet()
        if (m == null) {
            entry()
            report()
            return
        }
        title(
            when (m.step()) {
                Meet.Step.INTENT -> "Meet ${m.counterpartyName}"
                Meet.Step.BRIEF -> "Before you begin"
                Meet.Step.OPTICAL -> "Exchanging (1/3)"
                Meet.Step.PROXIMITY -> "Ranging (2/3)"
                Meet.Step.CAPTURE -> "Capturing (3/3)"
                Meet.Step.VERIFIERS -> "Verifiers"
                Meet.Step.REVIEW -> "Review and sign"
                Meet.Step.DONE -> "Done"
                Meet.Step.STOPPED -> "Stopped"
            },
        )
        when (m.step()) {
            Meet.Step.INTENT -> intent(m)
            Meet.Step.BRIEF -> brief(m)
            Meet.Step.OPTICAL -> optical(m)
            Meet.Step.PROXIMITY -> proximity(m)
            Meet.Step.CAPTURE -> capture(m)
            Meet.Step.VERIFIERS -> verifiers(m)
            Meet.Step.REVIEW -> review(m)
            Meet.Step.DONE -> done(m)
            Meet.Step.STOPPED -> stopped(m)
        }
        logLines(m)
        report()
    }

    /** The Report action (`Robot/field-test-diagnostics.md`, section 4):
     *  the run's events and a header, zipped and offered to the share
     *  sheet. Fieldtest flavour only; the releasable flavour has no button
     *  and nothing it would send. */
    private fun report() {
        if (!BuildConfig.FIELD_TEST) return
        button("Send diagnostics (field test)") { Report.send(this) }
    }

    /**
     * Every permission asked for is an event, and so is each answer
     * (`Robot/field-test-diagnostics.md`, section 3.6, `permission`).
     */
    private fun ask(code: Int, vararg permissions: String) {
        for (p in permissions) {
            Diag.event("permission", "which" to short(p), "state" to "requested")
        }
        requestPermissions(arrayOf(*permissions), code)
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        for (i in permissions.indices) {
            val granted = grantResults.getOrNull(i) == PackageManager.PERMISSION_GRANTED
            Diag.event("permission", "which" to short(permissions[i]), "state" to if (granted) "granted" else "denied")
        }
        redraw()
    }

    private fun short(permission: String) = permission.substringAfterLast('.').lowercase()

    private fun held(permission: String) =
        checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED

    /** The three Bluetooth permissions the bearer needs at API 31 and up;
     *  asked for before the hands-off phase, since a radio refused there
     *  would only be caught, not asked for. */
    private val bluetooth = arrayOf(
        Manifest.permission.BLUETOOTH_SCAN,
        Manifest.permission.BLUETOOTH_ADVERTISE,
        Manifest.permission.BLUETOOTH_CONNECT,
    )

    // ---- the entry, before a ceremony is live --------------------------

    private fun entry() {
        val peer = Kernel.peerKey()
        if (peer == null) {
            para("A ceremony is with someone you are provisioned to. None yet.")
            return
        }
        // D1a, the initiator's dialogue, as the author specified it: a
        // regular meeting is the DEFAULT, with the backup checkbox beside
        // it, and patronage a separate option that then asks the direction.
        para("A meeting establishes a presence record with the other person, in person. What this meeting is for is chosen now and cannot change once it begins.")
        heading("Regular meeting")
        val backup = CheckBox(this).apply {
            text = "Ask this person to backup my user data"
            setTextColor(Color.DKGRAY)
        }
        body.addView(backup)
        button("Show my code") {
            start(Meet.Kind(Meet.Adopt.NONE, backup.isChecked), Meet.Role.INITIATOR)
        }
        heading("Patronage")
        para("An adoption moves authority. What it costs is listed before either of you agrees.")
        button("I will be the Patron") {
            start(Meet.Kind(Meet.Adopt.THEM_UNDER_ME), Meet.Role.INITIATOR)
        }
        button("I will be the Client") {
            start(Meet.Kind(Meet.Adopt.ME_UNDER_THEM), Meet.Role.INITIATOR)
        }
        heading("Or join theirs")
        para("The other person has shown you a code. Their choice of transaction is in it, and the next screen is where you accept or refuse it.")
        button("Scan a QR code") { start(Meet.Kind(), Meet.Role.RESPONDER) }
    }

    private fun start(kind: Meet.Kind, role: Meet.Role) {
        Kernel.startMeet(kind, role)?.bind(sink) ?: para("could not begin: unprovisioned")
        redraw()
    }

    // ---- D1 intent -----------------------------------------------------

    private fun intent(m: Meet) {
        when (m.role) {
            Meet.Role.INITIATOR -> {
                para("Hold this up for ${m.counterpartyName} to scan with their REAR camera. It carries who you are and what kind of meeting this is — nothing of the meeting's own anchor, which comes later and goes both ways.")
                val code = Kernel.bootstrap()
                if (code == null) {
                    para("The code needs the kernel's identifier, which this device has not got yet.")
                } else {
                    qr(code, "bootstrap")
                }
                para("When they have scanned it, both of you will be shown what is about to happen.")
                button("They have scanned it") { m.crossBootstrap() }
            }
            Meet.Role.RESPONDER -> {
                para("Point the back of your phone at ${m.counterpartyName}'s screen. Their code carries who they are and the kind of meeting they chose; the next screen is where you accept or refuse it.")
                scan(QrCamera.Facing.REAR, "bootstrap") { bytes ->
                    // the bootstrap is the shell's own object and carries no
                    // anchor, so the shell reads it (`wire-format.md` §14.3
                    // fixes the ANCHORED objects and this is not one)
                    val read = Kernel.takeBootstrap(bytes)
                    runOnUiThread {
                        if (read == null) m.crossBootstrap() else m.stop(read)
                    }
                }
            }
        }
    }

    /** A QR on the screen, large enough to scan across a table. */
    private fun qr(bytes: ByteArray, which: String) {
        val m = Optical.matrix(bytes)
        Diag.event("qr.shown", "which" to which, "bytes" to bytes.size, "modules" to m.width)
        val scale = 8
        val w = m.width * scale
        val px = IntArray(w * w)
        for (y in 0 until w) {
            for (x in 0 until w) {
                px[y * w + x] = if (m.get(x / scale, y / scale)) Color.BLACK else Color.WHITE
            }
        }
        body.addView(
            ImageView(this).apply {
                setImageBitmap(Bitmap.createBitmap(px, w, w, Bitmap.Config.ARGB_8888))
                layoutParams = LinearLayout.LayoutParams(w, w).apply { topMargin = 24 }
            },
        )
    }

    /**
     * Read one QR with the named camera. The permission is asked for here
     * and a refusal stops the ceremony with a reason rather than silently:
     * a camera this shell does not hold is a meeting it cannot carry.
     */
    private fun scan(facing: QrCamera.Facing, which: String, found: (ByteArray) -> Unit) {
        if (!held(Manifest.permission.CAMERA)) {
            para("This step needs the camera. Nothing is read until you allow it.")
            button("Allow the camera") { ask(1, Manifest.permission.CAMERA) }
            return
        }
        para("Scanning…")
        val cam = camera ?: QrCamera(this).also { camera = it }
        cam.readOne(facing, null, which) { bytes -> found(bytes) }?.let { why -> para(why) }
    }

    // ---- D1.5 the brief: everything front-loaded -----------------------

    private fun brief(m: Meet) {
        para("This is what is about to happen. Both of you are being shown it, and both of you answer.")
        heading(
            when (m.adopt) {
                Meet.Adopt.NONE -> "A regular meeting with ${m.counterpartyName}"
                Meet.Adopt.ME_UNDER_THEM -> "${m.counterpartyName} as your patron"
                Meet.Adopt.THEM_UNDER_ME -> "You as ${m.counterpartyName}'s patron"
            },
        )
        // every item the model says this ceremony earns, and no reassurance
        // it has not earned
        m.brief().forEach { para("• ${it.text}") }
        heading("What may be weak in this meeting")
        para("• Nominated witnesses: none, until your horizon can offer them. The record carries that it had none.")
        para("• Verifiers and proximity are weighed, not required; a thin meeting is honest, not malformed.")
        if (bluetooth.any { !held(it) }) {
            // the bearer that carries the intent is Bluetooth LE; without
            // the permissions the exchange stops at the radio, said here
            // where the person can still act rather than caught there
            para("The intent crosses over Bluetooth, which this app is not yet allowed to use. Without it the meeting will stop at the exchange.")
            button("Allow Bluetooth") { ask(3, *bluetooth) }
        }
        button("Accept — begin") { m.accept() }
        button("Refuse") { m.refuse() }
    }

    // ---- D2 the optical exchange, mutual and on the selfie cameras -----

    /**
     * **Turn the phone to face the counterparty** [author, 2026-09-29].
     * This is the moment the device stops being its user's: from here it
     * shows its code to the other person and reads theirs, and takes no
     * input until the capture is done.
     */
    private fun optical(m: Meet) {
        para("↻  TURN YOUR PHONE AROUND so the screen faces ${m.counterpartyName}, and let them do the same. Each phone reads the other's code with its SELFIE camera.")
        para("Two codes cross, in order: each phone's contribution, then the meeting id both compute from the pair. If the two ids differ, something is between you and the meeting stops — that check is the whole of what looking at each other's screen buys.")
        val code = Kernel.optical()
        if (code == null) {
            para("The code needs an open ceremony, which this device has lost.")
            return
        }
        val which = if (Kernel.opticalTaken()) "transcript" else "contribution"
        qr(code, which)
        scan(QrCamera.Facing.SELFIE, which) { bytes ->
            val why = Kernel.takeOptical(bytes)
            runOnUiThread { if (why != null) m.stop(why) else redraw() }
        }
    }

    // ---- D3 proximity: the tap, and the optical pass D2 already made ---

    /**
     * The kernel runs the channel ladder (design §7.6.3) off the UI thread
     * and the screen follows. **The phone still faces the counterparty**,
     * so there is nothing to tap here: the NFC tap is two phones touching,
     * and the optical channel already passed when the two screens agreed at
     * D2. The strongest that passed is what the record carries.
     */
    private fun proximity(m: Meet) {
        para("Ranging with ${m.counterpartyName}. Hold the phones together for the tap; the screen-to-screen check at the last step is itself the weakest channel, so this cannot come back with nothing.")
        para("UWB is not among the channels this build can run, so the strongest here is a tap. The record carries the strongest that passed and no more — a weaker channel is never shown as a stronger one.")
        once(m, "proximity") {
            Kernel.runProximity(m)
        }
    }

    // ---- D4 capture: the counterparty's face ---------------------------

    /**
     * The guided capture runs on the kernel's thread, one frame per prompt,
     * on the selfie camera the phone is already pointing at the
     * counterparty. The disclosures were read at D1.5 and the device faces
     * away, so this shows a progress line and takes nothing.
     */
    private fun capture(m: Meet) {
        para("Capturing ${m.counterpartyName}: a few frames over a few seconds, with spoken or toned prompts for them. Nothing is shown to you and nothing is asked — you read what this holds and who may see it before the phone turned around.")
        if (!held(Manifest.permission.CAMERA)) {
            para("The capture needs the camera, which is not yet allowed.")
            button("Allow the camera") { ask(2, Manifest.permission.CAMERA) }
            return
        }
        once(m, "capture") {
            Kernel.runCapture(m)
            Kernel.selectVerifiers()
        }
    }

    /**
     * Run `work` once as the flow enters a hands-off step, off the UI
     * thread, and never again on a redraw. A recreated screen rejoins a
     * step already running rather than starting it twice — the same
     * discipline the verifier selection uses.
     */
    private fun once(m: Meet, tag: String, work: () -> Unit) {
        para("Working…")
        if (started.add("${m.counterpartyKey}:$tag")) {
            Thread { work() }.start()
        }
    }

    // ---- D5 verifiers --------------------------------------------------

    private fun verifiers(m: Meet) {
        para("Your device picks ${m.counterpartyName}'s verifiers from the records they handed over, preferring people you have met. The choice is computed, not offered: there is nothing here to pick.")
        if (!m.selectionRun()) {
            // the selection is asked for once, when the flow enters this
            // step, and never from inside a draw
            para("Selecting…")
            return
        }
        val chosen = m.chosen()
        heading("Selected")
        if (chosen.isEmpty()) {
            para("• None required. ${m.counterpartyName} handed over no records, so there is no pool to draw from and none is owed — a meeting with fewer verifiers is thinner, not malformed.")
        } else {
            chosen.forEach { c ->
                val answer = when (c.verdict) {
                    null -> "awaiting an answer"
                    Meet.Verdict.MATCH -> "answered: a match"
                    Meet.Verdict.NO_MATCH -> "answered: no match"
                    Meet.Verdict.INCONCLUSIVE -> "answered: inconclusive"
                    Meet.Verdict.UNAVAILABLE -> "unavailable — silence counts for nothing either way"
                }
                para("• ${c.key.take(16)}… — ${basisWords(c.basis)}; $answer")
            }
            para("Each query waits on the counterparty's consent, which crosses the same local bearer the intent did. Once consented, the kernel carries the query to its verifier over the network and the answer lands above.")
        }
        val mine = m.queriesAboutMe()
        if (mine.isNotEmpty()) {
            heading("Queries about you")
            para("${m.counterpartyName} asked these verifiers about you. Your device consented on each — consent is bound to the ceremony you are standing in, so it is given without stopping to ask, and you are told instead.")
            mine.forEach { para("• $it…") }
        }
        button("Continue to review") { m.verifiersDone() }
    }

    /** `wire-format.md` §5.5's basis, in words, and each says whose claim
     *  it is: nobody audits a selector's tier. */
    private fun basisWords(b: Meet.Basis): String = when (b) {
        Meet.Basis.MET -> "you have met them"
        Meet.Basis.IN_HORIZON -> "in your horizon"
        Meet.Basis.REACHABLE -> "one edge beyond it"
        Meet.Basis.DISCRETIONARY -> "a stranger, taken at your discretion"
    }

    // ---- D6 review and sign --------------------------------------------

    private fun review(m: Meet) {
        para("You review ${m.counterpartyName}'s selection of your verifiers before signing — a party who signs unseen may be vouching for strangers.")
        val warnings = m.presign()
        if (warnings.isEmpty()) {
            heading("Nothing to flag")
            para("Every check this screen makes came back unremarkable. That is not a guarantee about the person in front of you; it is the absence of the specific weaknesses listed at D6.")
        } else {
            heading("What is thin about this meeting")
            warnings.forEach { para("• ${it.text}") }
            // UX-003's whole point, said where the person reads it
            para("None of the above makes the record broken. Each is something a reader of the record can see for themselves, and a meeting that says less is still a meeting that happened.")
        }
        para("Signing needs the artifacts the steps above would have produced; this build has none, so it cannot finalize a real record.")
        walkthrough("signed (no real record)") { m.signed("(walkthrough, no record)") }
    }

    // ---- D7 after ------------------------------------------------------

    private fun done(m: Meet) {
        para("The record: ${m.recordTxid()}")
        when {
            // PRD-05: two people who each chose to be the patron have not
            // hit a protocol failure, and the screen must not say they have
            m.opposedAdoptions() && m.direction() == null -> {
                heading("You both offered to be the patron")
                para("Each of you chose a direction before seeing the other's, so this is two intentions rather than a fault — nothing malformed has happened, and the meeting itself stands whichever way this goes.")
                para("Choose a direction between you, or leave the authority question alone.")
                button("I will be the patron") { m.chooseDirection(Meet.Adopt.THEM_UNDER_ME) }
                button("${m.counterpartyName} will be the patron") {
                    m.chooseDirection(Meet.Adopt.ME_UNDER_THEM)
                }
                button("Neither — just the meeting") { m.chooseDirection(Meet.Adopt.NONE) }
                return
            }
            m.direction() == Meet.Adopt.NONE ->
                para("You left the authority question alone. The meeting stands on its own, which costs neither of you anything you had.")
            m.direction() != null || m.adopt != Meet.Adopt.NONE ->
                para("The adoption would now be proposed and taken in the direction settled here.")
        }
        button("Done") { Kernel.stopMeet("done"); finish() }
    }

    private fun stopped(m: Meet) {
        para("Stopped: ${m.stopReason()}")
        button("Back") { Kernel.stopMeet("dismissed"); finish() }
    }

    // ---- view helpers --------------------------------------------------

    private fun title(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 24f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 0, 0, 24)
        },
    )

    private fun heading(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 16f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 20, 0, 8)
        },
    )

    private fun para(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 15f
            setPadding(0, 8, 0, 8)
        },
    )

    private fun button(label: String, onClick: () -> Unit) = body.addView(
        Button(this).apply {
            text = label
            setPadding(0, 16, 0, 16)
            setOnClickListener { onClick() }
        },
    )

    /** A scaffold control, not the ceremony: it stands in for a signal a
     *  real device would raise, and is dim and labelled so. */
    private fun walkthrough(label: String, onClick: () -> Unit) = body.addView(
        TextView(this).apply {
            text = "▸ walkthrough: $label"
            textSize = 13f
            setTextColor(Color.GRAY)
            gravity = Gravity.END
            setPadding(0, 28, 0, 12)
            isClickable = true
            setOnClickListener { onClick() }
        },
    )

    private fun logLines(m: Meet) {
        m.log().forEach { line ->
            body.addView(
                TextView(this).apply {
                    text = line
                    textSize = 12f
                    typeface = Typeface.MONOSPACE
                    setTextColor(Color.GRAY)
                    setPadding(0, 2, 0, 2)
                },
            )
        }
        scroll.post { scroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }
}
