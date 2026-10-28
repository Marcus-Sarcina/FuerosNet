package com.comptus.fueros

/**
 * **What a resource's page may do, and what the person is told when it
 * may not** — the pure part, so a test can hold it
 * (`light-client-requirements.md` §8; the screens sheet's F, and the
 * resource page's frame policy, settled 2026-10-28).
 *
 * **The argument is the node pages' argument, one step further.**
 * [NodePageActivity] frames the node's own administration pages with no
 * script, no bridge, no local reach, no storage and one origin, because a
 * seized node serving a hostile page must reach nothing on the device
 * that still holds the seed (design §18.1, §23.3), and the node cannot
 * assert that isolation, being the party it is against. **A resource is
 * somebody else's code by exactly that argument** — more so: a resource
 * is an application its *owner* packaged, hosted by a node the owner
 * chose, and the person looking at it has decided to trust neither with
 * their device.
 *
 * **The policy, and why each part of it costs the application nothing.**
 *
 *  - **No origin at all.** §11's exchange is one request and one
 *    response carried by the kernel through the hosting node; there is no
 *    URL and no server a page could be loaded from. So the frame is given
 *    a null base, which is the strongest isolation a web view has: an
 *    opaque origin, same-origin with nothing, not even itself.
 *  - **No network.** Nothing in the page can be fetched, because there is
 *    nowhere to fetch it from — a `src` has no base to resolve against —
 *    and the frame blocks loads outright rather than leaving that to
 *    arithmetic. **Whatever a page needs it must carry**, which the one
 *    request and one response already decided.
 *  - **No script.** Not a tightening but a consequence: with no origin,
 *    no network and no bridge, script cannot reach the resource, this
 *    process, or the device. Every exchange goes back through the kernel,
 *    so a form is carried by this shell whether or not a page could run
 *    code. Script would add the largest attack surface a page has in
 *    return for local interactivity alone.
 *  - **No bridge.** Nothing is published into the page, which is the one
 *    mechanism by which it could call into this process.
 *  - **No storage, and nothing kept.** Nothing a page leaves behind
 *    outlives the screen.
 *  - **No navigation.** With no origin there is no origin to stay
 *    within, so every navigation is refused and the frame says so.
 *
 * **What the declared type decides, and what nothing decides.** The
 * kernel reads the one declared media type ([Kernel.Shape]); a page is
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
    const val FRAME_SAYS =
        "The resource's own page, in a frame that reaches nothing: no script, " +
            "no network, no storage, and nothing of yours."

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
        /** Frame this markup. */
        data class Page(val html: String) : Showing()

        /** Show this text, with no frame: text is not a page. */
        data class Text(val text: String) : Showing()

        /** Show this sentence and render nothing. */
        data class Said(val why: String) : Showing()
    }

    /**
     * **The whole of the policy as one decision.** A screen that branched
     * on the type itself would be a second place the policy lived, and
     * the two would come apart.
     */
    fun showing(a: Kernel.Answered?): Showing {
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
        val p = a.page ?: return Showing.Said("The resource answered with nothing.")
        return when (p.shape) {
            Kernel.Shape.PAGE -> Showing.Page(styled(String(p.body)))
            Kernel.Shape.TEXT -> Showing.Text(String(p.body))
            Kernel.Shape.OTHER -> Showing.Said(said(p))
        }
    }

    /**
     * What an unrenderable answer is called. **The declared type and the
     * size, never a guess**: a person told *this is a PDF of 40 kB* knows
     * what happened, where *could not display* tells them nothing.
     */
    fun said(p: Kernel.Page): String {
        val what = p.media ?: "nothing"
        return "The resource answered with $what, ${p.body.size} bytes, which this app " +
            "does not display. It is not guessed at: the declared type decides, and " +
            "a body read to find out what it is arrives as something you did not agree to see."
    }
}
