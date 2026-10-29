package com.comptus.fueros

import android.app.Activity
import android.graphics.Typeface
import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * **F: the catalog, and one request to a resource**
 * (`Robot/light-client-screens.md`; `wire-format.md` §6.4, §11.1).
 *
 * A serving node answers what it holds for a type, a page at a time, and
 * the client sweeps until the node's portion is complete or its hint
 * repeats — which `Client::browse` does; this screen shows what came and
 * says plainly where a portion is **truncated** or **stale**, since a
 * catalog that looked complete when it was not would be the one dishonest
 * thing a list can do.
 *
 * **A resource is reached through its host** (§11.1): the node that served
 * the entry is the node asked, it parses and re-serialises the application
 * message (§11.2), and what comes back is one of §11.1's six statuses with
 * the application's answer where there is one. The request this screen
 * composes is the simplest HTTP/1.1 there is; a real application screen is
 * the resource's own business and not this client's.
 */
class CatalogActivity : Activity() {

    private lateinit var roll: TextView
    private lateinit var narration: TextView
    private lateinit var answer: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        roll = TextView(this).apply {
            textSize = 14f
            typeface = Typeface.MONOSPACE
        }
        narration = TextView(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.sub)
            text = "What your serving node holds. Sweep to ask it."
        }
        answer = TextView(this).apply {
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setTextColor(Palette.now.muted)
        }
        val path = EditText(this).apply {
            hint = "a path to ask for, such as /"
            setText("/")
            textSize = 14f
            typeface = Typeface.MONOSPACE
        }
        val which = EditText(this).apply {
            hint = "the resource, 64 hex digits"
            textSize = 13f
            typeface = Typeface.MONOSPACE
        }
        val sweep = Button(this).apply {
            text = "Sweep the catalog"
            setOnClickListener {
                isEnabled = false
                narration.text = "asking…"
                Thread {
                    val why = Kernel.browse()
                    runOnUiThread {
                        narration.text = why ?: "swept."
                        isEnabled = true
                        draw()
                    }
                }.start()
            }
        }
        // **the resource's own page** [2026-10-28], which is where asking
        // one leads now: the diagnostic read of a status and a first line
        // stays beside it, because a status is what a person debugging a
        // node wants and a page is what everybody else does
        val open = Button(this).apply {
            text = "Open that resource"
            setOnClickListener {
                val entry = chosen(which)
                if (entry == null) {
                    answer.text = "no entry held names that resource; sweep, or paste one from the list"
                    return@setOnClickListener
                }
                startActivity(
                    android.content.Intent(this@CatalogActivity, ResourceActivity::class.java)
                        .putExtra("node", entry.node)
                        .putExtra("resource", entry.resource)
                        .putExtra("name", entry.instance)
                        .putExtra("path", path.text.toString().trim().ifBlank { "/" }),
                )
            }
        }
        val ask = Button(this).apply {
            text = "Ask that resource"
            setOnClickListener {
                val entry = chosen(which)
                if (entry == null) {
                    answer.text = "no entry held names that resource; sweep, or paste one from the list"
                    return@setOnClickListener
                }
                isEnabled = false
                answer.text = "asking ${entry.node}…"
                Thread {
                    val got = Kernel.ask(entry.node, entry.resource, path.text.toString().trim())
                    runOnUiThread {
                        answer.text = when {
                            got == null -> "the kernel would not ask"
                            else -> said(got)
                        }
                        isEnabled = true
                    }
                }.start()
            }
        }

        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(Screen.gutter(context), 48, Screen.gutter(context), 48)
            addView(
                TextView(this@CatalogActivity).apply {
                    textSize = 28f
                    typeface = Typeface.DEFAULT_BOLD
                    text = "Catalog"
                },
            )
            addView(narration)
            addView(sweep)
            addView(
                TextView(this@CatalogActivity).apply {
                    setPadding(0, 32, 0, 8)
                    textSize = 12f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Palette.now.sub)
                    text = "What is offered"
                },
            )
            addView(roll)
            addView(
                TextView(this@CatalogActivity).apply {
                    setPadding(0, 32, 0, 8)
                    textSize = 12f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Palette.now.sub)
                    text = "Ask one for something"
                },
            )
            addView(which)
            addView(path)
            addView(open)
            addView(ask)
            addView(answer)
        }
        // **the bars are the frame's, not the scrolling content's**
        // ([`Screen`]): an inset applied inside a `ScrollView`
        // scrolls away, which is how the foot of this screen came to
        // sit under the navigation bar
        val frame = ScrollView(this).apply { addView(body) }
        Screen.insetting(frame)
        setContentView(frame)
    }

    override fun onResume() {
        super.onResume()
        draw()
    }

    /** The catalog entry the pasted keyhash names, or null. */
    private fun chosen(which: EditText): Kernel.Offered? {
        val want = which.text.toString().trim().lowercase()
        return Kernel.catalog().firstOrNull { it.resource == want }
    }

    private fun draw() {
        roll.text = Catalog.roll(Kernel.catalog())
    }

    private fun said(a: Kernel.Answered): String = Catalog.answer(a)
}
