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
    private var pageAt: String? = null

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
        val go = Button(this).apply {
            text = "Check the proof, then enrol"
            setOnClickListener {
                isEnabled = false
                narration.text = "asking ${at.text}…"
                open.isEnabled = false
                val ask = Ask(
                    at = at.text.toString().trim(),
                    token = token.text.toString().trim(),
                    credentials = credentials.text.toString().trim().toIntOrNull() ?: 7,
                    endpoint = endpoint.text.toString().trim().ifEmpty { null },
                    subtree = subtree.text.toString().trim().toLongOrNull(),
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
                    )
                    runOnUiThread {
                        narration.text = said.joinToString("\n")
                        isEnabled = true
                        // the page is offered only where the run was taken
                        pageAt = ask.at.takeIf { said.any { l -> l.startsWith("credential") } }
                        open.isEnabled = pageAt != null
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
            listOf(at, token, credentials, endpoint, subtree).forEach { addView(it) }
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
    )
}
