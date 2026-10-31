package com.comptus.fueros

import android.annotation.SuppressLint
import android.app.Activity
import android.graphics.Typeface
import android.os.Bundle
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewClient
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
 * that enforces it. In one place, because a second screen deciding what a
 * page may do is how the two come apart:
 *
 *  - **A null base URL**, so the document has an opaque origin: same
 *    origin with nothing, not even itself. There is no URL to give it —
 *    §11's exchange is one request and one response through the kernel —
 *    and nothing to be same-origin *with* is the strongest isolation a
 *    web view has.
 *  - **No network, twice over**: the settings block loads, and
 *    `shouldInterceptRequest` answers every request with nothing. A
 *    page's `src` has no base to resolve against in any case, so this
 *    costs an application nothing it could have had.
 *  - **No script**, and with no origin, no network and no bridge there is
 *    nothing script could reach: not the resource, not this process, not
 *    the device. A form is carried by the shell through the kernel
 *    either way.
 *  - **No bridge**: nothing is published into the page.
 *  - **No local reach**: file and content access off, both file-URL
 *    escapes off.
 *  - **No storage**, and what a page leaves behind is deleted with the
 *    screen.
 *  - **No navigation**: there is no origin to stay inside, so every
 *    navigation is refused and the person is told the frame did not go.
 *
 * **Text takes no frame at all.** `text/plain` is not a page, so it is
 * drawn in a `TextView` and no web view is created — the policy's own
 * rule, which is also the cheaper path.
 */
class ResourceActivity : Activity() {

    private var web: WebView? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val node = intent.getStringExtra("node").orEmpty()
        val resource = intent.getStringExtra("resource").orEmpty()
        val name = intent.getStringExtra("name").orEmpty()
        val path = intent.getStringExtra("path") ?: "/"

        val title = TextView(this).apply {
            textSize = 19f
            typeface = Typeface.DEFAULT_BOLD
            text = name.ifBlank { "A resource" }
            setPadding(Screen.gutter(context), 20, Screen.gutter(context), 2)
        }
        val from = TextView(this).apply {
            textSize = 12f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.sub)
            text = ResourcePage.provenance(resource, node)
            setPadding(Screen.gutter(context), 0, Screen.gutter(context), 6)
        }
        val says = TextView(this).apply {
            textSize = 12f
            setTextColor(Palette.now.sub)
            text = ResourcePage.FRAME_SAYS
            setPadding(Screen.gutter(context), 0, Screen.gutter(context), 10)
        }
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
        Thread({ show(Kernel.ask(node, resource, path)) }, "fueros-resource").start()
    }

    private var held: LinearLayout? = null
    private var camera: QrCamera? = null
    private var still: Still? = null
    private var reading: android.nfc.NfcAdapter? = null
    private var cancelling: android.os.CancellationSignal? = null
    /** The ask a permission request is for, held across the dialogue. */
    private var waiting: Pair<Kernel.Ask, Giving.Plan>? = null
    /** The ask a file picker is for, held across the other app. */
    private var pending: Kernel.Ask? = null
    /** What a save is about to write, held across the picker. */
    private var saving: ByteArray? = null
    /** What the resource last answered with, for the save control. */
    private var last: Kernel.Page? = null

    private companion object {
        const val PERMISSION = 71
        const val PICK = 72
        const val SAVE = 73
    }

    /** Draw what the resource answered, from any thread. */
    private fun show(answered: Kernel.Answered?) {
        runOnUiThread {
            if (isFinishing) return@runOnUiThread
            val held = held ?: return@runOnUiThread
            last = answered?.page
            held.removeAllViews()
            when (val showing = ResourcePage.showing(answered)) {
                is ResourcePage.Showing.Page -> held.addView(frame(showing.html))
                is ResourcePage.Showing.Text -> held.addView(text(showing.text))
                is ResourcePage.Showing.Said -> {
                    held.addView(note(showing.why))
                    // **what cannot be shown can still be kept**: a file a
                    // resource answered with — an `export` the person
                    // agreed to, among others — goes where they choose
                    last?.takeIf { it.body.isNotEmpty() }?.let { held.addView(saveable(it)) }
                }
                is ResourcePage.Showing.Ask -> held.addView(asking(showing.ask))
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
    private fun asking(ask: Kernel.Ask): LinearLayout {
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
    private fun give(ask: Kernel.Ask, plan: Giving.Plan) {
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

    private fun proceed(ask: Kernel.Ask, plan: Giving.Plan) {
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
            Giving.Way.NONE -> answer(ask, declined(plan.says))
        }
    }

    // ---- a photograph -------------------------------------------------

    /** The camera, with the preview the person frames the shot in. */
    private fun photographing(ask: Kernel.Ask, plan: Giving.Plan) {
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
    private fun offering(ask: Kernel.Ask, bytes: ByteArray) {
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
    private fun picking(ask: Kernel.Ask) {
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
    private fun saveable(page: Kernel.Page): LinearLayout {
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
    private fun placing(ask: Kernel.Ask) {
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
        runCatching {
            lm.getCurrentLocation(provider, null, mainExecutor) { where ->
                runOnUiThread {
                    if (where == null) {
                        answer(ask, declined("this device could not find where it is"))
                    } else {
                        placed(ask, where)
                    }
                }
            }
        }.onFailure { answer(ask, declined("this device would not give its place: $it")) }
    }

    private fun placed(ask: Kernel.Ask, where: android.location.Location) {
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

    private fun readingCode(ask: Kernel.Ask, plan: Giving.Plan) {
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
    private fun readingTag(ask: Kernel.Ask, plan: Giving.Plan) {
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

    private fun tagRead(ask: Kernel.Ask, tag: android.nfc.Tag) {
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
    private fun unlocking(ask: Kernel.Ask, plan: Giving.Plan) {
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

    private fun unlock(ask: Kernel.Ask) {
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
    private fun allowing(ask: Kernel.Ask) {
        getSharedPreferences(Giving.NOTIFY_STORE, MODE_PRIVATE)
            .edit()
            .putBoolean(ask.resource, true)
            .apply()
        answer(ask, uniffi.rhtn_ffi.UtilityOutcome.Approved)
    }

    // ---- the plumbing the flows share ---------------------------------

    /** Hand the outcome to the kernel and draw what comes back. */
    private fun answer(ask: Kernel.Ask, outcome: uniffi.rhtn_ffi.UtilityOutcome) {
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
        Thread({ show(Kernel.answerUtility(ask.id, outcome)) }, "fueros-resource-answer").start()
    }

    private fun declined(reason: String) = uniffi.rhtn_ffi.UtilityOutcome.Declined(reason)

    /** A refusal from a capability, said where the person is looking, the
     *  ask left standing so they can decline it deliberately. */
    private fun said(ask: Kernel.Ask, why: String) {
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

    /** The frame, and the whole of what it permits. */
    @SuppressLint("SetJavaScriptEnabled")
    private fun frame(html: String): WebView = WebView(this).apply {
        web = this
        layoutParams = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.MATCH_PARENT,
        )
        settings.apply {
            javaScriptEnabled = false
            javaScriptCanOpenWindowsAutomatically = false
            domStorageEnabled = false
            databaseEnabled = false
            allowFileAccess = false
            allowContentAccess = false
            allowFileAccessFromFileURLs = false
            allowUniversalAccessFromFileURLs = false
            // nothing in the page can be fetched, and the arithmetic of
            // whether a `src` resolves is not what this rests on
            blockNetworkLoads = true
            blockNetworkImage = true
            setGeolocationEnabled(false)
            mediaPlaybackRequiresUserGesture = true
            cacheMode = android.webkit.WebSettings.LOAD_NO_CACHE
            useWideViewPort = true
            loadWithOverviewMode = true
            builtInZoomControls = true
            displayZoomControls = false
        }
        webViewClient = object : WebViewClient() {
            /** **No origin, so no navigation.** Every one is refused. */
            override fun shouldOverrideUrlLoading(
                view: WebView,
                request: WebResourceRequest,
            ): Boolean {
                view.post {
                    android.widget.Toast.makeText(
                        view.context,
                        "a resource's page goes nowhere: the frame did not follow that",
                        android.widget.Toast.LENGTH_SHORT,
                    ).show()
                }
                return true
            }

            /**
             * **And nothing is fetched.** The settings already block
             * loads; this is the same rule where a request would arrive
             * anyway, answered with an empty body rather than an error
             * the page could time.
             */
            override fun shouldInterceptRequest(
                view: WebView,
                request: WebResourceRequest,
            ): WebResourceResponse = WebResourceResponse(
                "text/plain",
                "utf-8",
                java.io.ByteArrayInputStream(ByteArray(0)),
            )
        }
        // the null base is the point: an opaque origin, same-origin with
        // nothing, not even with itself
        loadDataWithBaseURL(null, html, "text/html", "utf-8", null)
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
                typeface = Typeface.MONOSPACE
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

    override fun onDestroy() {
        camera?.close()
        camera = null
        still?.close()
        still = null
        runCatching { cancelling?.cancel() }
        cancelling = null
        stopReading()
        // **nothing the page left behind outlives the screen**
        web?.let {
            it.stopLoading()
            it.loadUrl("about:blank")
            it.destroy()
        }
        web = null
        WebStorage.getInstance().deleteAllData()
        android.webkit.CookieManager.getInstance().removeAllCookies(null)
        super.onDestroy()
    }
}
