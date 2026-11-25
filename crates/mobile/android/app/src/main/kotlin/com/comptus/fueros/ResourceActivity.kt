package com.comptus.fueros

import android.annotation.SuppressLint
import android.app.Activity
import android.os.Bundle
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **A resource's page, in a frame that reaches nothing** (the screens
 * sheet's F; `light-client-requirements.md` §8, PRD-12's argument one
 * step on).
 *
 * The gap this closes: a resource's answer was read as the first line of
 * its raw HTTP message on a diagnostic screen, so the catalog could ask a
 * resource for something and nobody could look at what came back. What a
 * page may do was written down as owed.
 *
 * **[ResourcePage] holds the policy and the reasons**; this is the frame
 * that enforces it [author, 2026-11-15]. In one place, because a second
 * screen deciding what a page may do is how the two come apart:
 *
 *  - **An origin of its own** ([Frames.origin]): the page is loaded at
 *    that origin's URL and served from the kernel's answer, so the engine
 *    enforces the page's own policy and the document is same-origin with
 *    its resource and nothing else.
 *  - **The same resource, carried**: the frame's engine has one proxy
 *    for everything, this app's own ([Carrier], [Engine]), which answers
 *    the origin from the kernel and refuses everything else; the frame
 *    goes inside the origin and is refused anywhere else, saying so.
 *  - **Script by declaration and consent** ([ResourcePage.runs]): the
 *    frame is made with script on only where the page declared it and
 *    the person allowed it, asked once per resource.
 *  - **One object** (`rhtn.request`, [Engine]) for a page that runs, carrying a
 *    request with a body to the same resource; no other bridge.
 *  - **No local reach**: file and content access off, both file-URL
 *    escapes off.
 *  - **Storage of the resource's own** under its origin, kept across
 *    screens and wiped when the person forgets the resource.
 *
 * **Text takes no frame at all.** `text/plain` is not a page, so it is
 * drawn in a `TextView` and no web view is created — the policy's own
 * rule, which is also the cheaper path.
 */
class ResourceActivity : Activity() {

    /** The frame's session in the engine, closed with the screen. */
    private var session: org.mozilla.geckoview.GeckoSession? = null

    /** The identity that holds the entry this screen was opened from. */
    private var by: String? = null

    /** Ask as that identity, or as the one that acts where none came. */
    private fun askAsHolder(node: String, resource: String, path: String): Gateway.Answered? {
        val holder = by
        return if (holder == null) {
            Gateway.ask(node, resource, path)
        } else {
            Gateway.askAs(holder, node, resource, path)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val node = intent.getStringExtra("node").orEmpty()
        val resource = intent.getStringExtra("resource").orEmpty()
        val name = intent.getStringExtra("name").orEmpty()
        val path = intent.getStringExtra("path") ?: "/"
        // **whose entry brought us here** [author, 2026-11-07]: a resource
        // is reached inside a subnet, so the identity holding its catalog
        // entry is the one that asks ([Identities.Owned]). Absent where
        // this screen was opened from a pasted keyhash with no entry, and
        // then the identity that acts asks.
        by = intent.getStringExtra("by")
        // **whose entry brought us here** [author, 2026-11-07]: a resource
        // is reached inside a subnet, so the identity that holds its
        // catalog entry is the one that asks ([Identities.Owned]). Absent
        // where the screen was opened from a pasted keyhash with no entry,
        // and then the identity that acts asks.

        val title = TextView(this).apply {
            textSize = 19f
            typeface = Type.bold
            text = name.ifBlank { "A resource" }
            setPadding(Screen.gutter(context), 20, Screen.gutter(context), 2)
        }
        val from = TextView(this).apply {
            textSize = 12f
            typeface = Type.mono
            setTextColor(Palette.now.sub)
            text = ResourcePage.provenance(resource, node)
            setPadding(Screen.gutter(context), 0, Screen.gutter(context), 6)
        }
        val says = TextView(this).apply {
            textSize = 12f
            setTextColor(Palette.now.sub)
            text = ResourcePage.frameSays(false)
            setPadding(Screen.gutter(context), 0, Screen.gutter(context), 10)
        }
        this.says = says
        val held = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
            ).apply { weight = 1f }
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(title)
            addView(from)
            addView(says)
            addView(held)
        }
        Screen.insetting(root)
        setContentView(root)

        this.held = held
        held.addView(note("asking…"))
        // **off the drawing thread**: asking a resource is a round trip
        // through the hosting node
        Thread({ show(askAsHolder(node, resource, path)) }, "fueros-resource").start()
    }

    private var held: LinearLayout? = null
    /** What the frame says of itself, under the provenance line. */
    private var says: TextView? = null
    /** The first page, served to the frame's first load of its origin. */
    private var first: Pair<String, Gateway.Page>? = null
    private var camera: QrCamera? = null
    private var still: Still? = null
    private var reading: android.nfc.NfcAdapter? = null
    private var cancelling: android.os.CancellationSignal? = null
    /** The ask a permission request is for, held across the dialogue. */
    private var waiting: Pair<Gateway.Ask, Giving.Plan>? = null
    /** The ask a file picker is for, held across the other app. */
    private var pending: Gateway.Ask? = null
    /** What a save is about to write, held across the picker. */
    private var saving: ByteArray? = null
    /** What the resource last answered with, for the save control. */
    private var last: Gateway.Page? = null

    private companion object {
        const val PERMISSION = 71
        const val PICK = 72
        const val SAVE = 73
        const val ENGINE = 74
    }

    /** Draw what the resource answered, from any thread. */
    private fun show(answered: Gateway.Answered?) {
        runOnUiThread {
            if (isFinishing) return@runOnUiThread
            val held = held ?: return@runOnUiThread
            last = answered?.page
            // whatever was framed goes with what it showed, a live
            // session's connection included
            unframe()
            held.removeAllViews()
            val showing = ResourcePage.showing(answered)
            // a live session the person approved is for the page that
            // answers the approval, and for nothing else
            if (showing !is ResourcePage.Showing.Page) liveFor = null
            when (showing) {
                is ResourcePage.Showing.Page -> {
                    val resource = intent.getStringExtra("resource").orEmpty()
                    when (ResourcePage.runs(showing.page, Frames.scriptAllowed(this, resource))) {
                        null -> held.addView(consenting(showing))
                        true -> held.addView(frame(showing.html, showing.page, allowed = true))
                        false -> held.addView(frame(showing.html, showing.page, allowed = false))
                    }
                }
                is ResourcePage.Showing.Text -> held.addView(text(showing.text))
                is ResourcePage.Showing.Said -> {
                    held.addView(note(showing.why))
                    // **what cannot be shown can still be kept**: a file a
                    // resource answered with — an `export` the person
                    // agreed to, among others — goes where they choose
                    last?.takeIf { it.body.isNotEmpty() }?.let { held.addView(saveable(it)) }
                }
                is ResourcePage.Showing.Ask -> {
                    // **and in the shade, where the resource holds the
                    // standing permission** ([Notify]): the person may
                    // have left the screen, and what waits is said once
                    Notify.post(this, showing.ask)
                    held.addView(asking(showing.ask))
                }
            }
        }
    }

    /**
     * **An ask, put to the person** (`light-client-requirements.md` §8.1).
     * The question is the kernel's reading in words; the answer goes back
     * through the kernel, which composes it. **Nothing is given that the
     * person did not press a button for**, and a utility this device
     * cannot give is declined **with that as the reason** — the
     * application sees a refusal, never a silence.
     *
     * [Giving] decides which of the two it is, and what the person is
     * told while it happens; this screen only draws it and drives the
     * capability.
     */
    private fun asking(ask: Gateway.Ask): LinearLayout {
        val plan = Giving.plan(ask, facts())
        Diag.event(
            "utility",
            "which" to ask.utility,
            "state" to "asked",
            "way" to plan.way.name.lowercase(),
        )
        val view = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        view.addView(words(ResourcePage.asking(ask)))
        val unable = plan.way == Giving.Way.NONE
        view.addView(note(if (unable) "This app cannot: ${plan.says}." else plan.says))
        val buttons = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(Screen.gutter(context), 0, Screen.gutter(context), 16)
        }
        if (!unable) {
            buttons.addView(button(offer(plan.way)) { give(ask, plan) })
        }
        buttons.addView(
            button(if (unable) "Tell it no, with that reason" else "Decline") {
                answer(ask, declined(if (unable) plan.says else "declined"))
            },
        )
        view.addView(buttons)
        return view
    }

    /** What the button that gives says. */
    private fun offer(way: Giving.Way): String = when (way) {
        Giving.Way.PHOTOGRAPH -> "Open the camera"
        Giving.Way.PICK -> "Pick a file"
        Giving.Way.PLACE -> "Find where I am"
        Giving.Way.TAG_QR -> "Read the code"
        Giving.Way.TAG_NFC -> "Read the tag"
        Giving.Way.UNLOCK -> "Read it and sign"
        Giving.Way.NOTIFY -> "Allow"
        Giving.Way.RECEIVE -> "Take the file"
        Giving.Way.LIVE -> "Start the live session"
        Giving.Way.NONE -> "Decline"
    }

    /** What this device has, which is half of what [Giving] decides on. */
    private fun facts(): Giving.Facts = Giving.Facts(
        camera = packageManager.hasSystemFeature(android.content.pm.PackageManager.FEATURE_CAMERA_ANY),
        nfc = runCatching {
            android.nfc.NfcAdapter.getDefaultAdapter(this)?.isEnabled == true
        }.getOrDefault(false),
        unlock = runCatching {
            val m = getSystemService(android.hardware.biometrics.BiometricManager::class.java)
            m?.canAuthenticate(AUTHENTICATORS) ==
                android.hardware.biometrics.BiometricManager.BIOMETRIC_SUCCESS
        }.getOrDefault(false),
        notifyNeedsPermission = android.os.Build.VERSION.SDK_INT >= 33,
    )

    /** **A weak biometric or the device's own credential**: the platform
     *  vouching the holder to the device, which is what authorises an
     *  application signature and is not a claim about the person to
     *  anybody else (§8.1's three things kept apart). */
    private val AUTHENTICATORS =
        android.hardware.biometrics.BiometricManager.Authenticators.BIOMETRIC_WEAK or
            android.hardware.biometrics.BiometricManager.Authenticators.DEVICE_CREDENTIAL

    /** Give what the ask wants, once what it needs is held. */
    private fun give(ask: Gateway.Ask, plan: Giving.Plan) {
        val missing = plan.permissions.filterNot { held(it) }
        if (missing.isNotEmpty()) {
            waiting = Pair(ask, plan)
            for (p in missing) {
                Diag.event("permission", "which" to short(p), "state" to "requested")
            }
            requestPermissions(missing.toTypedArray(), PERMISSION)
            return
        }
        proceed(ask, plan)
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == ENGINE) {
            val asked = engineAsked
            engineAsked = null
            val all = grantResults.isNotEmpty() &&
                grantResults.all { it == android.content.pm.PackageManager.PERMISSION_GRANTED }
            if (all) asked?.grant() else asked?.reject()
            return
        }
        val (ask, plan) = waiting ?: return
        waiting = null
        val refused = permissions.indices.filter {
            grantResults.getOrNull(it) != android.content.pm.PackageManager.PERMISSION_GRANTED
        }
        for (i in permissions.indices) {
            Diag.event(
                "permission",
                "which" to short(permissions[i]),
                "state" to if (refused.contains(i)) "denied" else "granted",
            )
        }
        // **a permission refused is the person's answer**, and the
        // resource is told what it was rather than left waiting
        if (refused.isEmpty()) {
            proceed(ask, plan)
        } else {
            answer(
                ask,
                declined(
                    "this device does not allow the app " +
                        refused.joinToString(" or ") { short(permissions[it]) },
                ),
            )
        }
    }

    private fun proceed(ask: Gateway.Ask, plan: Giving.Plan) {
        Diag.event("utility", "which" to ask.utility, "state" to "giving")
        when (plan.way) {
            Giving.Way.PHOTOGRAPH -> photographing(ask, plan)
            Giving.Way.PICK -> picking(ask)
            Giving.Way.PLACE -> placing(ask)
            Giving.Way.TAG_QR -> readingCode(ask, plan)
            Giving.Way.TAG_NFC -> readingTag(ask, plan)
            Giving.Way.UNLOCK -> unlocking(ask, plan)
            Giving.Way.NOTIFY -> allowing(ask)
            // **the person agrees to be handed the file**; what comes
            // back next carries it, and the answer below saves it
            Giving.Way.RECEIVE -> answer(ask, uniffi.rhtn_ffi.UtilityOutcome.Approved)
            // **the person agrees to a live session**; the page that
            // comes back next is the one framed with a peer connection,
            // for the seconds asked ([frame])
            Giving.Way.LIVE -> {
                liveFor = ask.seconds?.toLong()
                answer(ask, uniffi.rhtn_ffi.UtilityOutcome.Approved)
            }
            Giving.Way.NONE -> answer(ask, declined(plan.says))
        }
    }

    // ---- a live session -----------------------------------------------

    /** The seconds of a live session the person approved, until the page
     *  it is for is framed; nothing where none is waiting. */
    private var liveFor: Long? = null

    /** When the running live session ends, by the device's own clock that
     *  does not jump; 0 where none runs. */
    private var liveUntil: Long = 0

    private val clock = android.os.Handler(android.os.Looper.getMainLooper())

    /** What the engine is waiting on while the platform asks the person. */
    private var engineAsked: org.mozilla.geckoview.GeckoSession.PermissionDelegate.Callback? = null

    /** **Say how long is left, and end it when nothing is**: the
     *  indicator a live session carries (§8.1). */
    private fun counting() {
        if (liveUntil == 0L) return
        val left = (liveUntil - android.os.SystemClock.elapsedRealtime() + 999) / 1000
        if (left <= 0) {
            endLive()
            return
        }
        says?.text = "LIVE, ${Giving.lasting(left)} left: this page is connected past this app, " +
            "to where its package streams. Leave the page to end it now."
        says?.setTextColor(Palette.now.ink)
        clock.postDelayed({ counting() }, 1000)
    }

    /** **One frame to a screen**: the one in place is shut with whatever
     *  it had. */
    private fun unframe() {
        endLive()
        session?.let { Engine.closed(it); it.close() }
        session = null
    }

    /** **End the live session**: pages are given no peer connection again,
     *  and the page that had one is loaded afresh, which is what takes its
     *  connection away. */
    private fun endLive() {
        if (liveUntil == 0L) return
        liveUntil = 0
        clock.removeCallbacksAndMessages(null)
        Diag.event("live", "state" to "ended")
        says?.setTextColor(Palette.now.sub)
        says?.text = "The live session has ended. " + ResourcePage.frameSays(true)
        val had = session
        Engine.peers(false) { if (had != null && had === session && had.isOpen) had.reload() }
    }

    // ---- a photograph -------------------------------------------------

    /** The camera, with the preview the person frames the shot in. */
    private fun photographing(ask: Gateway.Ask, plan: Giving.Plan) {
        val h = held ?: return
        h.removeAllViews()
        h.addView(note(plan.says))
        val preview = android.view.SurfaceView(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
            ).apply { weight = 1f }
        }
        val cam = still ?: Still(this).also { still = it }
        preview.holder.addCallback(
            object : android.view.SurfaceHolder.Callback {
                override fun surfaceCreated(holder: android.view.SurfaceHolder) {
                    cam.start(holder.surface) { why -> said(ask, why) }
                        ?.let { why -> said(ask, why) }
                }

                override fun surfaceChanged(h: android.view.SurfaceHolder, f: Int, w: Int, y: Int) = Unit

                override fun surfaceDestroyed(holder: android.view.SurfaceHolder) {
                    cam.close()
                }
            },
        )
        h.addView(preview)
        val buttons = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(Screen.gutter(context), 8, Screen.gutter(context), 16)
        }
        buttons.addView(
            button("Take it") {
                cam.take(rotation()) { bytes ->
                    runOnUiThread { offering(ask, bytes) }
                }?.let { why -> said(ask, why) }
            },
        )
        buttons.addView(button("Decline") { cam.close(); answer(ask, declined("declined")) })
        h.addView(buttons)
    }

    /** **The photograph is sent only when the person sends it**: what was
     *  taken, how large it is, and the three ways on from there. */
    private fun offering(ask: Gateway.Ask, bytes: ByteArray) {
        val h = held ?: return
        still?.close()
        if (bytes.size > Giving.RESULT_CAP) {
            answer(ask, declined("the photograph this device took is larger than the app can send"))
            return
        }
        h.removeAllViews()
        h.addView(note("A photograph of ${bytes.size} bytes. Nothing has been sent."))
        h.addView(
            android.widget.ImageView(this).apply {
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    0,
                ).apply { weight = 1f }
                adjustViewBounds = true
                runCatching {
                    setImageBitmap(
                        android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size),
                    )
                }
            },
        )
        val buttons = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(Screen.gutter(context), 8, Screen.gutter(context), 16)
        }
        buttons.addView(
            button("Send it") {
                answer(
                    ask,
                    uniffi.rhtn_ffi.UtilityOutcome.Bytes("image/jpeg", bytes),
                )
            },
        )
        buttons.addView(button("Take another") { give(ask, Giving.plan(ask, facts())) })
        buttons.addView(button("Decline") { answer(ask, declined("declined")) })
        h.addView(buttons)
    }

    /** The display's own rotation in degrees, so the JPEG stands the way
     *  the person held the phone. */
    private fun rotation(): Int = when (display?.rotation) {
        android.view.Surface.ROTATION_90 -> 90
        android.view.Surface.ROTATION_180 -> 180
        android.view.Surface.ROTATION_270 -> 270
        else -> 0
    }

    // ---- a file, either way -------------------------------------------

    /** A file the person picks, handed over as its own declared type. */
    private fun picking(ask: Gateway.Ask) {
        pending = ask
        val types = Giving.types(ask)
        val intent = android.content.Intent(android.content.Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(android.content.Intent.CATEGORY_OPENABLE)
            .setType(types.first())
            .putExtra(android.content.Intent.EXTRA_MIME_TYPES, types)
        runCatching {
            @Suppress("DEPRECATION")
            startActivityForResult(intent, PICK)
        }.onFailure { answer(ask, declined("this device offers no file picker")) }
    }

    @Suppress("DEPRECATION")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: android.content.Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data
        when (requestCode) {
            PICK -> {
                val ask = pending ?: return
                pending = null
                if (resultCode != RESULT_OK || uri == null) {
                    answer(ask, declined("no file was picked"))
                    return
                }
                val read = runCatching {
                    contentResolver.openInputStream(uri)?.use { it.readBytes() }
                }.getOrNull()
                when {
                    read == null -> answer(ask, declined("this device would not open that file"))
                    read.size > Giving.RESULT_CAP ->
                        answer(ask, declined("that file is larger than the app can send"))
                    else -> answer(
                        ask,
                        uniffi.rhtn_ffi.UtilityOutcome.Bytes(
                            contentResolver.getType(uri) ?: "application/octet-stream",
                            read,
                        ),
                    )
                }
            }
            // the file the resource handed over, going where the person
            // chose to put it
            SAVE -> {
                val bytes = saving
                saving = null
                if (resultCode != RESULT_OK || uri == null || bytes == null) return
                val wrote = runCatching {
                    contentResolver.openOutputStream(uri)?.use { it.write(bytes) }
                }.isSuccess
                android.widget.Toast.makeText(
                    this,
                    if (wrote) "Saved." else "This device would not write there.",
                    android.widget.Toast.LENGTH_SHORT,
                ).show()
            }
        }
    }

    /** Offer to save what a resource answered with, which is how an
     *  `export` the person agreed to reaches them. */
    private fun saveable(page: Gateway.Page): LinearLayout {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(Screen.gutter(context), 0, Screen.gutter(context), 16)
        }
        row.addView(
            button("Save to a file") {
                saving = page.body
                val intent = android.content.Intent(android.content.Intent.ACTION_CREATE_DOCUMENT)
                    .addCategory(android.content.Intent.CATEGORY_OPENABLE)
                    .setType(page.media ?: "application/octet-stream")
                    .putExtra(android.content.Intent.EXTRA_TITLE, "resource-file")
                runCatching {
                    @Suppress("DEPRECATION")
                    startActivityForResult(intent, SAVE)
                }.onFailure {
                    android.widget.Toast.makeText(
                        this,
                        "This device offers nowhere to save it.",
                        android.widget.Toast.LENGTH_SHORT,
                    ).show()
                }
            },
        )
        return row
    }

    // ---- a place ------------------------------------------------------

    /** Where this device is, as a geohash at the precision asked, shown
     *  before anything is sent. */
    private fun placing(ask: Gateway.Ask) {
        val h = held ?: return
        h.removeAllViews()
        h.addView(note("Finding where this device is…"))
        val lm = getSystemService(android.location.LocationManager::class.java)
        val provider = listOf(
            android.location.LocationManager.FUSED_PROVIDER,
            android.location.LocationManager.NETWORK_PROVIDER,
            android.location.LocationManager.GPS_PROVIDER,
        ).firstOrNull { runCatching { lm?.isProviderEnabled(it) == true }.getOrDefault(false) }
        if (lm == null || provider == null) {
            answer(ask, declined("this device has no location service turned on"))
            return
        }
        try {
            lm.getCurrentLocation(provider, null, mainExecutor) { where ->
                runOnUiThread {
                    if (where == null) {
                        answer(ask, declined("this device could not find where it is"))
                    } else {
                        placed(ask, where)
                    }
                }
            }
        } catch (e: SecurityException) {
            // asked for before this screen; a permission taken back since
            // is a refusal, said as one
            answer(ask, declined("the location permission is not held"))
        } catch (e: Exception) {
            answer(ask, declined("this device would not give its place: $e"))
        }
    }

    private fun placed(ask: Gateway.Ask, where: android.location.Location) {
        val h = held ?: return
        val hash = Giving.geohash(where.latitude, where.longitude, ask.precision ?: 5)
        h.removeAllViews()
        h.addView(
            words(
                Giving.placeSays(hash, if (where.hasAccuracy()) where.accuracy else null) +
                    "\n\nNothing has been sent, and nothing of where you have been is kept.",
            ),
        )
        val buttons = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(Screen.gutter(context), 0, Screen.gutter(context), 16)
        }
        buttons.addView(
            button("Send it") { answer(ask, uniffi.rhtn_ffi.UtilityOutcome.Text(hash)) },
        )
        buttons.addView(button("Decline") { answer(ask, declined("declined")) })
        h.addView(buttons)
    }

    // ---- a tag, read two ways -----------------------------------------

    private fun readingCode(ask: Gateway.Ask, plan: Giving.Plan) {
        val h = held ?: return
        h.removeAllViews()
        h.addView(note(plan.says))
        val cam = camera ?: QrCamera(this).also { camera = it }
        cam.readOne(
            QrCamera.Facing.REAR,
            null,
            "utility-tag",
            failed = { why -> said(ask, why) },
        ) { bytes ->
            cam.close()
            runOnUiThread {
                answer(ask, uniffi.rhtn_ffi.UtilityOutcome.Text(String(bytes).trim()))
            }
        }?.let { why -> said(ask, why) }
        h.addView(
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                setPadding(Screen.gutter(context), 8, Screen.gutter(context), 16)
                addView(button("Decline") { cam.close(); answer(ask, declined("declined")) })
            },
        )
    }

    /** **The near-field radio, in reader mode**: the tag's own text where
     *  it carries one, and its identifier where it does not. Nothing is
     *  written to a tag, ever. */
    private fun readingTag(ask: Gateway.Ask, plan: Giving.Plan) {
        val h = held ?: return
        val adapter = android.nfc.NfcAdapter.getDefaultAdapter(this)
        if (adapter == null) {
            answer(ask, declined("this device has no near-field radio"))
            return
        }
        h.removeAllViews()
        h.addView(note(plan.says))
        h.addView(
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                setPadding(Screen.gutter(context), 8, Screen.gutter(context), 16)
                addView(button("Decline") { stopReading(); answer(ask, declined("declined")) })
            },
        )
        reading = adapter
        runCatching {
            adapter.enableReaderMode(
                this,
                { tag -> runOnUiThread { tagRead(ask, tag) } },
                android.nfc.NfcAdapter.FLAG_READER_NFC_A or
                    android.nfc.NfcAdapter.FLAG_READER_NFC_B or
                    android.nfc.NfcAdapter.FLAG_READER_NFC_F or
                    android.nfc.NfcAdapter.FLAG_READER_NFC_V or
                    // the platform's own chirp would be a second event the
                    // person reads as the app having done something
                    android.nfc.NfcAdapter.FLAG_READER_NO_PLATFORM_SOUNDS,
                null,
            )
        }.onFailure { answer(ask, declined("this device would not start the radio: $it")) }
    }

    private fun tagRead(ask: Gateway.Ask, tag: android.nfc.Tag) {
        stopReading()
        val said = runCatching {
            val ndef = android.nfc.tech.Ndef.get(tag)
            val record = ndef?.cachedNdefMessage?.records?.firstOrNull()
            record?.let { Giving.ndefText(it.tnf, it.type, it.payload) }
        }.getOrNull()
        val text = said ?: Giving.tagId(tag.id)
        answer(ask, uniffi.rhtn_ffi.UtilityOutcome.Text(text))
    }

    private fun stopReading() {
        runCatching { reading?.disableReaderMode(this) }
        reading = null
    }

    // ---- a signature, and a standing permission -----------------------

    /** **The exact payload, then the platform's unlock.** The kernel
     *  signs; this only shows what is being signed and gets the person's
     *  authorisation for it. */
    private fun unlocking(ask: Gateway.Ask, plan: Giving.Plan) {
        val h = held ?: return
        h.removeAllViews()
        h.addView(words(Giving.payloadSays(ask.payload)))
        h.addView(note(plan.says))
        val buttons = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(Screen.gutter(context), 0, Screen.gutter(context), 16)
        }
        buttons.addView(button("Unlock and sign") { unlock(ask) })
        buttons.addView(button("Decline") { answer(ask, declined("declined")) })
        h.addView(buttons)
    }

    private fun unlock(ask: Gateway.Ask) {
        val prompt = runCatching {
            android.hardware.biometrics.BiometricPrompt.Builder(this)
                .setTitle("Sign for this resource")
                .setSubtitle("A signature under this device's key for it, not your identity")
                .setAllowedAuthenticators(AUTHENTICATORS)
                .build()
        }.getOrNull()
        if (prompt == null) {
            answer(ask, declined("this device would not offer its unlock"))
            return
        }
        cancelling = android.os.CancellationSignal()
        prompt.authenticate(
            cancelling!!,
            mainExecutor,
            object : android.hardware.biometrics.BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(
                    result: android.hardware.biometrics.BiometricPrompt.AuthenticationResult,
                ) {
                    Diag.event("utility", "which" to ask.utility, "state" to "unlocked")
                    runOnUiThread { answer(ask, uniffi.rhtn_ffi.UtilityOutcome.Approved) }
                }

                override fun onAuthenticationError(code: Int, message: CharSequence?) {
                    runOnUiThread {
                        answer(ask, declined("the unlock did not complete: ${message ?: code}"))
                    }
                }
            },
        )
    }

    /** Standing permission, kept by this shell per resource — the
     *  kernel holds none of it, and the person can take it back here. */
    private fun allowing(ask: Gateway.Ask) {
        getSharedPreferences(Giving.NOTIFY_STORE, MODE_PRIVATE)
            .edit()
            .putBoolean(ask.resource, true)
            .apply()
        answer(ask, uniffi.rhtn_ffi.UtilityOutcome.Approved)
    }

    // ---- the plumbing the flows share ---------------------------------

    /** Hand the outcome to the kernel and draw what comes back. */
    private fun answer(ask: Gateway.Ask, outcome: uniffi.rhtn_ffi.UtilityOutcome) {
        stopReading()
        still?.close()
        camera?.close()
        Diag.event(
            "utility",
            "which" to ask.utility,
            "state" to if (outcome is uniffi.rhtn_ffi.UtilityOutcome.Declined) "declined" else "given",
        )
        held?.let { h ->
            h.removeAllViews()
            h.addView(note("answering…"))
        }
        Thread({ show(Gateway.answerUtility((by ?: Identities.focused()), ask.id, outcome)) }, "fueros-resource-answer").start()
    }

    private fun declined(reason: String) = uniffi.rhtn_ffi.UtilityOutcome.Declined(reason)

    /** A refusal from a capability, said where the person is looking, the
     *  ask left standing so they can decline it deliberately. */
    private fun said(ask: Gateway.Ask, why: String) {
        runOnUiThread {
            still?.close()
            camera?.close()
            val h = held ?: return@runOnUiThread
            h.removeAllViews()
            h.addView(note("This device would not: $why."))
            h.addView(
                LinearLayout(this).apply {
                    orientation = LinearLayout.HORIZONTAL
                    setPadding(Screen.gutter(context), 0, Screen.gutter(context), 16)
                    addView(button("Tell it no, with that reason") { answer(ask, declined(why)) })
                },
            )
        }
    }

    private fun button(label: String, press: () -> Unit) = Button(this).apply {
        text = label
        setOnClickListener { press() }
    }

    private fun words(t: String) = TextView(this).apply {
        textSize = 15f
        text = t
        setPadding(Screen.gutter(context), 12, Screen.gutter(context), 12)
    }

    private fun held(permission: String) =
        checkSelfPermission(permission) == android.content.pm.PackageManager.PERMISSION_GRANTED

    private fun short(permission: String) = permission.substringAfterLast('.').lowercase()

    /**
     * **The question, once per resource** ([ResourcePage.SCRIPT_ASKS]):
     * the page declares that it runs code and the person has not said.
     * Either answer frames the page; only one runs it.
     */
    private fun consenting(showing: ResourcePage.Showing.Page): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        val resource = intent.getStringExtra("resource").orEmpty()
        addView(words(ResourcePage.SCRIPT_ASKS))
        val decide = { allowed: Boolean ->
            Frames.allowScript(this@ResourceActivity, resource, allowed)
            Diag.event("resource.script", "resource" to resource.take(12), "allowed" to allowed.toString())
            held?.let {
                it.removeAllViews()
                it.addView(frame(showing.html, showing.page, allowed = allowed))
            }
        }
        addView(button("Allow its code for this resource") { decide(true) })
        addView(button("Show it without its code") { decide(false) })
    }

    /**
     * **The frame, and the whole of what it permits**: a view of this
     * app's own engine ([Engine]) on a session made for this page, its
     * origin answered by [Carrier] as this screen's identity and node.
     */
    private fun frame(html: String, page: Gateway.Page, allowed: Boolean): android.view.View {
        val node = intent.getStringExtra("node").orEmpty()
        val resource = intent.getStringExtra("resource").orEmpty()
        val path = intent.getStringExtra("path") ?: "/"
        val origin = Frames.origin(resource)
        if (!Engine.carried()) {
            return note("This build carries no engine for this device's processor, so a resource's page is not shown here.")
        }
        says?.text = ResourcePage.frameSays(allowed)
        // the page as it was answered, styled, served to the first load
        Carrier.serve(
            Carrier.Serving(resource, by, node, path to page.copy(body = html.toByteArray(Charsets.UTF_8))),
        )
        unframe()
        val live = liveFor
        liveFor = null
        val view = org.mozilla.geckoview.GeckoView(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.MATCH_PARENT,
            )
        }
        Engine.with(this) { engine ->
            if (isFinishing || isDestroyed) return@with
            if (engine == null) {
                held?.removeAllViews()
                held?.addView(note("This device's engine could not be held to the frame, so the page is not shown."))
                return@with
            }
            val made = Engine.session(allowed)
            made.navigationDelegate = object : org.mozilla.geckoview.GeckoSession.NavigationDelegate {
                /** **Inside the origin the frame goes; anywhere else it does not.** */
                override fun onLoadRequest(
                    session: org.mozilla.geckoview.GeckoSession,
                    request: org.mozilla.geckoview.GeckoSession.NavigationDelegate.LoadRequest,
                ): org.mozilla.geckoview.GeckoResult<org.mozilla.geckoview.AllowOrDeny> {
                    if (Frames.resourceGoing(origin, request.uri) == Frames.Going.INSIDE) {
                        return org.mozilla.geckoview.GeckoResult.allow()
                    }
                    android.widget.Toast.makeText(
                        this@ResourceActivity,
                        "a resource's page reaches its own resource and nothing else: the frame did not follow that",
                        android.widget.Toast.LENGTH_SHORT,
                    ).show()
                    return org.mozilla.geckoview.GeckoResult.deny()
                }

                /** A frame inside the page goes where the page may, and gets nothing elsewhere. */
                override fun onSubframeLoadRequest(
                    session: org.mozilla.geckoview.GeckoSession,
                    request: org.mozilla.geckoview.GeckoSession.NavigationDelegate.LoadRequest,
                ): org.mozilla.geckoview.GeckoResult<org.mozilla.geckoview.AllowOrDeny> =
                    if (Frames.resourceGoing(origin, request.uri) == Frames.Going.INSIDE ||
                        request.uri.startsWith("about:") || request.uri.startsWith("blob:")
                    ) {
                        org.mozilla.geckoview.GeckoResult.allow()
                    } else {
                        org.mozilla.geckoview.GeckoResult.deny()
                    }

                /** No window a page could open. */
                override fun onNewSession(
                    session: org.mozilla.geckoview.GeckoSession,
                    uri: String,
                ): org.mozilla.geckoview.GeckoResult<org.mozilla.geckoview.GeckoSession>? = null
            }
            made.permissionDelegate = object : org.mozilla.geckoview.GeckoSession.PermissionDelegate {
                /** The camera and the microphone, to a page in a live
                 *  session and to no other. */
                override fun onMediaPermissionRequest(
                    session: org.mozilla.geckoview.GeckoSession,
                    uri: String,
                    video: Array<out org.mozilla.geckoview.GeckoSession.PermissionDelegate.MediaSource>?,
                    audio: Array<out org.mozilla.geckoview.GeckoSession.PermissionDelegate.MediaSource>?,
                    callback: org.mozilla.geckoview.GeckoSession.PermissionDelegate.MediaCallback,
                ) {
                    if (liveUntil == 0L) callback.reject() else callback.grant(video?.firstOrNull(), audio?.firstOrNull())
                }

                /** The platform's own question, put to the person by the platform. */
                override fun onAndroidPermissionsRequest(
                    session: org.mozilla.geckoview.GeckoSession,
                    permissions: Array<out String>?,
                    callback: org.mozilla.geckoview.GeckoSession.PermissionDelegate.Callback,
                ) {
                    if (liveUntil == 0L || permissions.isNullOrEmpty()) {
                        callback.reject()
                        return
                    }
                    engineAsked = callback
                    requestPermissions(permissions, ENGINE)
                }

                /** Sound without a tap, in a live session: a call is heard when it connects. */
                override fun onContentPermissionRequest(
                    session: org.mozilla.geckoview.GeckoSession,
                    perm: org.mozilla.geckoview.GeckoSession.PermissionDelegate.ContentPermission,
                ): org.mozilla.geckoview.GeckoResult<Int> {
                    val sound = perm.permission ==
                        org.mozilla.geckoview.GeckoSession.PermissionDelegate.PERMISSION_AUTOPLAY_AUDIBLE ||
                        perm.permission ==
                        org.mozilla.geckoview.GeckoSession.PermissionDelegate.PERMISSION_AUTOPLAY_INAUDIBLE
                    return org.mozilla.geckoview.GeckoResult.fromValue(
                        if (sound && liveUntil != 0L) {
                            org.mozilla.geckoview.GeckoSession.PermissionDelegate.ContentPermission.VALUE_ALLOW
                        } else {
                            org.mozilla.geckoview.GeckoSession.PermissionDelegate.ContentPermission.VALUE_DENY
                        },
                    )
                }
            }
            val load = {
                made.open(engine)
                session = made
                Engine.opened(made)
                view.setSession(made)
                // **the origin's own URL, served from the answer in hand**:
                // the document is same-origin with its resource, and the
                // engine holds it to the policy the resource declared
                made.loadUri("$origin$path")
            }
            if (live == null) {
                load()
            } else {
                // **a live session the person approved** (§8.1): every
                // other frame is shut, pages are given a peer connection,
                // and this one is loaded with the time it has
                Engine.only(made)
                Engine.peers(true) { on ->
                    if (isFinishing || isDestroyed) {
                        Engine.peers(false) {}
                        return@peers
                    }
                    if (!on) {
                        held?.removeAllViews()
                        held?.addView(note("This device's engine did not take the live session, so it was not started."))
                        return@peers
                    }
                    Diag.event("live", "state" to "started", "seconds" to live.toString())
                    liveUntil = android.os.SystemClock.elapsedRealtime() + live * 1000
                    load()
                    counting()
                }
            }
        }
        return view
    }

    /** `text/plain` is not a page, so it is not framed as one. */
    private fun text(body: String): ScrollView = ScrollView(this).apply {
        layoutParams = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.MATCH_PARENT,
        )
        addView(
            TextView(this@ResourceActivity).apply {
                textSize = 14f
                typeface = Type.mono
                text = body
                setPadding(Screen.gutter(context), 8, Screen.gutter(context), 48)
            },
        )
    }

    private fun note(t: String) = TextView(this).apply {
        textSize = 14f
        text = t
        setPadding(Screen.gutter(context), 12, Screen.gutter(context), 24)
    }

    override fun onPause() {
        // the radio is the foreground activity's alone, and a reader mode
        // left on belongs to a screen nobody is looking at
        stopReading()
        super.onPause()
    }

    override fun onStop() {
        // **leaving the page ends a live session**: nothing is connected
        // past this app for a screen nobody is looking at
        endLive()
        super.onStop()
    }

    override fun onResume() {
        super.onResume()
        // another screen's live session shut this frame ([Engine.only]):
        // made again now that this one is in front
        if (session?.isOpen == false) recreate()
    }

    override fun onDestroy() {
        camera?.close()
        camera = null
        still?.close()
        still = null
        runCatching { cancelling?.cancel() }
        cancelling = null
        stopReading()
        // **what the page stored stays, under its own origin** [author,
        // 2026-11-15]: the resource's, until the person forgets the
        // resource ([Frames.forget]); the view itself goes with the screen
        unframe()
        intent.getStringExtra("resource")?.takeIf { it.length == 64 }?.let { runCatching { Carrier.stop(it) } }
        super.onDestroy()
    }
}
