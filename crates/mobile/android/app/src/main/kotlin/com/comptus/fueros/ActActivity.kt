package com.comptus.fueros

import android.app.Activity
import android.content.Context
import android.graphics.Typeface
import android.os.Bundle
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **One act an instance declared, drawn from its declaration**
 * (`infra-client-requirements.md` §8.3; [Acts]).
 *
 * This screen has no knowledge of any act. It is handed a name, fetches
 * the instance's declaration, finds the act there, and draws a field per
 * declared parameter from the closed set of kinds — so a node that gains
 * an act gains a screen here without this client changing, which is what
 * §8.3's division is for.
 *
 * **Three things it does that the node's page cannot.**
 *
 *  - **It proves the act.** A management act off the loopback carries a
 *    MAC under the instance's token over its own body ([Provision.proved]),
 *    which HTML in a frame cannot compute.
 *  - **It brings the credential.** Where the declaration says the act
 *    needs the host key, this derives it from the signing seed for that
 *    one act — and **asks first**, because an act that reaches the
 *    operator's machine is one they should be looking at. The frame is
 *    isolated from the seed by design (§8.3, design §23.3), so this is the
 *    only place it could happen.
 *  - **It confirms.** Nothing is sent until the person says so. The
 *    declaration arrived from a node, a seized node is planned for (design
 *    §18.1), and the hints that pre-fill these fields came from that
 *    node's own page — so they are **text in a field and never an act**.
 */
class ActActivity : Activity() {

    private lateinit var narration: TextView
    private lateinit var answer: TextView
    private lateinit var fields: LinearLayout
    private var at: String = ""
    private var token: String = ""
    private var act: Acts.Act? = null
    private val answers = LinkedHashMap<String, String>()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        at = intent.getStringExtra("at") ?: ""
        val name = intent.getStringExtra("act") ?: ""
        val query = intent.getStringExtra("query")
        token = getSharedPreferences("operator", Context.MODE_PRIVATE)
            .getString("token:$at", null) ?: ""

        narration = TextView(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.sub)
            text = "reading what $at declares…"
        }
        answer = TextView(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.muted)
        }
        fields = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }

        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            val g = Screen.gutter(this@ActActivity)
            setPadding(g, g, g, g)
            addView(narration)
            addView(fields)
            addView(answer)
        }
        val root = ScrollView(this).apply { addView(body) }
        Screen.insetting(root)
        setContentView(root)

        if (at.isEmpty() || name.isEmpty()) {
            narration.text = "No act was named."
            return
        }
        if (token.isEmpty()) {
            narration.text = "This client holds no administration token for $at, so it cannot " +
                "prove an act to it. Enrol the instance from the Operator screen first."
            return
        }
        Thread {
            val said = Provision.acts(at, token)
            runOnUiThread { found(said, name, query) }
        }.start()
    }

    /** What came back from the declaration, and the act in it. */
    private fun found(said: Result<String>, name: String, query: String?) {
        val text = said.getOrElse {
            narration.text = "could not read what $at declares: ${it.message}"
            return
        }
        val declared = Acts.read(text).getOrElse {
            narration.text = "what $at declares does not read: ${it.message}"
            return
        }
        // **a vocabulary this client does not read is said, not guessed
        // at** (§8.3): the fields may mean something else in it
        if (!declared.understood) {
            narration.text = "that instance declares its acts in `${declared.surface}` and this " +
                "client reads `${Acts.SURFACE}`. A newer client will draw them."
            return
        }
        val a = declared.act(name)
        if (a == null) {
            narration.text = "`$name` is not an act $at declares. It may have been removed, or " +
                "this client may be older than the instance."
            return
        }
        val why = Acts.undrawable(a)
        if (why != null) {
            narration.text = why
            return
        }
        act = a
        answers.putAll(Acts.hints(a, query))
        draw(a)
    }

    private fun draw(a: Acts.Act) {
        narration.text = buildString {
            append(a.label)
            if (a.help.isNotEmpty()) {
                append("\n\n")
                append(a.help)
            }
            append("\n\non $at")
        }
        fields.removeAllViews()
        for (p in a.parameters) {
            fields.addView(label(p.label.ifEmpty { p.name }))
            fields.addView(field(p))
        }
        if (a.brings == Acts.Brings.HOST_KEY) {
            fields.addView(
                label(
                    "This act asks the machine itself. Making it hands the instance the " +
                        "restricted key for its host, derived here for this one act and bound " +
                        "on the machine to one program.",
                ),
            )
        }
        fields.addView(
            Button(this).apply {
                text = "Make this act"
                setOnClickListener { send(a, this) }
            },
        )
    }

    private fun label(text: String) = TextView(this).apply {
        textSize = 12f
        typeface = Typeface.MONOSPACE
        setTextColor(Palette.now.sub)
        setPadding(0, Screen.gutter(this@ActActivity), 0, 0)
        this.text = text
    }

    /**
     * One field for one declared parameter, from the closed set.
     *
     * **A hint pre-fills it and nothing more.** The value came from the
     * node's own page; it is shown where the person can see and change it,
     * which is the whole of what a bounce is for.
     */
    private fun field(p: Acts.Parameter): LinearLayout {
        val box = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        val held = answers[p.name] ?: ""
        when (val k = p.kind) {
            is Acts.Kind.Flag -> box.addView(
                CheckBox(this).apply {
                    text = p.name
                    isChecked = held == "on"
                    answers[p.name] = if (isChecked) "on" else "off"
                    setOnCheckedChangeListener { _, on -> answers[p.name] = if (on) "on" else "off" }
                },
            )
            // **one of a list, and exactly one**: a group, so ticking one
            // unticks the rest without this screen redrawing itself from
            // inside a listener on a view it is about to remove
            is Acts.Kind.Choice -> box.addView(
                android.widget.RadioGroup(this).apply {
                    orientation = android.widget.RadioGroup.VERTICAL
                    for ((i, one) in k.of.withIndex()) {
                        addView(
                            android.widget.RadioButton(this@ActActivity).apply {
                                id = i + 1
                                this.text = one
                                isChecked = held == one
                            },
                        )
                    }
                    setOnCheckedChangeListener { _, id ->
                        k.of.getOrNull(id - 1)?.let { answers[p.name] = it }
                    }
                },
            )
            is Acts.Kind.Several -> {
                val picked = held.split(',').filter { it.isNotEmpty() }.toMutableSet()
                for (one in k.of) {
                    box.addView(
                        CheckBox(this).apply {
                            text = one
                            isChecked = one in picked
                            setOnCheckedChangeListener { _, on ->
                                if (on) picked.add(one) else picked.remove(one)
                                answers[p.name] = picked.joinToString(",")
                            }
                        },
                    )
                }
                answers[p.name] = picked.joinToString(",")
            }
            else -> box.addView(
                EditText(this).apply {
                    textSize = 13f
                    typeface = Typeface.MONOSPACE
                    setText(held)
                    hint = when (k) {
                        is Acts.Kind.Number -> "a number, ${k.low} to ${k.high}"
                        is Acts.Kind.Text -> "text, at most ${k.max} characters"
                        is Acts.Kind.Keyhash -> "64 hex digits"
                        else -> p.name
                    }
                    addTextChangedListener(
                        object : android.text.TextWatcher {
                            override fun afterTextChanged(s: android.text.Editable?) {
                                answers[p.name] = s?.toString() ?: ""
                            }

                            override fun beforeTextChanged(
                                s: CharSequence?,
                                start: Int,
                                count: Int,
                                after: Int,
                            ) = Unit

                            override fun onTextChanged(
                                s: CharSequence?,
                                start: Int,
                                before: Int,
                                count: Int,
                            ) = Unit
                        },
                    )
                },
            )
        }
        return box
    }

    /**
     * Send it, once every declared bound holds.
     *
     * **The bounds are checked here because they were declared**: a number
     * outside its range is the instance's refusal, and meeting it on the
     * screen the person is looking at is better than meeting it after a
     * round trip.
     */
    private fun send(a: Acts.Act, button: Button) {
        for (p in a.parameters) {
            val given = answers[p.name] ?: ""
            // an empty optional field is left out rather than refused;
            // what the act needs, the instance names
            if (given.isEmpty()) continue
            val why = Acts.refused(p, given)
            if (why != null) {
                answer.text = "`${p.name}`: $why"
                return
            }
        }
        val fields = Acts.body(a, answers).toMutableList()
        button.isEnabled = false
        answer.text = "asking $at…"
        Thread {
            // **the credential, derived for this act and held no longer**:
            // the kernel holds the seed, and nothing of the key is written
            // down here or there
            if (a.brings == Acts.Brings.HOST_KEY) {
                val key = Kernel.hostRunnerPrivateKey()
                if (key == null) {
                    runOnUiThread {
                        answer.text = "this device could not derive the key for that machine; " +
                            "is the kernel running under the identity that launched it?"
                        button.isEnabled = true
                    }
                    return@Thread
                }
                fields.add("key" to key)
            }
            val said = Provision.act(at, token, a.name, fields)
            runOnUiThread {
                answer.text = said.fold({ it.trim() }, { "${it.message}" })
                button.isEnabled = true
            }
        }.start()
    }
}
