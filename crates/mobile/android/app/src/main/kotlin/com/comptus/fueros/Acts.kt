package com.comptus.fueros

/**
 * **What an instance declares it can be asked for, as this client reads
 * it** (`infra-client-requirements.md` §8.3; `rhtn_daemon::acts`).
 *
 * §8.3 divides the surface and gives this client the frame: *"a node
 * develops and serves its own administration pages"*, and *"a client ships
 * the provisioning pages and no others"*. So there is no screen here for
 * any particular act — a native screen per node version, and per
 * third-party package, is exactly what that rule prevents. **The acts
 * arrive as a declaration and are drawn from it**, the way a node already
 * draws a package's declared operations.
 *
 * **What this client contributes is the asking.** An act that does not
 * arrive on the instance's own loopback proves itself with a MAC over its
 * own body, and HTML in a frame cannot compute one ([Provision.proved]);
 * an act that reaches the machine needs a key derived from the signing
 * seed, and the frame is deliberately isolated from that seed. Both live
 * here.
 *
 * **Nothing in a declaration is trusted beyond its shape.** It comes from
 * a node, and a seized node is a case the design plans for (design §18.1,
 * §23.3): a label is text to show, a bound is a bound to draw, and an act
 * happens when the person says so and not before.
 */
object Acts {

    /** The vocabulary this client reads. */
    const val SURFACE = "rhtn/administration@1"

    /** **The scheme the node's page bounces an act back through.** */
    const val SCHEME = "rhtn-act"

    /** What a parameter holds: the closed set, as the declaration names it. */
    sealed class Kind {
        object Flag : Kind()
        data class Number(val low: Long, val high: Long) : Kind()
        data class Text(val max: Int) : Kind()
        data class Choice(val of: List<String>) : Kind()
        data class Several(val of: List<String>) : Kind()
        object Keyhash : Kind()

        /** **A kind this client does not draw**, named so it can say so
         *  rather than draw a field whose meaning it is guessing at. */
        data class Unknown(val named: String) : Kind()
    }

    /** One of an act's parameters. */
    data class Parameter(val name: String, val label: String, val kind: Kind)

    /** What the client must supply that the person cannot. */
    enum class Brings {
        NOTHING,

        /** The restricted ssh key for the instance's host, for one act. */
        HOST_KEY,

        /** **Something this client does not know how to bring**, which is
         *  a refusal and not a field left empty: an act is not sent
         *  without what it says it needs. */
        UNKNOWN,
    }

    /** One act, as declared. */
    data class Act(
        val name: String,
        val label: String,
        val help: String,
        /** `node` or `resource`. */
        val about: String,
        val brings: Brings,
        val parameters: List<Parameter>,
    )

    /** One resource's acts, with the resource they are about. */
    data class Hosted(
        val resource: String,
        val authority: String,
        val installed: Boolean,
        val acts: List<Act>,
    )

    /** A whole declaration. */
    data class Declared(
        val surface: String,
        val node: List<Act>,
        val resources: List<Hosted>,
    ) {
        /** Whether this client reads the vocabulary it is written in. */
        val understood: Boolean get() = surface == SURFACE

        /** One act by name, wherever it was declared. */
        fun act(name: String): Act? =
            node.firstOrNull { it.name == name }
                ?: resources.firstNotNullOfOrNull { h -> h.acts.firstOrNull { it.name == name } }
    }

    /** Read a declaration out of what the surface sent. */
    fun read(text: String): Result<Declared> = runCatching {
        val top = Json.obj(Json.parse(text))
        Declared(
            surface = Json.string(top["surface"]) ?: "",
            node = Json.list(top["node"]).map { act(it) },
            resources = Json.list(top["resources"]).map { r ->
                val o = Json.obj(r)
                Hosted(
                    resource = Json.string(o["resource"]) ?: "",
                    authority = Json.string(o["authority"]) ?: "",
                    installed = Json.bool(o["installed"]) ?: false,
                    acts = Json.list(o["acts"]).map { act(it) },
                )
            },
        )
    }

    private fun act(v: Any?): Act {
        val o = Json.obj(v)
        return Act(
            name = Json.string(o["name"]) ?: "",
            label = Json.string(o["label"]) ?: "",
            help = Json.string(o["help"]) ?: "",
            about = Json.string(o["about"]) ?: "node",
            brings = when (Json.string(o["brings"])) {
                "nothing" -> Brings.NOTHING
                "host-key" -> Brings.HOST_KEY
                else -> Brings.UNKNOWN
            },
            parameters = Json.list(o["parameters"]).map { parameter(it) },
        )
    }

    private fun parameter(v: Any?): Parameter {
        val o = Json.obj(v)
        val of = { Json.list(o["of"]).mapNotNull { Json.string(it) } }
        val named = Json.string(o["kind"]) ?: ""
        return Parameter(
            name = Json.string(o["name"]) ?: "",
            label = Json.string(o["label"]) ?: "",
            kind = when (named) {
                "flag" -> Kind.Flag
                "number" -> Kind.Number(
                    low = Json.number(o["low"])?.toLong() ?: 0,
                    high = Json.number(o["high"])?.toLong() ?: 0,
                )
                "text" -> Kind.Text(Json.number(o["max"])?.toInt() ?: 0)
                "choice" -> Kind.Choice(of())
                "several" -> Kind.Several(of())
                "keyhash" -> Kind.Keyhash
                else -> Kind.Unknown(named)
            },
        )
    }

    /**
     * **Whether this client can draw and send this act at all**, and why
     * not where it cannot.
     *
     * A client meeting something it does not understand **says so rather
     * than guessing**: a field whose meaning may have moved is not one to
     * fill in, and a credential this client does not know how to derive is
     * not one to omit.
     */
    fun undrawable(a: Act): String? {
        if (a.brings == Brings.UNKNOWN) {
            return "this act needs something this client does not know how to bring; a newer " +
                "client will make it"
        }
        val odd = a.parameters.firstOrNull { it.kind is Kind.Unknown }
        if (odd != null) {
            val named = (odd.kind as Kind.Unknown).named
            return "`${odd.name}` is a `$named`, which this client does not draw; a newer " +
                "client will make this act"
        }
        return null
    }

    /**
     * **What the page's bounce suggested**, as fields of the act.
     *
     * The query of a `rhtn-act:` link, read into the parameters it names
     * and nothing else. **Every value is a hint**: it came from the node's
     * own page, so it fills a field the person confirms and authorises
     * nothing. A name the act does not declare is dropped rather than
     * carried, so a page cannot smuggle a field past the screen.
     */
    fun hints(a: Act, query: String?): Map<String, String> {
        if (query.isNullOrEmpty()) return emptyMap()
        val declared = a.parameters.map { it.name }.toSet()
        val out = LinkedHashMap<String, String>()
        for (pair in query.split('&')) {
            if (pair.isEmpty()) continue
            val at = pair.indexOf('=')
            val k = if (at < 0) pair else pair.substring(0, at)
            val v = if (at < 0) "" else pair.substring(at + 1)
            if (k in declared) out[unescape(k)] = unescape(v)
        }
        return out
    }

    /** The act named in a `rhtn-act:` link, and its query. */
    fun bounced(url: String): Pair<String, String?>? {
        val rest = url.removePrefix("$SCHEME:").takeIf { it != url } ?: return null
        val at = rest.indexOf('?')
        val name = if (at < 0) rest else rest.substring(0, at)
        // **the shape a declared name has**, checked here so a page
        // cannot put a path, a scheme or anything else where an act's
        // name goes: lower-case ASCII, digits, dash and underscore, as a
        // role name is (`rhtn_node::resources::namelike`)
        val namelike = name.all { it in 'a'..'z' || it in '0'..'9' || it == '-' || it == '_' }
        if (name.isEmpty() || name.length > 32 || !namelike) return null
        return name to if (at < 0) null else rest.substring(at + 1)
    }

    /**
     * **Whether an answer is one the instance will take**, checked here so
     * a refusal reaches the person on the screen they are looking at
     * rather than after a round trip.
     *
     * The bounds are the declaration's, which is the point of declaring
     * them: a number outside its range and a choice that is not one of the
     * literals are the instance's refusals, said early.
     */
    fun refused(p: Parameter, given: String): String? = when (val k = p.kind) {
        is Kind.Flag -> if (given == "on" || given == "off") null else "it is on or off"
        is Kind.Number -> {
            val n = given.toLongOrNull()
            when {
                n == null -> "it is a number"
                n < k.low || n > k.high -> "it is between ${k.low} and ${k.high}"
                else -> null
            }
        }
        is Kind.Text ->
            if (given.length > k.max) "at most ${k.max} characters" else null
        is Kind.Choice ->
            if (given in k.of) null else "it is one of ${k.of.joinToString(", ")}"
        is Kind.Several -> {
            val picked = given.split(',').filter { it.isNotEmpty() }
            val stray = picked.firstOrNull { it !in k.of }
            if (stray != null) "`$stray` is not one of ${k.of.joinToString(", ")}" else null
        }
        is Kind.Keyhash ->
            if (given.length == 64 && given.all { it in "0123456789abcdef" }) {
                null
            } else {
                "it is 64 lower-case hex digits"
            }
        is Kind.Unknown -> "this client does not draw a `${k.named}`"
    }

    /**
     * **The body of an invocation**: the declared fields, in the order
     * they were declared, and nothing else.
     *
     * A `several` posts one field per value, which is how a set of roles
     * reaches the instance — the same shape a checkbox group had on the
     * node's own page.
     */
    fun body(a: Act, answers: Map<String, String>): List<Pair<String, String>> {
        val out = mutableListOf<Pair<String, String>>()
        for (p in a.parameters) {
            val given = answers[p.name] ?: continue
            if (p.kind is Kind.Several) {
                for (one in given.split(',').filter { it.isNotEmpty() }) {
                    out.add(p.name to one)
                }
            } else if (given.isNotEmpty()) {
                out.add(p.name to given)
            }
        }
        return out
    }

    /** Percent-decoding, and `+` for a space. */
    fun unescape(s: String): String {
        val out = java.io.ByteArrayOutputStream()
        var i = 0
        while (i < s.length) {
            val c = s[i]
            when {
                c == '+' -> { out.write(' '.code); i += 1 }
                c == '%' && i + 2 < s.length -> {
                    val v = s.substring(i + 1, i + 3).toIntOrNull(16)
                    if (v == null) { out.write('%'.code); i += 1 } else { out.write(v); i += 3 }
                }
                else -> { out.write(c.toString().toByteArray(Charsets.UTF_8)); i += 1 }
            }
        }
        return out.toString("UTF-8")
    }
}
