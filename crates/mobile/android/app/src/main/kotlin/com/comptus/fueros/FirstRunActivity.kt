package com.comptus.fueros

import android.app.Activity
import android.content.Intent
import android.graphics.Typeface
import android.os.Bundle
import android.text.InputType
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.MultiAutoCompleteTextView
import android.widget.ScrollView
import android.widget.TextView
import java.security.SecureRandom

/**
 * **The first run: a new identity, or one restored from its phrase**
 * [author, 2026-10-29] (the screens sheet's A1–A2;
 * `light-client-requirements.md` §9).
 *
 * The shell minted the seeds silently at the first start until now. The
 * author's ruling replaces that with a choice, and makes the key material
 * and the phrase the thing the person sees first — which is also where
 * the bench reads a fresh phone's material from, the Conversations screen
 * that showed it having gone with the old Home.
 *
 * **Three steps, one screen each.**
 *
 *  - **The choice**: start a new identity, or restore one from its seed
 *    phrase.
 *  - **New**: 32 bytes are minted here, the kernel's two seeds derived
 *    from them ([Identity.seeds]) and sealed under the shell's own
 *    custody, the kernel brought up on them, and the screen shows the
 *    public key material and the 24 words — **once**, as §9 requires;
 *    nothing redisplays them. **Restore**: the words typed with the
 *    standard list completing each, read back into the same master and
 *    the same seeds, so the identity is the one the phrase was written
 *    for.
 *  - **The name**: a default alias, required, which the kernel keeps and
 *    publishes to every subnet the person has not named themselves
 *    differently in.
 *
 * What the kernel exports — restore from a backup envelope, recovery — is
 * still owed a door here (A1's three doors); the phrase is the second of
 * them and the one the author asked for first.
 */
class FirstRunActivity : Activity() {

    private lateinit var body: LinearLayout
    private var words: List<String> = emptyList()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(Screen.gutter(context), 48, Screen.gutter(context), 48)
        }
        val frame = ScrollView(this).apply { addView(body) }
        Screen.insetting(frame)
        setContentView(frame)
        words = runCatching {
            assets.open("bip39/english.txt").bufferedReader().useLines { Identity.wordlist(it) }
        }.getOrDefault(emptyList())
        // **an identity with no name yet lands on the name step** [2026-10-29]:
        // the seeds are sealed before the material is shown, so a process
        // killed between that screen and the name — the bench's restart
        // does exactly this — has an identity and owes the name still.
        // **Decided by what the device holds, not by the intent that first
        // opened this screen** [bench, 2026-10-30]: the keyboard's
        // appearance recreates this activity on the TCLs, and a restored
        // identity sitting on the name step came back as the choice of new
        // or restored, which reads as a wipe
        when {
            !Kernel.hasIdentity(this) -> choice()
            intent.getBooleanExtra(NAME_ONLY, false) || Kernel.defaultAlias() == null -> name()
            else -> {
                startActivity(Intent(this, ShellActivity::class.java))
                finish()
            }
        }
    }

    companion object {
        /** The extra that opens this screen at the name step alone. */
        const val NAME_ONLY = "name_only"
    }

    // ---- the choice ----------------------------------------------------

    private fun choice() {
        body.removeAllViews()
        title("Fueros")
        para(
            "This device has no identity yet. Start a new one, or restore one " +
                "from the phrase you wrote down when it was made.",
        )
        button("Start a new identity") { create() }
        button("Restore from a seed phrase") { restore() }
        if (words.size != Identity.LIST) {
            para("The wordlist is not in this build, so neither door can open: ${words.size} words read.")
        }
    }

    // ---- new -----------------------------------------------------------

    private fun create() {
        if (words.size != Identity.LIST) return
        val master = ByteArray(Identity.ENTROPY_BYTES).also { SecureRandom().nextBytes(it) }
        val phrase = Identity.phrase(master, words)
        val seeds = Identity.seeds(master)
        master.fill(0)
        body.removeAllViews()
        title("Your identity")
        para("Bringing the kernel up on it…")
        if (!Kernel.createIdentity(this, seeds)) {
            para("This device's own storage refused the seeds; nothing was made.")
            button("Back") { choice() }
            return
        }
        seeds.fill(0)
        Thread({
            val material = waitForMaterial()
            runOnUiThread { if (!isFinishing) shown(material, phrase) }
        }, "fueros-first-run").start()
    }

    /** The material once the kernel is up, or null after thirty seconds. */
    private fun waitForMaterial(): String? {
        val until = System.currentTimeMillis() + 30_000
        while (System.currentTimeMillis() < until) {
            Kernel.materialHex()?.let { return it }
            Thread.sleep(200)
        }
        return null
    }

    /**
     * **The material and the phrase, once.** The material is this
     * device's public identity and is what a node is told; the phrase is
     * the whole of the private one and is never shown again. **A restore
     * shows the material alone**: the phrase was the person's input, and
     * the material is how they can see it came back as the identity it
     * was written for.
     */
    private fun shown(material: String?, phrase: List<String>?) {
        body.removeAllViews()
        title(if (phrase == null) "Your identity, restored" else "Your identity")
        // **the phrase above the material**: the phrase is what the
        // person must act on and the material is three screens of hex
        // the bench reads; put first, both begin on the first screen,
        // which is also what a screen reader of the bench's kind sees
        if (phrase != null) {
            heading("Your seed phrase")
            para(
                "Write these 24 words down, in order, and keep them apart from this " +
                    "device. They are the whole of your identity: anyone holding them is " +
                    "you, and nothing can show them again.",
            )
            mono(Identity.laidOut(phrase))
            button("I have written them down") { name() }
        }
        heading("Public key material")
        para("What a node is told about you. The bench reads it from here.")
        mono(material ?: "the kernel did not come up in time; the material will show on a later start")
        if (phrase == null) button("Continue") { name() }
    }

    // ---- restore -------------------------------------------------------

    private fun restore() {
        if (words.size != Identity.LIST) return
        body.removeAllViews()
        title("Restore")
        para("The 24 words, in order. Each completes as you type it.")
        val typed = MultiAutoCompleteTextView(this).apply {
            hint = "word word word …"
            textSize = 16f
            typeface = Typeface.MONOSPACE
            inputType = InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS or
                InputType.TYPE_TEXT_FLAG_MULTI_LINE
            threshold = 1
            setAdapter(ArrayAdapter(context, android.R.layout.simple_dropdown_item_1line, words))
            setTokenizer(SpaceTokenizer())
            minLines = 4
        }
        val said = TextView(this).apply {
            textSize = 13f
            setTextColor(Palette.now.warn)
            setPadding(0, 8, 0, 8)
        }
        body.addView(typed)
        body.addView(said)
        button("Restore") {
            val given = typed.text.toString().split(Regex("\\s+"))
            when (val read = Identity.entropy(given, words)) {
                is Identity.Read.Refused -> said.text = read.why
                is Identity.Read.Entropy -> {
                    val seeds = Identity.seeds(read.bytes)
                    read.bytes.fill(0)
                    if (!Kernel.createIdentity(this, seeds)) {
                        said.text = "This device's own storage refused the seeds; nothing was restored."
                    } else {
                        seeds.fill(0)
                        said.text = "bringing the kernel up on it…"
                        Thread({
                            val material = waitForMaterial()
                            runOnUiThread { if (!isFinishing) shown(material, null) }
                        }, "fueros-first-run").start()
                    }
                }
            }
        }
        button("Back") { choice() }
    }

    /** Words are separated by whitespace; the platform offers only commas. */
    private class SpaceTokenizer : MultiAutoCompleteTextView.Tokenizer {
        override fun findTokenStart(text: CharSequence, cursor: Int): Int {
            var i = cursor
            while (i > 0 && !text[i - 1].isWhitespace()) i--
            return i
        }

        override fun findTokenEnd(text: CharSequence, cursor: Int): Int {
            var i = cursor
            while (i < text.length && !text[i].isWhitespace()) i++
            return i
        }

        override fun terminateToken(text: CharSequence): CharSequence =
            if (text.isNotEmpty() && text[text.length - 1].isWhitespace()) text else "$text "
    }

    // ---- the name ------------------------------------------------------

    /**
     * **A default name, required** [author, 2026-10-29]. The kernel keeps
     * it and publishes it per subnet (`light-client-requirements.md`
     * §4.4) wherever the person has set nothing else.
     */
    private fun name() {
        body.removeAllViews()
        title("Your name")
        para(
            "What people will see you as, in every subnet where you do not " +
                "choose a different name. Yours to change later.",
        )
        val field = EditText(this).apply {
            hint = "a name"
            textSize = 18f
            setSingleLine()
        }
        val said = TextView(this).apply {
            textSize = 13f
            setTextColor(Palette.now.warn)
            setPadding(0, 8, 0, 8)
        }
        body.addView(field)
        body.addView(said)
        button("Continue") {
            val typed = field.text.toString().trim()
            Identity.aliasRefusal(typed)?.let { said.text = it; return@button }
            said.text = "keeping it…"
            Thread({
                val why = Kernel.setDefaultAlias(typed)
                runOnUiThread {
                    if (isFinishing) return@runOnUiThread
                    if (why != null) {
                        said.text = why
                    } else {
                        startActivity(Intent(this, ShellActivity::class.java))
                        finish()
                    }
                }
            }, "fueros-alias").start()
        }
    }

    // ---- view helpers --------------------------------------------------

    private fun title(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 26f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 0, 0, 16)
        },
    )

    private fun heading(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 16f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 28, 0, 6)
        },
    )

    private fun para(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 15f
            setTextColor(Palette.now.sub)
            setPadding(0, 6, 0, 10)
        },
    )

    private fun mono(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setTextIsSelectable(true)
            setPadding(0, 6, 0, 10)
        },
    )

    private fun button(label: String, onClick: () -> Unit) = body.addView(
        Button(this).apply {
            text = label
            setOnClickListener { onClick() }
        },
    )
}
