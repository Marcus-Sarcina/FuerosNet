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
    data class Instance(val at: String, val subnet: String?, val label: String)

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
                Instance(parts[0], parts.getOrNull(1)?.ifEmpty { null }, parts.getOrElse(2) { parts[0] })
            }

    private fun remember(i: Instance) {
        val kept = instances().filter { it.at != i.at } + i
        getSharedPreferences("operator", MODE_PRIVATE).edit()
            .putString("instances", kept.joinToString("\n") { "${it.at}\t${it.subnet ?: ""}\t${it.label}" })
            .apply()
    }

    private fun forget(at: String) {
        val kept = instances().filter { it.at != at }
        getSharedPreferences("operator", MODE_PRIVATE).edit()
            .putString("instances", kept.joinToString("\n") { "${it.at}\t${it.subnet ?: ""}\t${it.label}" })
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
                                val said = Kernel.renew(i.at, token, 45)
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
        }
    }

    private fun field(hint: String, value: String = "", numeric: Boolean = false): EditText =
        EditText(this).apply {
            this.hint = hint
            setText(value)
            textSize = 15f
            if (numeric) {
                inputType = InputType.TYPE_CLASS_NUMBER
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
        val subtree = field("subtree to claim, or blank", numeric = true)

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
            if (scopes.size > 1) {
                addView(
                    TextView(this@OperatorActivity).apply {
                        textSize = 13f
                        setTextColor(Color.GRAY)
                        text = "It serves:"
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
                    subtree = subtree.text.toString().trim().toLongOrNull(),
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
                        ask.subtree,
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
            fitsSystemWindows = true
            setPadding(48, 48, 48, 48)
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
                    text = "Enrol another"
                },
            )
            listOf(at, token, credentials, endpoint, subtree).forEach { addView(it) }
            addView(choice)
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
        setContentView(ScrollView(this).apply { addView(body) })
    }

    /** What the screen was asked to do, so the thread carries no views. */
    private data class Ask(
        val at: String,
        val token: String,
        val credentials: Int,
        val endpoint: String?,
        val subtree: Long?,
        val subnet: String?,
    )
}
