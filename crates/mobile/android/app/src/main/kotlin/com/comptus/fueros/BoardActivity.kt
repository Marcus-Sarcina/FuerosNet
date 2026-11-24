package com.comptus.fueros

import android.app.Activity
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast

/**
 * **One thread of a subnet's board, or a thread being begun**
 * (`light-client-requirements.md` §4.5) [author, 2026-10-27: a thread
 * takes the whole screen].
 *
 * A thread is laid out as a conversation is: the box at the foot and the
 * posts stacked up from it, this person's to the right in the colour of
 * the identity they speak as, each saying who wrote it, how far it
 * travels as read from where this person stands, and when. **A reply
 * states its thread's reach and offers none**; a new thread offers `+2/-2`
 * and moves either count a level at a time.
 *
 * What is drawn is the kernel's own pool, read through [Gateway] and
 * redrawn when the kernel says the board changed ([Model]).
 */
class BoardActivity : Activity() {

    private lateinit var by: String
    private lateinit var subnet: String

    /** The thread shown, or null while one is being begun. */
    private var thread: String? = null

    private lateinit var posts: LinearLayout
    private lateinit var scroll: ScrollView
    private lateinit var heading: TextView
    private lateinit var travels: TextView

    private var up = Chrome.DEFAULT_REACH
    private var down = Chrome.DEFAULT_REACH

    private val watcher = Model.Watcher { nouns ->
        if (uniffi.rhtn_model.Noun.BOARD in nouns) redraw()
    }

    private val colours: Map<String, Int> by lazy { Identities.identityColours(this) }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // **a board is one identity's in one subnet, named by the row that
        // opened it**: opened without either is a defect, not a default
        val (b, s) = Pair(intent.getStringExtra("by"), intent.getStringExtra("subnet"))
        if (b == null || s == null) {
            Diag.warn("board.opened", "why" to "no row named the identity and the subnet")
            finish()
            return
        }
        by = b
        subnet = s
        thread = intent.getStringExtra("thread")
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(Screen.gutter(context), 24, Screen.gutter(context), 16)
        }
        heading = TextView(this).apply {
            textSize = 22f
            typeface = Type.bold
            maxLines = 2
            setPadding(0, 0, 0, px(8))
        }
        root.addView(heading)
        if (thread == null) begin(root) else continueThread(root)
        Screen.insetting(root)
        setContentView(root)
    }

    /** A thread being begun: a title, the words, and how far it travels. */
    private fun begin(root: LinearLayout) {
        heading.text = "New thread"
        val title = EditText(this).apply {
            hint = "Title"
            maxLines = 1
            filters = arrayOf(android.text.InputFilter.LengthFilter(128))
        }
        val words = EditText(this).apply {
            hint = "What you want to say"
            minLines = 4
            gravity = Gravity.TOP
            layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f)
        }
        val says = TextView(this).apply {
            textSize = 14f
            setTextColor(Palette.now.sub)
            setPadding(0, px(8), 0, px(8))
        }
        val reach = TextView(this).apply {
            textSize = 20f
            typeface = Type.mono
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }
        fun say() {
            reach.text = Chrome.reach(up, down)
            says.text = "It reaches $up ${levels(up)} above you, with each one's direct " +
                "subordinates, and $down ${levels(down)} below you."
        }
        fun stepper(label: String, move: () -> Unit) = Button(this).apply {
            text = label
            setOnClickListener {
                move()
                say()
            }
        }
        val steps = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(stepper("up −") { up = Chrome.step(up, -1) })
            addView(stepper("up +") { up = Chrome.step(up, 1) })
            addView(reach)
            addView(stepper("down −") { down = Chrome.step(down, -1) })
            addView(stepper("down +") { down = Chrome.step(down, 1) })
        }
        val post = Button(this).apply { text = "Post" }
        post.setOnClickListener {
            val (t, w) = Pair(title.text.toString(), words.text.toString())
            if (t.isBlank() || w.isBlank()) {
                Toast.makeText(this, "A title and something to say, both.", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            post.isEnabled = false
            Thread({
                val why = Gateway.boardPost(by, subnet, up, down, t, w)
                runOnUiThread {
                    if (why == null) {
                        finish()
                    } else {
                        post.isEnabled = true
                        Toast.makeText(this, "Not posted: $why", Toast.LENGTH_LONG).show()
                    }
                }
            }, "fueros-board-post").start()
        }
        say()
        root.addView(title)
        root.addView(words)
        root.addView(steps)
        root.addView(says)
        root.addView(post)
    }

    /** A thread shown: its posts stacked up from the box. */
    private fun continueThread(root: LinearLayout) {
        posts = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.BOTTOM
        }
        scroll = ScrollView(this).apply {
            isFillViewport = true
            addView(posts, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.MATCH_PARENT))
            layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f)
        }
        travels = TextView(this).apply {
            textSize = 12f
            setTextColor(Palette.now.sub)
        }
        val box = EditText(this).apply {
            hint = "reply"
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }
        val send = Button(this).apply { text = "Reply" }
        send.setOnClickListener {
            val text = box.text.toString()
            val t = thread
            if (text.isBlank() || t == null) return@setOnClickListener
            box.setText("")
            Thread({
                val why = Gateway.boardReply(by, subnet, t, text)
                if (why != null) {
                    runOnUiThread {
                        box.setText(text)
                        Toast.makeText(this, "Not posted: $why", Toast.LENGTH_LONG).show()
                    }
                }
            }, "fueros-board-reply").start()
        }
        root.addView(scroll)
        root.addView(travels)
        root.addView(
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                addView(box)
                addView(send)
            },
        )
    }

    override fun onStart() {
        super.onStart()
        if (thread != null) {
            Model.watch(watcher)
            redraw()
        }
    }

    override fun onStop() {
        if (thread != null) Model.unwatch(watcher)
        super.onStop()
    }

    /** Read the thread off the drawing thread, mark it shown, and draw it. */
    private fun redraw() {
        val t = thread ?: return
        Thread({
            val held = Gateway.boardThread(by, subnet, t)
            // **on screen, what it holds of the thread is read**
            if (held.any { !it.read }) Gateway.boardRead(by, subnet, t)
            val title = Model.boards[Chrome.boardKey(by, subnet)].orEmpty()
                .firstOrNull { it.id.hex() == t }?.title
            runOnUiThread {
                if (isFinishing) return@runOnUiThread
                draw(title, held)
            }
        }, "fueros-board").start()
    }

    private fun draw(title: String?, held: List<uniffi.rhtn_model.BoardPost>) {
        heading.text = title ?: "A thread begun out of your reach"
        posts.removeAllViews()
        if (held.isEmpty()) {
            posts.addView(
                TextView(this).apply {
                    text = "Nothing of this thread is held."
                    textSize = 14f
                    setTextColor(Palette.now.sub)
                },
            )
        }
        val now = System.currentTimeMillis() / 1000
        held.forEach { posts.addView(bubble(it, now)) }
        // **a reply states its thread's reach and offers none** [author,
        // 2026-10-27], read from where this person stands
        travels.text = held.firstOrNull()
            ?.let { "It travels as the thread does: ${Chrome.reach(it.up.toInt(), it.down.toInt())}" }
            ?: ""
        scroll.post { scroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }

    /** One post: this person's to the right, anyone else's to the left. */
    private fun bubble(p: uniffi.rhtn_model.BoardPost, now: Long): View {
        val c = Palette.now
        val key = p.writer.hex()
        val who = Chrome.writer(Model.aliases[key], p.alias, key)
        val inner = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(px(12), px(8), px(12), px(8))
            background = GradientDrawable().apply {
                cornerRadius = px(14).toFloat()
                if (p.mine) {
                    setColor(c.control)
                    setStroke(px(2), Identities.colourOf(colours, by))
                } else {
                    setColor(Palette.mix(Chrome.personColour(null, key).second, c.ground, Chrome.BUBBLE_SHARE))
                }
            }
            addView(
                TextView(this@BoardActivity).apply {
                    text = "$who · ${Chrome.reach(p.up.toInt(), p.down.toInt())} · " +
                        Chrome.ago(now, p.writtenAt.toLong())
                    textSize = 12f
                    typeface = Type.bold
                    setTextColor(c.muted)
                },
            )
            addView(
                TextView(this@BoardActivity).apply {
                    text = p.text
                    textSize = 16f
                    setTextColor(c.ink)
                },
            )
        }
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = if (p.mine) Gravity.END else Gravity.START
            setPadding(if (p.mine) px(48) else 0, px(4), if (p.mine) 0 else px(48), px(4))
            addView(inner, LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT))
        }
    }

    private fun levels(n: Int): String = if (n == 1) "level" else "levels"

    private fun px(v: Int): Int = (v * resources.displayMetrics.density).toInt()
}
