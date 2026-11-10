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
        // **A phrase owed outranks everything** ([owedPhrase]): it is the
        // one thing here that cannot be shown again, and every other
        // branch reads device state that a recreation reads the same way.
        // **[ADDING] is tried after the name step**, for the matching
        // reason: the intent survives a recreation, so an add that had
        // reached the name would come back as the choice of new or
        // restored and read as a wipe.
        when {
            owedPhrase != null -> shown(Kernel.materialHex(), owedPhrase)
            !Kernel.hasIdentity(this) -> choice()
            intent.getBooleanExtra(NAME_ONLY, false) || Kernel.defaultAlias() == null -> name()
            // **the picker already asked which way**, so neither door is
            // offered a second time
            intent.getBooleanExtra(GENERATE, false) -> create()
            intent.getBooleanExtra(RESTORE, false) -> restore()
            intent.getBooleanExtra(ADDING, false) -> choice()
            else -> {
                startActivity(Intent(this, ShellActivity::class.java))
                finish()
            }
        }
    }

    companion object {
        /** The extra that opens this screen at the name step alone. */
        const val NAME_ONLY = "name_only"

        /**
         * The extra that opens the choice although this device already
         * holds an identity: the selector's own last row
         * ([Identities.ANOTHER]) is the only thing that passes it, and
         * both doors behind it add rather than replace
         * ([Kernel.createIdentity]).
         */
        const val ADDING = "adding"

        /**
         * **The phrase this screen still owes somebody** [bench,
         * 2026-11-10], held for the process and nowhere else.
         *
         * **The defect it answers.** The routing below reads what the
         * device holds, and the phrase lived in one activity's memory: a
         * recreation — the soft keyboard is enough — came back, found an
         * identity whose alias was not set yet, and went to the name step,
         * **skipping the one screen the phrase is ever shown on**. It
         * happened on GXV to the second identity of run
         * `18521b9-new-universe-1`, whose phrase is gone. The opposite
         * ordering has the same shape of failure: a recreation mid-add
         * comes back as the choice of new or restored, which reads as a
         * wipe.
         *
         * **So a phrase owed outranks every other route**, and nothing
         * clears it but the person saying they have written it down.
         *
         * **In memory and never in the saved state.** A `Bundle` is
         * written where the system pleases, including a recents snapshot,
         * and this is the whole of an identity. Process death still loses
         * it — nothing held outside the process could be safe — and that
         * is the one case this cannot cover.
         */
        @Volatile
        var owedPhrase: List<String>? = null

        /**
         * **Which way the picker chose** [author, 2026-11-10]: its
         * `<New Identity>` row asks *generate* or *from a seed phrase*
         * before anything opens, so this screen goes straight to the one
         * that was chosen and the choice is not put twice.
         */
        const val GENERATE = "generate"

        /** The other way, by the same route. */
        const val RESTORE = "restore"

        /** The one request code this screen opens a file under. */
        private const val OPEN_BACKUP = 7449
    }

    // ---- the choice ----------------------------------------------------

    private fun choice() {
        body.removeAllViews()
        title("Fueros")
        para(
            if (Kernel.hasIdentity(this)) {
                "Another identity on this device. Start a new one, or restore one " +
                    "from the phrase you wrote down when it was made. What this " +
                    "device already holds stays held."
            } else {
                "This device has no identity yet. Start a new one, or restore one " +
                    "from the phrase you wrote down when it was made."
            },
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
        // **owed before the kernel is touched**: everything after this
        // line can recreate this activity, and from here on a recreation
        // redraws the phrase rather than routing past it
        owedPhrase = phrase
        if (!Kernel.createIdentity(this, seeds)) {
            owedPhrase = null
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
        if (phrase == null) restoredMaterial = material
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
        }
        heading("Public key material")
        para("What a node is told about you. The bench reads it from here.")
        mono(material ?: "the kernel did not come up in time; the material will show on a later start")
        // **the way on goes after both, never between them** [2026-11-03].
        // It used to sit under the phrase and above the material, so on a
        // short screen the only control was reached before the second
        // half of what the screen exists to show — and tapping it was how
        // a tester got past the material without seeing it.
        // **the archive is the half a phrase does not bring back**
        // [2026-11-05], and the kernel has always been able to open a
        // backup while the shell had no door. Offered on the restored path
        // alone: a brand-new identity has no archive of its own to miss,
        // and a backup made by somebody else would not open under it.
        if (phrase == null) {
            heading("The records")
            para(
                "Your identity is back. What a device loss also took is the history: " +
                    "the records of what you did and who you met. If you kept a backup, " +
                    "it opens under the identity you just restored.",
            )
            // PRD-09: what a restored archive is, in the author's words
            for (said in Disclosure.words(Disclosure.Topic.RESTORE)) {
                para(said)
            }
            restoreSaid?.let { para(it) }
            button("Open a backup file") { openBackup() }
        }
        button(if (phrase == null) "Continue" else "I have written them down") {
            // **the only thing that clears it** ([owedPhrase]): the person
            // saying they have it, not a redraw and not a route
            owedPhrase = null
            name()
        }
    }

    // ---- the archive, from a backup the person kept ---------------------

    /** What the import came to, across a redraw of [shown]. */
    private var restoreSaid: String? = null

    /** Whether this screen reached [shown] by restoring rather than creating. */
    private var restoredMaterial: String? = null

    private fun openBackup() {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("*/*")
        runCatching {
            @Suppress("DEPRECATION")
            startActivityForResult(intent, OPEN_BACKUP)
        }.onFailure {
            restoreSaid = "This device offers no way to pick a file."
            shown(restoredMaterial, null)
        }
    }

    @Deprecated("the shell is on startActivityForResult throughout")
    override fun onActivityResult(request: Int, result: Int, data: Intent?) {
        @Suppress("DEPRECATION")
        super.onActivityResult(request, result, data)
        if (request != OPEN_BACKUP) return
        val uri = data?.data ?: return
        val blob = runCatching {
            contentResolver.openInputStream(uri)?.use { it.readBytes() }
        }.getOrNull()
        if (blob == null) {
            restoreSaid = "That file could not be read, so nothing was imported."
            shown(restoredMaterial, null)
            return
        }
        restoreSaid = "opening it…"
        shown(restoredMaterial, null)
        Thread({
            val out = Kernel.restoreBackup(blob)
            runOnUiThread {
                if (isFinishing) return@runOnUiThread
                restoreSaid = out.fold(
                    // **PRD-07: what the scan discarded is said, not
                    // swallowed**, and the words are [Backup]'s so a test
                    // can hold them without a screen
                    { r -> Backup.imported(r.records, r.discarded) },
                    { e -> "That backup was not taken: ${e.message ?: "the kernel refused it"}" },
                )
                shown(restoredMaterial, null)
            }
        }, "fueros-restore-backup").start()
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
        // **the material again, because the screen that showed it can be
        // lost** [2026-11-03]. `create` brings the kernel up and *then*
        // waits for the material — five seconds on a low-specification
        // phone — and `onCreate` sends an identity with no name straight
        // here. So anything that recreates this activity in that window,
        // which a small-memory device does routinely, took the material
        // screen with it and nothing showed it again.
        //
        // **It is public and derived, so showing it twice costs nothing**
        // — unlike the phrase, which is not stored and genuinely cannot
        // come back. A person who lost the first screen is not asked to
        // wipe the device to see their own public identity.
        Kernel.materialHex()?.let {
            heading("Public key material")
            para("What a node is told about you. The bench reads it from here.")
            mono(it)
        }
        heading("The name people will see")
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
