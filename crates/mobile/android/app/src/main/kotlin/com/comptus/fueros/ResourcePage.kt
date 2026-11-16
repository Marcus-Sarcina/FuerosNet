package com.comptus.fueros

/**
 * **What a resource's page may do, and what the person is told of it**
 * — the pure part, so a test can hold it (`light-client-requirements.md`
 * §8; the screens sheet's F; the frame policy settled 2026-10-28 and
 * relaxed by the author on 2026-11-15).
 *
 * **The argument is the node pages' argument**, and what it forbids is
 * narrower than the first policy drew it. [NodePageActivity] frames the
 * node's own pages with no script, no bridge and one origin, because a
 * seized node serving a hostile page must reach nothing on the device
 * that still holds the seed (design §18.1, §23.3), and the node cannot
 * assert that isolation, being the party it is against. A resource is
 * somebody else's code by exactly that argument. **What the argument
 * forbids is reach into this device**: a bridge into this app, the
 * device's files, another resource. It does not forbid a page running
 * code against its own resource, keeping its own state, or asking its
 * own resource for more — and the first policy forbade those because
 * their cost was taken to be nil, which for an application of any size
 * it is not.
 *
 * **The policy** [author, 2026-11-15]:
 *
 *  - **An origin of its own.** The shell mints one per resource
 *    ([Frames.origin]), a name no network resolves: the page is
 *    same-origin with its own resource and nothing else, and its storage
 *    is its own.
 *  - **The same resource, and nothing else.** Every load and navigation
 *    inside that origin is carried by this screen through the kernel and
 *    the node that served the page — a `GET` as the engine asks for it,
 *    a request with a body through the one object below — and every
 *    other one is answered with nothing. The kernel stays the only
 *    network, and sees every exchange.
 *  - **Script by declaration and consent.** A page runs code only where
 *    its own `content-security-policy` names `script-src` — the
 *    resource's declaration, read by the kernel and handed to the engine
 *    to enforce — and the person has allowed it for that resource, asked
 *    once and remembered ([Frames.scriptAllowed]). A page that declares
 *    nothing runs nothing, whatever its markup carries.
 *  - **One object, no bridge.** A page that runs is given
 *    [ResourceBridge]: a request to the same resource and nothing else.
 *    Nothing of this app is published into it.
 *  - **Storage of its own, kept.** What a page stores under its origin
 *    outlives the screen and belongs to that resource; forgetting the
 *    resource wipes it ([Frames.forget]).
 *  - **No local reach.** File and content access off, both file-URL
 *    escapes off, geolocation off: what a resource may ask of the
 *    device it asks as a typed ask (§8.1), and never from a page.
 *
 * **What the declared type decides, and what nothing decides.** The
 * kernel reads the one declared media type ([Gateway.Shape]); a page is
 * `text/html`, text is `text/plain`, and everything else — including
 * anything undeclared — is named to the person rather than rendered.
 * **Nothing is sniffed.** A body inspected to find out what it is arrives
 * as something nobody agreed to look at.
 *
 * **And the author's styling ruling** [author, 2026-10-27]: *what the
 * resource has not styled for itself, the application styles to match*.
 * So [styled] puts this app's own sheet **before** the page's own markup,
 * where the page's rules win on any property it sets and the app's hold
 * everywhere it does not.
 */
object ResourcePage {

    /** The dark ground, ink and rule the rest of the shell draws on. */
    const val GROUND = "#1C1F24"
    const val INK = "#E8EAED"
    const val MUTED = "#9AA1AA"
    const val RULE = "#23272E"
    const val ACCENT = "#5FB3A1"

    /**
     * **This app's sheet, which the page's own rules override.** Every
     * selector is a bare element: no class, no id, no `!important`, so
     * anything the resource styles for itself wins on specificity alone
     * rather than on order.
     */
    val SHEET = """
        html{background:$GROUND;color:$INK;-webkit-text-size-adjust:100%}
        body{margin:0;padding:16px;font:15px/1.5 -apple-system,Roboto,'Helvetica Neue',Arial,sans-serif}
        h1{font-size:21px;margin:0 0 10px}
        h2{font-size:17px;margin:18px 0 8px}
        h3{font-size:15px;margin:16px 0 6px}
        p,ul,ol,dl,blockquote,pre,table{margin:0 0 12px}
        a{color:$ACCENT}
        small,figcaption{color:$MUTED}
        hr{border:0;border-top:1px solid $RULE;margin:16px 0}
        table{border-collapse:collapse;width:100%}
        th,td{border-bottom:1px solid $RULE;padding:8px 6px;text-align:left}
        th{color:$MUTED;font-weight:600;font-size:13px}
        pre,code,kbd{font-family:'Roboto Mono',monospace;font-size:13px}
        pre{overflow-x:auto;background:#15171A;padding:10px;border-radius:8px}
        img,video,canvas,svg{max-width:100%;height:auto}
        input,select,textarea,button{font:inherit;color:$INK;background:#15171A;border:1px solid #3A3F47;border-radius:8px;padding:9px 10px}
        button{background:#2E3540;min-height:44px}
        blockquote{border-left:2px solid $RULE;padding-left:11px;color:$MUTED}
    """.trimIndent()

    /**
     * The page as the frame loads it: this app's sheet, then the
     * resource's own markup unaltered.
     *
     * **The markup is not rewritten.** A client that edited somebody
     * else's document to make it safe would be asserting it understood
     * the document; the frame's refusals do not depend on understanding
     * it at all.
     */
    fun styled(markup: String): String =
        "<!doctype html><html><head><meta name=\"viewport\" " +
            "content=\"width=device-width,initial-scale=1\">" +
            "<style>$SHEET</style></head><body>$markup</body></html>"

    /** The line under the title: where this page came from. */
    fun provenance(resource: String, node: String): String =
        "served for ${resource.take(12)}… by ${node.take(12)}…"

    /**
     * **What the frame is, said to the person.** Shown with every page,
     * because the containment is the reason the page can be shown at all.
     */
    fun frameSays(script: Boolean): String =
        if (script) {
            "The resource's own page, in a frame of its own: it reaches this resource " +
                "and nothing else, keeps its own storage, and runs its code, as you allowed."
        } else {
            "The resource's own page, in a frame of its own: it reaches this resource " +
                "and nothing else, runs no code, and reaches nothing of yours."
        }

    /**
     * **The question, once per resource**, where a page declares that it
     * runs code and the person has not answered yet.
     */
    const val SCRIPT_ASKS: String =
        "This resource's pages run their own code. In this frame, code reaches the " +
            "resource it came from and nothing else — not this app, not your files, not " +
            "another resource — and keeps storage of its own on this device. " +
            "Allow it for this resource? You can forget the resource later."

    /** §11.1's six statuses, in words. */
    fun delivery(status: ULong): String = when (status.toInt()) {
        0 -> "delivered"
        1 -> "refused"
        2 -> "the resource is unavailable"
        3 -> "the request was malformed"
        4 -> "no subtree acknowledgement covers you"
        5 -> "no role of yours matches"
        else -> "status $status"
    }

    /** What the screen does with an answer. */
    sealed class Showing {
        /** Frame this markup, as the page it came from. */
        data class Page(val html: String, val page: Gateway.Page) : Showing()

        /** Show this text, with no frame: text is not a page. */
        data class Text(val text: String) : Showing()

        /** Show this sentence and render nothing. */
        data class Said(val why: String) : Showing()

        /** **Put this ask to the person**: a question, not a page
         *  (`light-client-requirements.md` §8.1). */
        data class Ask(val ask: Gateway.Ask) : Showing()
    }

    /**
     * **What the person is asked, in words**: the utility named as the
     * thing it would hand over, the resource's purpose and retention in
     * its own words and marked as such, and where what they give goes.
     * The kernel's reading decides the kind; this only words it.
     */
    fun asking(a: Gateway.Ask): String {
        val what = when (a.utility) {
            "scan" -> "a document scanned with the camera"
            "capture" -> "a photograph or video taken now, with the time it was taken and a claim, signed by this device, that it took it"
            "location" -> "your place, as a geohash of ${a.precision ?: 0} characters, sent once"
            "check-in" -> "a check-in: that you are at a place, read from a tag there"
            "tag" -> "a tag read: the code on a thing"
            "sign" -> "a signature over ${a.payload?.size ?: 0} bytes it supplied, under this device's key for it"
            "live" -> "a live audio/video session of ${a.seconds ?: 0} seconds"
            "notify" -> "standing permission to show you its notifications"
            "export" -> "a copy of what it holds, handed to you as a file"
            "import" -> "a file you pick, handed to it"
            else -> a.utility
        }
        return "This resource asks for $what.\n\n" +
            "It says it is for: \u201c${a.purpose}\u201d\n" +
            "It says it keeps it: \u201c${a.retention}\u201d\n\n" +
            "Those are its words, not promises this app can hold it to. " +
            "What you give goes back to this resource and nowhere else, through the node that served it, " +
            "as something you submitted. Your network identity is not in it: a claim is signed under a key " +
            "made for this resource alone. Declining is an answer it sees."
    }

    /** **Whether this device can give a utility is [Giving]'s**, since it
     *  depends on what the device has; this file holds what the person is
     *  told, which does not. */

    /**
     * **The whole of the policy as one decision.** A screen that branched
     * on the type itself would be a second place the policy lived, and
     * the two would come apart.
     */
    fun showing(a: Gateway.Answered?): Showing {
        if (a == null) return Showing.Said("The kernel did not carry the request.")
        if (a.status.toInt() != 0) {
            return Showing.Said("Not delivered: ${delivery(a.status)}.")
        }
        a.unreadable?.let {
            return Showing.Said(
                "The resource answered, and the answer could not be read: $it. " +
                    "Nothing of it is shown rather than some of it.",
            )
        }
        a.ask?.let { return Showing.Ask(it) }
        val p = a.page ?: return Showing.Said("The resource answered with nothing.")
        return when (p.shape) {
            Gateway.Shape.PAGE -> Showing.Page(styled(String(p.body)), p)
            Gateway.Shape.TEXT -> Showing.Text(String(p.body))
            Gateway.Shape.OTHER -> Showing.Said(said(p))
        }
    }

    /**
     * **Whether this page runs**, as one decision: the page's own
     * declaration, and the person's answer for the resource — `null` says
     * the question is owed before the page is framed.
     */
    fun runs(page: Gateway.Page, allowed: Boolean?): Boolean? =
        if (!page.runsScript) false else allowed

    /**
     * What an unrenderable answer is called. **The declared type and the
     * size, never a guess**: a person told *this is a PDF of 40 kB* knows
     * what happened, where *could not display* tells them nothing.
     */
    fun said(p: Gateway.Page): String {
        val what = p.media ?: "nothing"
        return "The resource answered with $what, ${p.body.size} bytes, which this app " +
            "does not display. It is not guessed at: the declared type decides, and " +
            "a body read to find out what it is arrives as something you did not agree to see."
    }
}
