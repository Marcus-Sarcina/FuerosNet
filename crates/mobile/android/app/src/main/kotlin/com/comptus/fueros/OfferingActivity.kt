package com.comptus.fueros

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.graphics.Typeface
import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **A package offered for installation, read from a link or a code**
 * (`light-client-requirements.md` §10; `infra-client-requirements.md`
 * §9.1, §9.4; `rhtn_client::offering`).
 *
 * A distributor publishes one string carrying the image, a suggested
 * authority and port, and **the package's own manifest** — which §9.1 puts
 * with the package, so it has to come from them. Opening it here hands
 * this client everything an install wants except what only the operator
 * has.
 *
 * ## It authorises nothing
 *
 * **This is untrusted content.** It arrived from a page, a message or a
 * sticker on a wall, and every field in it is text that fills a screen the
 * operator confirms. Four things keep that true:
 *
 *  - **What the format cannot say.** There is no resource identity in it
 *    (this client mints one), no owner (that is the operator's own
 *    identity), no instance (which of theirs is their choice) and no key
 *    (derived here, for the act). An offering naming any of them is read
 *    without them, because they are not fields.
 *  - **The manifest is shown before they agree**, because that is what
 *    they are admitting: §9.1's supply chain is *trusting the package
 *    author's judgment about access*, and `resource-requirements.md` §7.3
 *    has that author's incentive run toward breadth.
 *  - **The name is a claim and is presented as one.** Nothing compares it
 *    against the image or the manifest.
 *  - **Bounded on the way in**, by the reader rather than by this screen
 *    (`rhtn_client::offering`), so a refusal reaches the person here
 *    rather than reaching the instance.
 *
 * The install itself is the declared act, made the way any other is
 * ([ActActivity], [Acts]): this screen's work is to fill it in.
 */
class OfferingActivity : Activity() {

    private lateinit var narration: TextView
    private lateinit var answer: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val g = Screen.gutter(this)

        narration = TextView(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.sub)
        }
        answer = TextView(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.muted)
        }
        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(g, g, g, g)
            addView(narration)
        }
        val root = ScrollView(this).apply { addView(body) }
        Screen.insetting(root)
        setContentView(root)

        // **a deep link opens this screen cold too** [2026-11-04]: nothing
        // but the launcher brings the kernel up, and this screen needs one
        // to mint a resource and to name its owner. Idempotent, and the
        // bring-up runs on its own thread, so by the time an operator has
        // read a manifest it is there.
        if (Kernel.hasIdentity(this)) {
            Kernel.start(this)
        }
        // **the link as it arrived**, from a tap or from this screen being
        // opened with one in hand; a code reader hands over the same text
        val link = intent.data?.toString()
            ?: intent.getStringExtra("offering")
            ?: ""
        if (link.isEmpty()) {
            // **opened with nothing in hand**, which is an operator who
            // has an offering somewhere else: a field to put it in, since
            // a link that arrives by some route this client has no
            // handler for is still a link they can paste
            narration.text = "Paste a package offering. One begins `fueros://install`."
            val pasted = EditText(this).apply {
                textSize = 13f
                typeface = Typeface.MONOSPACE
                hint = "fueros://install?v=1&…"
            }
            body.addView(pasted)
            body.addView(
                Button(this).apply {
                    this.text = "Read it"
                    setOnClickListener {
                        startActivity(
                            Intent(this@OfferingActivity, OfferingActivity::class.java)
                                .putExtra("offering", pasted.text.toString().trim()),
                        )
                        finish()
                    }
                },
            )
            return
        }
        val o = runCatching { uniffi.rhtn_ffi.packageOfferingRead(link) }.getOrElse {
            narration.text = "That is not a package offering this client reads.\n\n${it.message}"
            return
        }

        narration.text = buildString {
            append("A package is offered for installation on an instance you run.\n\n")
            append("Its distributor calls it:\n  ")
            append(o.name)
            append("\n\nThat name is their claim and nothing this client has checked.\n\n")
            append("image      ")
            append(o.image)
            append("\nauthority  ")
            append(o.authority)
            append("\nport       ")
            append(o.port?.toString() ?: "none suggested")
            append("\n\nWhat the package declares it needs — this is what you would be ")
            append("admitting:\n\n")
            append(o.manifest.trimEnd())
        }

        val instances = instances()
        if (instances.isEmpty()) {
            body.addView(
                label(
                    "You run no instance this client knows of, so there is nowhere to install " +
                        "it. Launch or enrol one from the Operator screen first.",
                ),
            )
            return
        }
        // **what the offering could not say, asked for here**
        body.addView(label("Which of your instances"))
        val instanceAt = EditText(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setText(instances.first())
            hint = instances.joinToString(", ")
        }
        body.addView(instanceAt)
        body.addView(label("The authority it answers under, which is yours to change"))
        val authority = EditText(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setText(o.authority)
        }
        body.addView(authority)
        body.addView(label("Where it listens on that machine's loopback"))
        val port = EditText(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setText((o.port ?: 7460u).toString())
        }
        body.addView(port)
        body.addView(
            label(
                "The resource's identity is minted here and its owner is you. Nothing of " +
                    "either was in the link.",
            ),
        )
        body.addView(
            Button(this).apply {
                this.text = "Install it"
                setOnClickListener {
                    go(
                        o,
                        instanceAt.text.toString().trim(),
                        authority.text.toString().trim(),
                        port.text.toString().trim(),
                    )
                }
            },
        )
        body.addView(answer)
    }

    private fun label(text: String) = TextView(this).apply {
        textSize = 12f
        typeface = Typeface.MONOSPACE
        setTextColor(Palette.now.sub)
        setPadding(0, Screen.gutter(this@OfferingActivity), 0, 0)
        this.text = text
    }

    /** The instances this client was told about ([OperatorActivity]). */
    private fun instances(): List<String> =
        getSharedPreferences("operator", Context.MODE_PRIVATE)
            .getString("instances", "")
            .orEmpty()
            .lineSequence()
            .filter { it.isNotBlank() }
            .mapNotNull { it.split('\t').firstOrNull()?.trim() }
            .filter { it.isNotEmpty() }
            .toList()

    /**
     * **Hand it to the act**, filled in: the declared install, with what
     * the offering suggested and what this client supplied, for the person
     * to confirm there as they confirm any act.
     *
     * The resource is minted here because an offering cannot carry one and
     * the operator should not be typing one: a fresh identity, of which
     * only the keyhash is kept — a socket-backed package's resource is an
     * identifier its host hands out, and nothing signs with it in this
     * version.
     */
    private fun go(
        o: uniffi.rhtn_ffi.PackageOffering,
        at: String,
        authority: String,
        port: String,
    ) {
        if (at.isEmpty()) {
            answer.text = "name one of your instances"
            return
        }
        val owner = Kernel.keyhash()
        if (owner == null) {
            answer.text = "this device holds no identity, so there is no owner to install under"
            return
        }
        val resource = Kernel.mintResource()
        if (resource == null) {
            answer.text = "this device could not mint an identity for the resource"
            return
        }
        // **the act, pre-filled and not made**: every value goes into a
        // field on the act's own screen, where the person confirms it
        val query = listOf(
            "resource" to resource,
            "owner" to owner,
            "authority" to authority,
            "image" to o.image,
            "port" to port,
            "manifest" to o.manifest,
        ).joinToString("&") { (k, v) -> "$k=${Provision.escape(v)}" }
        startActivity(
            Intent(this, ActActivity::class.java)
                .putExtra("at", at)
                .putExtra("act", "install")
                .putExtra("query", query),
        )
        finish()
    }
}
