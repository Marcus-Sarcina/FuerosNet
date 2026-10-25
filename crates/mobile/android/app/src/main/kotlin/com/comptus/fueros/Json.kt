package com.comptus.fueros

/**
 * **A reader of JSON, and nothing more**, for what a hosting provider's
 * API answers ([Launch]). Objects come back as maps, arrays as lists,
 * strings, numbers as doubles, booleans and null as themselves.
 *
 * Written here rather than taken from a library because the platform's
 * own `org.json` is a stub under the unit tests, and the one library that
 * matches it carries a licence this project does not take. A reader of
 * five value kinds is a page, and a page that is tested is cheaper than a
 * dependency that is not.
 */
object Json {
    fun parse(text: String): Any? {
        val p = Parser(text)
        val v = p.value()
        p.skip()
        require(p.at >= text.length) { "trailing characters at ${p.at}" }
        return v
    }

    /** A string for a JSON document: quoted and escaped. */
    fun quote(s: String): String {
        val b = StringBuilder("\"")
        for (c in s) {
            when (c) {
                '"' -> b.append("\\\"")
                '\\' -> b.append("\\\\")
                '\n' -> b.append("\\n")
                '\r' -> b.append("\\r")
                '\t' -> b.append("\\t")
                else -> if (c < ' ') b.append(String.format("\\u%04x", c.code)) else b.append(c)
            }
        }
        return b.append('"').toString()
    }

    @Suppress("UNCHECKED_CAST")
    fun obj(v: Any?): Map<String, Any?> = (v as? Map<String, Any?>) ?: emptyMap()

    fun list(v: Any?): List<Any?> = (v as? List<Any?>) ?: emptyList()

    fun string(v: Any?): String? = v as? String

    fun number(v: Any?): Double? = v as? Double

    fun bool(v: Any?): Boolean? = v as? Boolean

    private class Parser(val s: String) {
        var at = 0

        fun skip() {
            while (at < s.length && s[at].isWhitespace()) at++
        }

        fun value(): Any? {
            skip()
            require(at < s.length) { "unexpected end" }
            return when (s[at]) {
                '{' -> obj()
                '[' -> arr()
                '"' -> str()
                't' -> lit("true", true)
                'f' -> lit("false", false)
                'n' -> lit("null", null)
                else -> num()
            }
        }

        private fun lit(word: String, v: Any?): Any? {
            require(s.startsWith(word, at)) { "bad literal at $at" }
            at += word.length
            return v
        }

        private fun num(): Double {
            val start = at
            while (at < s.length && (s[at].isDigit() || s[at] in "+-.eE")) at++
            return s.substring(start, at).toDoubleOrNull() ?: error("bad number at $start")
        }

        private fun str(): String {
            require(s[at] == '"')
            at++
            val b = StringBuilder()
            while (true) {
                require(at < s.length) { "unterminated string" }
                val c = s[at++]
                when (c) {
                    '"' -> return b.toString()
                    '\\' -> {
                        val e = s[at++]
                        when (e) {
                            '"' -> b.append('"')
                            '\\' -> b.append('\\')
                            '/' -> b.append('/')
                            'b' -> b.append(8.toChar())
                            'f' -> b.append(12.toChar())
                            'n' -> b.append('\n')
                            'r' -> b.append('\r')
                            't' -> b.append('\t')
                            'u' -> {
                                b.append(s.substring(at, at + 4).toInt(16).toChar())
                                at += 4
                            }
                            else -> error("bad escape at $at")
                        }
                    }
                    else -> b.append(c)
                }
            }
        }

        private fun obj(): Map<String, Any?> {
            require(s[at] == '{')
            at++
            val m = LinkedHashMap<String, Any?>()
            skip()
            if (s[at] == '}') { at++; return m }
            while (true) {
                skip()
                val k = str()
                skip()
                require(s[at] == ':') { "expected : at $at" }
                at++
                m[k] = value()
                skip()
                when (s[at]) {
                    ',' -> at++
                    '}' -> { at++; return m }
                    else -> error("expected , or } at $at")
                }
            }
        }

        private fun arr(): List<Any?> {
            require(s[at] == '[')
            at++
            val l = ArrayList<Any?>()
            skip()
            if (s[at] == ']') { at++; return l }
            while (true) {
                l.add(value())
                skip()
                when (s[at]) {
                    ',' -> at++
                    ']' -> { at++; return l }
                    else -> error("expected , or ] at $at")
                }
            }
        }
    }
}
