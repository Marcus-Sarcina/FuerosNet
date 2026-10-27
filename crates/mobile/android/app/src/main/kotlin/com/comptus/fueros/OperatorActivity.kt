package com.comptus.fueros

import android.app.Activity
import android.content.Intent
import android.graphics.Color
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

    private fun drawInstances() {
        roll.removeAllViews()
        val held = instances()
        if (held.isEmpty()) {
            roll.addView(
                TextView(this).apply {
                    textSize = 13f
                    setTextColor(Color.GRAY)
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
                    setTextColor(Color.GRAY)
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
            setTextColor(Color.GRAY)
            text = ""
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
        val scopes = Kernel.scopes()
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
                        setTextColor(Color.GRAY)
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
                        setTextColor(Color.GRAY)
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
                        }
                    }
                }.start()
            }
        }

        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(Screen.GUTTER, 48, Screen.GUTTER, 48)
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
                    setTextColor(Color.GRAY)
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
                    setTextColor(Color.GRAY)
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
                    setTextColor(Color.GRAY)
                    text = "Which of your subnets the instance serves"
                },
            )
            addView(choice)
            addView(
                TextView(this@OperatorActivity).apply {
                    setPadding(0, 32, 0, 8)
                    textSize = 12f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Color.GRAY)
                    text = "Launch one at a provider"
                },
            )
            addView(launchSection(credentials, at, token, endpoint))
            addView(
                TextView(this@OperatorActivity).apply {
                    setPadding(0, 32, 0, 8)
                    textSize = 12f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Color.GRAY)
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
                    setTextColor(Color.GRAY)
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
        val about = TextView(this).apply {
            textSize = 13f
            setTextColor(Color.GRAY)
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
                        regionRows = r.getOrThrow().filter { it.available }
                        sizeRows = s.getOrThrow().sortedBy { it.monthly }
                        regions.adapter = android.widget.ArrayAdapter(
                            this@OperatorActivity,
                            android.R.layout.simple_spinner_dropdown_item,
                            regionRows.map { "${it.slug}  ${it.name}" },
                        )
                        sizes.adapter = android.widget.ArrayAdapter(
                            this@OperatorActivity,
                            android.R.layout.simple_spinner_dropdown_item,
                            sizeRows.map { "${it.slug}  ${it.memoryMb} MB, ${it.vcpus} vcpu, \$${it.monthly}/month" },
                        )
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
                val scopes = Kernel.scopes()
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
                    val up = System.currentTimeMillis() + 8 * 60_000
                    var answered = false
                    while (!answered && System.currentTimeMillis() < up) {
                        Thread.sleep(10_000)
                        answered = Provision.fetch(surface).isSuccess
                    }
                    if (!answered) {
                        said += "the surface at $surface did not answer within eight minutes; the machine is running and can be enrolled by hand or destroyed"
                        remember(Instance(surface, subnet, "$label — launched, not enrolled", provider.name.lowercase(), id))
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
            addView(TextView(this@OperatorActivity).apply { textSize = 12f; setTextColor(Color.GRAY); text = "Provider" })
            addView(chooser)
            addView(about)
            addView(getToken)
            addView(credential)
            addView(fetch)
            addView(TextView(this@OperatorActivity).apply { textSize = 12f; setTextColor(Color.GRAY); text = "Region" })
            addView(regions)
            addView(TextView(this@OperatorActivity).apply { textSize = 12f; setTextColor(Color.GRAY); text = "Size" })
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
}
