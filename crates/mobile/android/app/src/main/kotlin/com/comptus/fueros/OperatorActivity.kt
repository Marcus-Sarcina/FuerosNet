package com.comptus.fueros

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.graphics.Typeface
import android.os.Bundle
import android.text.InputType
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **H3: the provisioning pages** (`Robot/light-client-screens.md`,
 * `infra-client-requirements.md` §8.3).
 *
 * §8.3 divides the surface: "a node develops and serves its own
 * administration pages; a client provides the frame they are presented
 * in". So the node's own pages are not drawn here — [NodePageActivity]
 * frames them — and what this screen owns is the one thing that cannot be
 * served by a node, because it has to work **before any node exists**: the
 * enrolment.
 *
 * **What this screen asks for is what an instance cannot know.** It minted
 * a transport key and it waits; what it needs is a run signed over that
 * key by the operator whose identity it is to speak as, and the two
 * records §4.4 says it cannot sign for itself. The seed is on this device
 * and nowhere else, which is why this is the device that does it.
 *
 * **The address and the token come from the operator**, out of band (§8.2)
 * — they wrote the token into the instance's configuration, and the
 * address is wherever they put the instance. Later these arrive from a
 * provider's launch flow (H1); nothing here assumes they were typed.
 */
class OperatorActivity : Activity() {

    private lateinit var narration: TextView
    private lateinit var open: Button
    private lateinit var controlKey: Button
    private lateinit var offering: Button
    private lateinit var roll: LinearLayout
    private var pageAt: String? = null
    private var chosen: Meet.Scope? = null

    /**
     * **One instance this operator administers**, as this screen keeps
     * it: where it is, and the subnet it serves by the label it was
     * enrolled under (`light-client-requirements.md` §10). The list is
     * the shell's own state — an operator in several subnets runs one
     * instance in each, and nothing here infers an instance from a
     * position.
     */
    data class Instance(
        val at: String,
        val subnet: String?,
        val label: String,
        /** The provider it was launched at and its id there, where this
         *  client launched it (H1); blank where it was enrolled by hand. */
        val provider: String = "",
        val remoteId: String = "",
    )

    /**
     * **The token this screen was given for an instance**, kept so a
     * renewal need not be retyped. It is the instance's configuration
     * secret, not an identity's: what it authorises is signing a run for
     * **that** instance's key, and the proof is checked against it on
     * every exchange (§8.2).
     */
    private fun tokenFor(at: String): String? =
        getSharedPreferences("operator", MODE_PRIVATE).getString("token:$at", null)

    /** The node address a member was last invited to, where they have
     *  been: what keeps a redrawing of this screen from texting them
     *  again.  Keyed by the member and never by their address, which is
     *  the one thing here that names a person outside this network. */
    private fun invitedTo(who: String): String? =
        getSharedPreferences("operator", MODE_PRIVATE).getString("invited:$who", null)

    private fun rememberInvited(who: String, at: String) {
        getSharedPreferences("operator", MODE_PRIVATE).edit()
            .putString("invited:$who", at).apply()
    }

    private fun rememberToken(at: String, token: String) {
        getSharedPreferences("operator", MODE_PRIVATE).edit()
            .putString("token:$at", token).apply()
    }

    private fun instances(): List<Instance> =
        getSharedPreferences("operator", MODE_PRIVATE)
            .getString("instances", "")!!
            .lines()
            .filter { it.isNotBlank() }
            .map { line ->
                val parts = line.split('\t')
                Instance(
                    parts[0],
                    parts.getOrNull(1)?.ifEmpty { null },
                    parts.getOrElse(2) { parts[0] },
                    parts.getOrElse(3) { "" },
                    parts.getOrElse(4) { "" },
                )
            }

    private fun line(it: Instance) = "${it.at}\t${it.subnet ?: ""}\t${it.label}\t${it.provider}\t${it.remoteId}"

    private fun remember(i: Instance) {
        val kept = instances().filter { it.at != i.at } + i
        getSharedPreferences("operator", MODE_PRIVATE).edit()
            .putString("instances", kept.joinToString("\n") { line(it) })
            .apply()
    }

    private fun forget(at: String) {
        val kept = instances().filter { it.at != at }
        getSharedPreferences("operator", MODE_PRIVATE).edit()
            .putString("instances", kept.joinToString("\n") { line(it) })
            .apply()
    }

    private companion object {
        /** The one request code this screen asks a permission under. */
        const val SMS_REQUEST = 7447
    }

    /**
     * **The invitations this node owes its members, sent** (`Invite`,
     * `light-client-requirements.md` §1.5).
     *
     * **It sends without asking, which is the ruling and not an
     * oversight** [author, 2026-11-03]: *"I want the required
     * click-throughs and exposure of network plumbing to be minimized… If
     * the patron has adopted people into a subnet then he has decided to
     * give those people access to his node that serves that subnet."* The
     * decision was taken at the adoption; a confirmation here would be
     * that decision asked again.
     *
     * **So `SEND_SMS` rather than the composer** [author, 2026-11-05]. The
     * cost is named rather than hidden: the permission is restricted, and
     * it routes the number through this application where the composer
     * would have kept it in the system's hands. The number still reaches
     * nothing off the device but the message itself.
     *
     * **Once per node address.** A member already invited to this address
     * is not texted again however often this screen is drawn; a node at a
     * new address owes a new invitation, which is the whole case for
     * holding an address at all.
     *
     * **Asked for once, and never per invite.** Where the permission is
     * not held the invitations wait and the screen says how many, with the
     * grant as the only action — a denial leaves them waiting rather than
     * turning each into a tap.
     */
    private fun sendInvitations(at: String) {
        val owed = Kernel.invitations(at).filter { invitedTo(it.who) != at }
        if (owed.isEmpty()) return
        if (checkSelfPermission(android.Manifest.permission.SEND_SMS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            narration.text = "${owed.size} invitation${if (owed.size == 1) "" else "s"} " +
                "to send for $at. This phone needs permission to send them as messages; " +
                "nothing of the numbers leaves it."
            requestPermissions(arrayOf(android.Manifest.permission.SEND_SMS), SMS_REQUEST)
            return
        }
        val sms = getSystemService(android.telephony.SmsManager::class.java) ?: return
        var sent = 0
        for (i in owed) {
            try {
                sms.sendTextMessage(i.to, null, i.link, null, null)
                rememberInvited(i.who, at)
                sent += 1
            } catch (e: Exception) {
                // **a number that will not take a message is not a reason
                // to stop**: the rest are owed too, and the mark is only
                // written for one that went
                Diag.warn("operator.invite", "state" to "refused", "why" to (e.message ?: "unsaid"))
            }
        }
        Diag.event("operator.invite", "sent" to sent, "owed" to owed.size)
        if (sent > 0) {
            narration.text = "$sent invitation${if (sent == 1) "" else "s"} sent for $at."
        }
    }

    override fun onRequestPermissionsResult(
        code: Int,
        permissions: Array<out String>,
        granted: IntArray,
    ) {
        super.onRequestPermissionsResult(code, permissions, granted)
        if (code != SMS_REQUEST) return
        if (granted.firstOrNull() == android.content.pm.PackageManager.PERMISSION_GRANTED) {
            drawInstances()
        } else {
            narration.text = "The invitations are waiting: without permission to send a " +
                "message this phone cannot deliver them, and your members can still scan a " +
                "code at your next meeting."
        }
    }

    private fun drawInstances() {
        roll.removeAllViews()
        val held = instances()
        // **the horizon is not a thing a screen remembers to send**
        // [2026-11-04]. It hung off one of two "open its administration
        // page" buttons and the other was the one an operator actually
        // used, so a node was told nothing and nothing said why. Every
        // instance this device holds a token for is caught up here, each
        // time this list is drawn: idempotent, since a row the node holds
        // at a later revision is answered `Stale` and nothing moves.
        for (i in held) {
            tokenFor(i.at)?.let { t -> assertHorizon(i.at, t, narration) }
        }
        // **and the invitations the same way**, for the same reason the
        // horizon is caught up here rather than hung off a button. The one
        // difference is that a message is outward and costs its reader
        // something, so it goes **once per node address** and not once per
        // drawing: [invite] keeps that mark, and a node at a new address
        // is a new invitation — which is the case that prompted the
        // feature, an operator not owning their address space [author,
        // 2026-11-04].
        held.forEach { sendInvitations(it.at) }
        if (held.isEmpty()) {
            roll.addView(
                TextView(this).apply {
                    textSize = 13f
                    setTextColor(Palette.now.sub)
                    text = "none yet — an instance serves one subnet, and this is where every one you run is administered"
                },
            )
            return
        }
        held.forEach { i ->
            roll.addView(
                TextView(this).apply {
                    textSize = 15f
                    typeface = Typeface.DEFAULT_BOLD
                    text = i.label
                },
            )
            roll.addView(
                TextView(this).apply {
                    textSize = 13f
                    typeface = Typeface.MONOSPACE
                    setTextColor(Palette.now.sub)
                    text = i.at
                },
            )
            roll.addView(
                Button(this).apply {
                    text = "Open its administration page"
                    setOnClickListener {
                        startActivity(
                            Intent(this@OperatorActivity, NodePageActivity::class.java)
                                .putExtra("at", i.at),
                        )
                    }
                },
            )
            roll.addView(
                Button(this).apply {
                    // **a run is 45 × 48 h and nothing renewed it**
                    // (`infra-client-requirements.md` §7): the next run
                    // starts where this one ends, which the instance says
                    text = "Renew its run"
                    setOnClickListener {
                        isEnabled = false
                        narration.text = "asking ${i.at} where its run ends…"
                        val token = tokenFor(i.at)
                        if (token == null) {
                            narration.text = "this screen holds no token for ${i.at}; " +
                                "paste it above and enrol again, which renews too"
                            isEnabled = true
                        } else {
                            Thread {
                                val said = Kernel.renew(i.at, token, 45, i.subnet)
                                runOnUiThread {
                                    narration.text = said.joinToString("\n")
                                    isEnabled = true
                                }
                            }.start()
                        }
                    }
                },
            )
            roll.addView(
                Button(this).apply {
                    text = "Forget it here"
                    setOnClickListener {
                        forget(i.at)
                        drawInstances()
                    }
                },
            )
            // the machine itself, where this client launched it (H1)
            if (i.remoteId.isNotEmpty()) {
                roll.addView(
                    Button(this).apply {
                        text = "Destroy the machine at ${i.provider}"
                        setOnClickListener { destroy(i) }
                    },
                )
            }
        }
    }

    /**
     * **The key that controls the machines this identity launched.**  The
     * public line first, because that is what is already on them; then
     * the private half on asking again, with what holding it means said
     * before it appears rather than after.
     */
    private fun showControlKey() {
        val line = Kernel.hostControlPublicKey()
        if (line == null) {
            narration.text = "this device holds no seed, so it derives no key"
            return
        }
        AlertDialog.Builder(this)
            .setTitle("Node-control ssh key")
            .setMessage(
                "This is on every machine you have launched, in root's " +
                    "authorized_keys:\n\n$line\n\n" +
                    "The private half is what ssh needs. Anyone who has it has a " +
                    "shell on those machines. It is derived from this identity, so " +
                    "it is not stored anywhere and a device restored from your " +
                    "phrase derives it again.",
            )
            .setPositiveButton("Copy the public line") { _, _ -> copy("the public line", line) }
            .setNeutralButton("Show the private key") { _, _ -> showPrivateKey() }
            .setNegativeButton("Close", null)
            .show()
    }

    private fun showPrivateKey() {
        val key = Kernel.hostControlPrivateKey()
        if (key == null) {
            narration.text = "this device holds no seed, so it derives no key"
            return
        }
        val text = TextView(this).apply {
            textSize = 10f
            typeface = Typeface.MONOSPACE
            setTextIsSelectable(true)
            this.text = key
            setPadding(Screen.gutter(context), 12, Screen.gutter(context), 12)
        }
        AlertDialog.Builder(this)
            .setTitle("The private key — a secret")
            .setView(ScrollView(this).apply { addView(text) })
            .setPositiveButton("Copy") { _, _ -> copy("the private key", key) }
            .setNegativeButton("Close", null)
            .show()
        Diag.event("operator", "op" to "control-key", "state" to "shown")
    }

    private fun copy(what: String, value: String) {
        val clip = getSystemService(android.content.ClipboardManager::class.java)
        if (clip == null) {
            narration.text = "this device offers no clipboard"
            return
        }
        clip.setPrimaryClip(android.content.ClipData.newPlainText("rhtn-host-control", value))
        narration.text = "$what is on the clipboard"
    }

    private fun field(hint: String, value: String = "", numeric: Boolean = false, secret: Boolean = false): EditText =
        EditText(this).apply {
            this.hint = hint
            setText(value)
            textSize = 15f
            if (numeric) {
                inputType = InputType.TYPE_CLASS_NUMBER
            } else if (secret) {
                // a provider's token is a key: not shown, not suggested,
                // not read off the screen by a dump
                inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            } else {
                inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
                typeface = Typeface.MONOSPACE
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val at = field("the instance's surface — host:port", "10.0.2.2:7449")
        val token = field("its enrolment token — 64 hex digits")
        val credentials = field("credentials, 48 hours each", "7", numeric = true)
        val endpoint = field("the address to publish — host:port, or blank")

        narration = TextView(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.sub)
            text = ""
        }
        // **the operator's own key, shown on the one screen that is about
        // their machines** (`rhtn_crypto::ssh`): the public line is what a
        // launch installs, and the private half is a secret this shell
        // shows and never writes. It is derived, so showing it again is
        // not a second copy of anything.
        controlKey = Button(this).apply {
            text = "Show this identity's node-control ssh key"
            setOnClickListener { showControlKey() }
        }
        // **a package offered for installation** [author, 2026-11-02]: a
        // link opens this client on its own (the manifest's intent
        // filter), and this is the way in for one that arrived by some
        // route with no handler — a page read on a desktop, a code
        // somebody read out. It authorises nothing either way
        // ([OfferingActivity]).
        offering = Button(this).apply {
            text = "Install a package from an offering"
            setOnClickListener {
                startActivity(Intent(this@OperatorActivity, OfferingActivity::class.java))
            }
        }
        open = Button(this).apply {
            text = "Open its administration page"
            isEnabled = false
            setOnClickListener {
                pageAt?.let {
                    startActivity(
                        Intent(this@OperatorActivity, NodePageActivity::class.java)
                            .putExtra("at", it),
                    )
                }
            }
        }
        // **which subnet the instance serves** [author, 2026-10-15]: one
        // it holds a position in, chosen here; where it holds one, that one
        val scopes = Kernel.subnets().map(Meet.Scope::of)
        chosen = scopes.singleOrNull()
        val choice = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            // **one chooser for both ways of enrolling** [author, 2026-10-25]:
            // the launch below needs this choice too, and when it sat in the
            // by-hand section it looked like the by-hand section's alone
            if (scopes.size <= 1) {
                addView(
                    TextView(this@OperatorActivity).apply {
                        textSize = 13f
                        setTextColor(Palette.now.sub)
                        text = when (val s = scopes.singleOrNull()) {
                            null -> "you hold a position in no subnet yet; an instance enrolled now serves none"
                            else -> "your one subnet, ${s.label()}: an instance enrolled here serves it"
                        }
                    },
                )
            }
            if (scopes.size > 1) {
                addView(
                    TextView(this@OperatorActivity).apply {
                        textSize = 13f
                        setTextColor(Palette.now.sub)
                        text = "You hold positions in several; choose which one this instance serves:"
                    },
                )
                val group = android.widget.RadioGroup(this@OperatorActivity)
                scopes.forEachIndexed { n, s ->
                    group.addView(
                        android.widget.RadioButton(this@OperatorActivity).apply {
                            id = n + 1
                            text = s.label()
                            setOnClickListener { chosen = s }
                        },
                    )
                }
                addView(group)
            }
        }

        val go = Button(this).apply {
            text = "Check the proof, then enrol"
            setOnClickListener {
                if (scopes.size > 1 && chosen == null) {
                    narration.text = "choose which subnet the instance serves first"
                    return@setOnClickListener
                }
                isEnabled = false
                narration.text = "asking ${at.text}…"
                open.isEnabled = false
                val ask = Ask(
                    at = at.text.toString().trim(),
                    token = token.text.toString().trim(),
                    credentials = credentials.text.toString().trim().toIntOrNull() ?: 7,
                    endpoint = endpoint.text.toString().trim().ifEmpty { null },
                    // the line the records are signed on (§7.6)
                    subnet = chosen?.id?.takeIf { it.length == 64 },
                )
                // **off the main thread**: this is several round trips to a
                // host that may not answer, and a screen that froze while
                // waiting would tell its operator nothing
                Thread {
                    val said = Kernel.enrol(
                        ask.at,
                        ask.token,
                        ask.credentials,
                        ask.endpoint,
                        ask.subnet,
                    )
                    runOnUiThread {
                        narration.text = said.joinToString("\n")
                        isEnabled = true
                        // the page is offered only where the run was taken
                        pageAt = ask.at.takeIf { said.any { l -> l.startsWith("credential") } }
                        open.isEnabled = pageAt != null
                        // and an instance that took its run is one this
                        // operator administers from now, under its subnet
                        if (pageAt != null) {
                            val s = chosen
                            remember(Instance(ask.at, s?.id, s?.label() ?: "an instance (subnet not named)"))
                            rememberToken(ask.at, ask.token)
                            drawInstances()
                            // **and it is told who is in the horizon it
                            // serves** [author, 2026-11-04]: a node learns
                            // its members from the table its operator
                            // maintains, not from a replay it has no reach
                            // to judge. Off this thread, since it is a row
                            // per member over the network, and the operator
                            // is told the count rather than asked anything.
                            assertHorizon(ask.at, ask.token, narration)
                        }
                    }
                }.start()
            }
        }

        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(Screen.gutter(context), 48, Screen.gutter(context), 48)
            addView(
                TextView(this@OperatorActivity).apply {
                    textSize = 28f
                    typeface = Typeface.DEFAULT_BOLD
                    text = "Operator"
                },
            )
            addView(
                TextView(this@OperatorActivity).apply {
                    textSize = 13f
                    setTextColor(Palette.now.sub)
                    setPadding(0, 8, 0, 32)
                    text = "An instance holds no seed. It mints a transport key and waits for a " +
                        "run you sign over it — and for the endpoint record and anchor entry it " +
                        "cannot sign for itself. Nothing is signed until the proof checks that " +
                        "the key is that instance's."
                },
            )
            addView(
                TextView(this@OperatorActivity).apply {
                    setPadding(0, 8, 0, 8)
                    textSize = 12f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Palette.now.sub)
                    text = "Instances you run"
                },
            )
            roll = LinearLayout(this@OperatorActivity).apply { orientation = LinearLayout.VERTICAL }
            addView(roll)
            drawInstances()
            addView(
                TextView(this@OperatorActivity).apply {
                    setPadding(0, 32, 0, 8)
                    textSize = 12f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Palette.now.sub)
                    text = "Which of your subnets the instance serves"
                },
            )
            addView(choice)
            addView(
                TextView(this@OperatorActivity).apply {
                    setPadding(0, 32, 0, 8)
                    textSize = 12f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Palette.now.sub)
                    text = "Launch one at a provider"
                },
            )
            addView(launchSection(credentials, at, token, endpoint))
            addView(
                TextView(this@OperatorActivity).apply {
                    setPadding(0, 32, 0, 8)
                    textSize = 12f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Palette.now.sub)
                    text = "Enrol one by hand"
                },
            )
            listOf(at, token, credentials, endpoint).forEach { addView(it) }
            addView(go)
            addView(open)
            addView(
                TextView(this@OperatorActivity).apply {
                    setPadding(0, 32, 0, 8)
                    textSize = 12f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Palette.now.sub)
                    text = "Your way in to the machines you rent"
                },
            )
            addView(
                TextView(this@OperatorActivity).apply {
                    textSize = 13f
                    setTextColor(Palette.now.sub)
                    text = "Every machine you launch takes an ssh key this identity " +
                        "derives, so you have a shell on it. It is derived from your own " +
                        "signing key: restore this identity from its phrase on any device " +
                        "and the same key comes back, which is how a machine outlives the " +
                        "phone that launched it."
                },
            )
            addView(controlKey)
            addView(
                TextView(this@OperatorActivity).apply {
                    setPadding(0, 32, 0, 8)
                    textSize = 12f
                    typeface = Typeface.MONOSPACE
                    setTextColor(Palette.now.sub)
                    text = "A package's distributor publishes one link or code carrying the " +
                        "image, a suggested authority and port, and the package's own " +
                        "manifest. Opening one fills this screen's install and nothing more: " +
                        "the resource is minted here, the owner is you, and which instance it " +
                        "goes on is your choice."
                },
            )
            addView(offering)
            addView(
                TextView(this@OperatorActivity).apply {
                    setPadding(0, 32, 0, 8)
                    textSize = 12f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Palette.now.sub)
                    text = "What happened"
                },
            )
            addView(narration)
        }
        // **the bars are the frame's, not the scrolling content's**
        // ([`Screen`]): an inset applied inside a `ScrollView`
        // scrolls away, which is how the foot of this screen came to
        // sit under the navigation bar
        val frame = ScrollView(this).apply { addView(body) }
        Screen.insetting(frame)
        setContentView(frame)
    }

    /**
     * **H1, the launch** (`Launch`; `light-client-requirements.md` §10): a
     * provider and the credential the operator holds there, a region and a
     * size, then one button that launches the machine, waits for its
     * address and its surface, and runs the enrolment above with the
     * token this client minted. Each step is narrated as the enrolment's
     * are, and a step that fails stops the rest — a machine left running
     * with no run signed is said so, with its id, so it can be destroyed.
     */
    private fun launchSection(
        credentials: EditText,
        atField: EditText,
        tokenField: EditText,
        endpointField: EditText,
    ): LinearLayout {
        // **every provider this build drives, least crowded first**
        // (PRD-11; `light-client-requirements.md` §6): the ordering speaks
        // to this operator's own redundancy rather than the network's, it
        // hides none and it scores none
        val all = Launch.providers().sortedBy { Kernel.crowded(it.asn).first }
        var provider = all.first()
        // **and the ordering says what it is** (PRD-11), in the author's
        // own words rather than this screen's: what the table holds is
        // fixed by the kind of thing being disclosed, so the sentence
        // lives in [Disclosure] and is rendered here
        val ordering = TextView(this).apply {
            textSize = 13f
            setTextColor(Palette.now.sub)
            text = Disclosure.words(Disclosure.Topic.PROVISIONING).joinToString("\n\n")
        }
        val heldFor = { p: Launch.Provider -> Kernel.providerCredentials()[p.name.lowercase()] ?: "" }
        val credential = field("${provider.name} API token", heldFor(provider), secret = true)
        // the token is the operator's own, made at the provider's page:
        // open it, and say the steps there
        val getToken = Button(this).apply {
            text = "Get a token at ${provider.name}"
            setOnClickListener {
                narration.text = provider.tokenWalkthrough.withIndex()
                    .joinToString("\n") { (n, step) -> "${n + 1}. $step" }
                runCatching { startActivity(Intent(Intent.ACTION_VIEW, android.net.Uri.parse(provider.tokenPage))) }
                    .onFailure { narration.text = "no browser opened; the page is ${provider.tokenPage}\n\n${narration.text}" }
            }
        }
        val registry = field("registry to pull the node image from", Launch.REGISTRY_DEFAULT)
        val tag = field("image tag", Launch.TAG_DEFAULT)
        val regions = android.widget.Spinner(this)
        val sizes = android.widget.Spinner(this)
        var regionRows: List<Launch.Region> = listOf()
        var sizeRows: List<Launch.Size> = listOf()
        // everything the provider offers, before either picker narrows it
        var allRegions: List<Launch.Region> = listOf()
        var allSizes: List<Launch.Size> = listOf()
        // what the person has settled on; whichever was chosen first stays
        var heldRegion: String? = null
        var heldSize: String? = null

        val about = TextView(this).apply {
            textSize = 13f
            setTextColor(Palette.now.sub)
        }

        /**
         * **Draw both pickers against each other**, and say what the
         * ordering and the marks mean.
         *
         * A region chosen narrows the sizes to those offered there; a size
         * chosen narrows the regions to those offering it. Regions that
         * cannot host a node at all are shown **last and not hidden**,
         * because an operator may know better than this screen, and
         * `light-client-requirements.md` §9's rule is that a refusal
         * renders its reason — a silent reordering is a refusal in
         * disguise.
         */
        fun narrowPickers(regions: android.widget.Spinner, sizes: android.widget.Spinner) {
            val forSize = heldSize?.let { held ->
                allSizes.firstOrNull { it.slug == held }?.regions ?: listOf()
            } ?: listOf()
            regionRows = Sizing.regionsByRecommended(
                Sizing.regionsFor(forSize, allRegions) { it.slug },
                allSizes,
                { it.slug },
                { it.memoryMb.toLong() },
                { it.regions },
            )
            sizeRows = heldRegion?.let { Sizing.sizesIn(it, allSizes) { z -> z.regions } } ?: allSizes
            regions.adapter = android.widget.ArrayAdapter(
                this@OperatorActivity,
                android.R.layout.simple_spinner_dropdown_item,
                regionRows.map { reg ->
                    val ok = Sizing.meetsRecommended(
                        reg.slug, allSizes, { it.memoryMb.toLong() }, { it.regions },
                    )
                    "${reg.slug}  ${reg.name}" + if (ok) "" else "  — too small here"
                },
            )
            sizes.adapter = android.widget.ArrayAdapter(
                this@OperatorActivity,
                android.R.layout.simple_spinner_dropdown_item,
                sizeRows.map { z ->
                    val mark = if (Sizing.recommended(z.memoryMb.toLong())) "★ " else ""
                    "$mark${z.slug}  ${z.memoryMb} MB, ${z.vcpus} vcpu, \$${z.monthly}/month"
                },
            )
            about.text = Sizing.note() + "\n\n" + Sizing.whyBehind()
        }
        fun describe() {
            val (count, crowded) = Kernel.crowded(provider.asn)
            about.text = "${provider.name} (AS${provider.asn}): " +
                (if (crowded) "$count of the parties in your horizon already run there — crowded, and still yours to choose. " else "nobody in your horizon runs there. ") +
                "That speaks to your own redundancy, not the network's."
        }
        describe()
        // **which one of them**, chosen here: a build driving two
        // providers and offering one would be choosing for the operator
        val chooser = android.widget.Spinner(this).apply {
            adapter = android.widget.ArrayAdapter(
                this@OperatorActivity,
                android.R.layout.simple_spinner_dropdown_item,
                all.map { p ->
                    val (count, crowded) = Kernel.crowded(p.asn)
                    "${p.name} (AS${p.asn})" + if (crowded) "  — $count in your horizon" else ""
                },
            )
            onItemSelectedListener = object : android.widget.AdapterView.OnItemSelectedListener {
                override fun onItemSelected(
                    parent: android.widget.AdapterView<*>?,
                    view: android.view.View?,
                    position: Int,
                    id: Long,
                ) {
                    provider = all[position]
                    describe()
                    getToken.text = "Get a token at ${provider.name}"
                    credential.hint = "${provider.name} API token"
                    credential.setText(heldFor(provider))
                    // what was fetched was the provider chosen then: a
                    // region of one provider is not a region of another
                    regionRows = listOf()
                    sizeRows = listOf()
                    val empty = android.widget.ArrayAdapter(
                        this@OperatorActivity,
                        android.R.layout.simple_spinner_dropdown_item,
                        listOf<String>(),
                    )
                    regions.adapter = empty
                    sizes.adapter = empty
                }

                override fun onNothingSelected(parent: android.widget.AdapterView<*>?) {}
            }
        }
        val fetch = Button(this).apply {
            text = "Fetch regions and sizes"
            setOnClickListener {
                val cred = credential.text.toString().trim()
                if (cred.isEmpty()) { narration.text = "paste the ${provider.name} API token first"; return@setOnClickListener }
                isEnabled = false
                narration.text = "asking ${provider.name}…"
                Thread {
                    val r = provider.regions(cred)
                    val s = provider.sizes(cred)
                    runOnUiThread {
                        isEnabled = true
                        if (r.isFailure || s.isFailure) {
                            narration.text = (r.exceptionOrNull() ?: s.exceptionOrNull())?.message ?: "refused"
                            return@runOnUiThread
                        }
                        allRegions = r.getOrThrow().filter { it.available }
                        allSizes = s.getOrThrow().sortedBy { it.monthly }
                        // **the two pickers constrain each other**
                        // (`Robot/future-features.md` entry 2): both lists
                        // already knew how they intersect — a size carries
                        // the regions it is offered in — and the screen
                        // populated them independently, so many pairs a
                        // person could choose were refused at creation.
                        narrowPickers(regions, sizes)
                        // **whichever was chosen first stays chosen**
                        // (entry 2): a listener that re-narrowed on every
                        // redraw would move the selection under the
                        // person's thumb
                        regions.onItemSelectedListener =
                            object : android.widget.AdapterView.OnItemSelectedListener {
                                override fun onItemSelected(
                                    parent: android.widget.AdapterView<*>?,
                                    view: android.view.View?,
                                    position: Int,
                                    id: Long,
                                ) {
                                    val picked = regionRows.getOrNull(position) ?: return
                                    if (heldRegion == picked.slug) return
                                    heldRegion = picked.slug
                                    heldSize = null
                                    narrowPickers(regions, sizes)
                                }

                                override fun onNothingSelected(p: android.widget.AdapterView<*>?) = Unit
                            }
                        sizes.onItemSelectedListener =
                            object : android.widget.AdapterView.OnItemSelectedListener {
                                override fun onItemSelected(
                                    parent: android.widget.AdapterView<*>?,
                                    view: android.view.View?,
                                    position: Int,
                                    id: Long,
                                ) {
                                    val picked = sizeRows.getOrNull(position) ?: return
                                    if (heldSize == picked.slug) return
                                    // a size chosen while a region is held
                                    // narrows nothing further: the region
                                    // already decided this list
                                    if (heldRegion != null) return
                                    heldSize = picked.slug
                                    narrowPickers(regions, sizes)
                                }

                                override fun onNothingSelected(p: android.widget.AdapterView<*>?) = Unit
                            }
                        // the credential is kept once it has worked, and
                        // travels in the backup from here
                        Kernel.holdProviderCredential(provider.name.lowercase(), cred)?.let { narration.text = it }
                            ?: run { narration.text = "${regionRows.size} regions, ${sizeRows.size} sizes" }
                    }
                }.start()
            }
        }
        val go = Button(this).apply {
            text = "Launch, wait for it, then enrol"
            setOnClickListener {
                val cred = credential.text.toString().trim()
                val region = regionRows.getOrNull(regions.selectedItemPosition)
                val size = sizeRows.getOrNull(sizes.selectedItemPosition)
                if (cred.isEmpty() || region == null || size == null) {
                    narration.text = "fetch the regions and sizes first, and pick one of each"
                    return@setOnClickListener
                }
                val scopes = Kernel.subnets().map(Meet.Scope::of)
                if (scopes.size > 1 && chosen == null) {
                    narration.text = "choose which subnet the instance serves first"
                    return@setOnClickListener
                }
                val material = Kernel.materialHex() ?: run { narration.text = "the kernel is not running"; return@setOnClickListener }
                isEnabled = false
                val plan = Launch.Plan(region.slug, size.slug, registry = registry.text.toString().trim(), tag = tag.text.toString().trim())
                val minted = Launch.token()
                val creds = credentials.text.toString().trim().toIntOrNull() ?: 7
                val subnet = chosen?.id?.takeIf { it.length == 64 }
                val label = chosen?.label() ?: "an instance (subnet not named)"
                Thread {
                    val said = mutableListOf<String>()
                    fun show() = runOnUiThread { narration.text = said.joinToString("\n") }
                    val launched = provider.launch(cred, plan, "rhtn-${minted.take(8)}", Launch.userData(material, minted, plan.registry, plan.tag))
                    val id = launched.getOrElse {
                        said += "launch refused: ${it.message}"; show(); runOnUiThread { isEnabled = true }; return@Thread
                    }.id
                    said += "${provider.name} created machine $id in ${region.slug}; waiting for its address"; show()
                    var ip: String? = null
                    val deadline = System.currentTimeMillis() + 5 * 60_000
                    while (ip == null && System.currentTimeMillis() < deadline) {
                        Thread.sleep(5_000)
                        val st = provider.standing(cred, id).getOrNull() ?: continue
                        if (st.status == "active" && st.ipv4 != null) ip = st.ipv4
                    }
                    if (ip == null) {
                        said += "no address within five minutes; the machine $id is running at ${provider.name} and can be destroyed from the list"
                        remember(Instance("", subnet, "$label — launching, no address", provider.name.lowercase(), id))
                        show(); runOnUiThread { isEnabled = true; drawInstances() }; return@Thread
                    }
                    val surface = "$ip:${Launch.ADMIN_PORT}"
                    said += "address $ip; waiting for the instance to install, pull the image and open its surface"; show()
                    val began = System.currentTimeMillis()
                    val up = began + 8 * 60_000
                    var answered = false
                    // **the wait says what it is waiting for, and stops as
                    // soon as the answer is known** [author, 2026-11-02]:
                    // a machine answering ssh with no node has had its
                    // engine and its pull, so there is nothing further to
                    // wait for ([Launch.silence])
                    var reachedSsh = false
                    var waited = 0L
                    while (!answered && System.currentTimeMillis() < up) {
                        Thread.sleep(10_000)
                        waited = (System.currentTimeMillis() - began) / 1000
                        answered = Provision.fetch(surface).isSuccess
                        if (answered) break
                        reachedSsh = Launch.reachable(ip, 22)
                        if (reachedSsh && waited >= Launch.firstBootSeconds()) break
                        // one progress line, replaced rather than piled up
                        if (said.isNotEmpty() && said.last().startsWith("waiting ")) {
                            said.removeAt(said.size - 1)
                        }
                        said += Launch.silence(reachedSsh, false, waited, false, surface, provider.name, id)
                        show()
                    }
                    if (!answered) {
                        // **what it was, and the one place the truth is**:
                        // the client cannot read the machine's log, and
                        // the operator's own key can
                        said += Launch.silence(reachedSsh, false, waited, true, surface, provider.name, id)
                        val how = Launch.silenceLabel(reachedSsh, false, waited, true)
                        remember(Instance(surface, subnet, "$label — $how", provider.name.lowercase(), id))
                        rememberToken(surface, minted)
                        show(); runOnUiThread { isEnabled = true; drawInstances() }; return@Thread
                    }
                    said += "its surface answers; enrolling"; show()
                    said += Kernel.enrol(surface, minted, creds, "$ip:${Launch.QUIC_PORT}", subnet)
                    val took = said.any { it.startsWith("credential") }
                    runOnUiThread {
                        narration.text = said.joinToString("\n")
                        isEnabled = true
                        if (took) {
                            remember(Instance(surface, subnet, label, provider.name.lowercase(), id))
                            rememberToken(surface, minted)
                            // **the launch enrols too, and this is the path
                            // a new machine actually takes** [2026-11-04]:
                            // the first wiring of this put it only on the
                            // separate enrolment flow, so a node launched
                            // from here was never told its horizon.
                            assertHorizon(surface, minted, narration)
                            // **and this device serves from it, without
                            // being asked** [author, 2026-11-04]:
                            // "attachment is not a separate act. The
                            // operator should attach upon launch". The key
                            // is this device's own, because an operator's
                            // node speaks as its operator; the address is
                            // the machine it just launched.
                            Kernel.keyhash()?.let { me ->
                                Kernel.serveFrom(this@OperatorActivity, me, "$ip:${Launch.QUIC_PORT}")
                            }
                            atField.setText(surface)
                            tokenField.setText(minted)
                            endpointField.setText("$ip:${Launch.QUIC_PORT}")
                            pageAt = surface
                            open.isEnabled = true
                            drawInstances()
                        }
                    }
                }.start()
            }
        }
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(TextView(this@OperatorActivity).apply { textSize = 12f; setTextColor(Palette.now.sub); text = "Provider" })
            // **what the ordering below means, before the list of it**
            // (PRD-11): a person reading a sorted list is owed the basis
            // of the sort before they act on it
            addView(ordering)
            addView(chooser)
            addView(about)
            addView(getToken)
            addView(credential)
            addView(fetch)
            addView(TextView(this@OperatorActivity).apply { textSize = 12f; setTextColor(Palette.now.sub); text = "Region" })
            addView(regions)
            addView(TextView(this@OperatorActivity).apply { textSize = 12f; setTextColor(Palette.now.sub); text = "Size" })
            addView(sizes)
            addView(registry)
            addView(tag)
            addView(go)
        }
    }

    /**
     * **Destroy a machine this client launched**, at the provider, with
     * the credential held there: the one lifecycle act beyond enrol and
     * renew that H1 gives an operator, and only for what H1 launched.
     */
    private fun destroy(i: Instance) {
        val provider = Launch.providers().firstOrNull { it.name.lowercase() == i.provider } ?: return
        val cred = Kernel.providerCredentials()[i.provider] ?: run {
            narration.text = "no ${provider.name} credential is held; paste it above first"
            return
        }
        narration.text = "destroying machine ${i.remoteId} at ${provider.name}…"
        Thread {
            val r = provider.destroy(cred, i.remoteId)
            runOnUiThread {
                narration.text = r.fold({ "destroyed machine ${i.remoteId}" }, { "destroy refused: ${it.message}" })
                if (r.isSuccess) { forget(i.at); drawInstances() }
            }
        }.start()
    }

    /** What the screen was asked to do, so the thread carries no views. */
    private data class Ask(
        val at: String,
        val token: String,
        val credentials: Int,
        val endpoint: String?,
        val subnet: String?,
    )

    /**
     * **Tell a node this operator administers who is in the horizon it
     * serves**, a row per member (`Robot/deferred-decisions.md`, the
     * horizon as a table its operator maintains).
     *
     * **What the operator is told is a count, and what was refused.** They
     * are not asked anything: a node runs under their authority, and the
     * rows are hex they have no reason to read. A member left out for want
     * of key material is the kernel's decision and is not reported here as
     * a failure, because nothing failed.
     */
    private fun assertHorizon(at: String, token: String, narration: TextView) {
        Thread {
            val rows = Kernel.horizonRows()
            if (rows.isEmpty()) {
                return@Thread
            }
            val said = Provision.assertHorizon(at, token, rows).fold(
                onSuccess = { p ->
                    val refused = if (p.refused.isEmpty()) {
                        ""
                    } else {
                        "\n" + p.refused.joinToString("\n") { "  refused — $it" }
                    }
                    "the node holds ${p.taken} of ${rows.size} member(s) of its horizon$refused"
                },
                onFailure = { "the horizon could not be sent: ${it.message ?: "refused"}" },
            )
            runOnUiThread { narration.text = "${narration.text}\n$said" }
        }.start()
    }
}
