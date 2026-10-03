package com.comptus.fueros

import java.security.SecureRandom

/**
 * The shell's diagnostic events (`Robot/field-test-diagnostics.md`,
 * section 3.6), rendered as the kernel renders its own: one JSON object per
 * line with `ms` since process start, `level`, `layer`, `event`, then the
 * event's fields in the order given. The merge tool reads one format
 * whichever side wrote the line.
 *
 * **No Android in this file**, so `Meet`, `Bearer` and `Carriage` can raise
 * events and still be held by a JVM test. The sink is installed by
 * `FuerosApp` in the fieldtest flavour and by a test; with none installed
 * every call returns at its first branch, which is the releasable flavour.
 *
 * **What a field may hold** follows the plan's section 2: variants, counts,
 * sizes, durations, step names, reasons the code already produces, and
 * identities as eight hex characters. [id8] is the constructor for an
 * identity; [scrub] shortens any longer hex run in a free-text line, so a
 * note the screen shows with sixteen characters reaches the file with
 * eight. Nothing here takes a frame, a key, a seed or a payload, and the
 * callers are written not to hand one over.
 */
object Diag {

    /** Where the events go, or nothing. */
    @Volatile private var sink: ((String) -> Unit)? = null
    @Volatile private var flusher: (() -> Unit)? = null
    @Volatile private var lister: (() -> List<java.io.File>)? = null

    /** The run: this process's name in the bundle, made once at class
     *  load, which is the first use in `FuerosApp.onCreate`. Wall-clock
     *  seconds plus six random hex characters: sortable, and two phones
     *  started in the same second do not collide. */
    val runId: String = run {
        val r = SecureRandom()
        val suffix = (0 until 3).joinToString("") { "%02x".format(r.nextInt(256)) }
        "%d-%s".format(System.currentTimeMillis() / 1000, suffix)
    }

    /** The process's start on the wall clock, the bundle's one anchor. */
    val startedWallMs: Long = System.currentTimeMillis()
    private val startedNanos: Long = System.nanoTime()

    /** Milliseconds since this object was first used, which `FuerosApp`
     *  makes the process's start. */
    fun ms(): Long = (System.nanoTime() - startedNanos) / 1_000_000

    /** Whether anything hears. Callers with a cost to avoid ask first. */
    fun enabled(): Boolean = sink != null

    fun install(
        sink: (String) -> Unit,
        flush: () -> Unit = {},
        files: () -> List<java.io.File> = { listOf() },
    ) {
        this.sink = sink
        this.flusher = flush
        this.lister = files
    }

    fun uninstall() {
        sink = null
        flusher = null
        lister = null
    }

    /** An informational event on the shell's layer. */
    fun event(name: String, vararg fields: Pair<String, Any?>) = emit("info", "shell", name, fields)

    /** A refusal, an error, a timeout: what a tester reads first. */
    fun warn(name: String, vararg fields: Pair<String, Any?>) = emit("warn", "shell", name, fields)

    /** The chatty tier: frames, packets. */
    fun debug(name: String, vararg fields: Pair<String, Any?>) = emit("debug", "shell", name, fields)

    /** A line the kernel rendered already (`Diagnostics.event`): appended
     *  as it is. */
    fun kernel(line: String) {
        sink?.invoke(line)
    }

    /** Drain what is queued to disk, for the crash handler. */
    fun flush() {
        flusher?.invoke()
    }

    /** The event files, current first, for the bundle. */
    fun files(): List<java.io.File> = lister?.invoke() ?: listOf()

    private fun emit(level: String, layer: String, name: String, fields: Array<out Pair<String, Any?>>) {
        val out = sink ?: return
        out(render(ms(), level, layer, name, fields.asList()))
    }

    /**
     * One event as one line: the kernel's field order, the fields given
     * after, a null value left out. Visible for the tests and for
     * `Report`'s header.
     */
    fun render(
        ms: Long,
        level: String,
        layer: String,
        name: String,
        fields: List<Pair<String, Any?>>,
    ): String {
        val sb = StringBuilder(128)
        sb.append("{\"ms\":").append(ms)
        sb.append(",\"level\":").append(json(level))
        sb.append(",\"layer\":").append(json(layer))
        sb.append(",\"event\":").append(json(name))
        for ((k, v) in fields) {
            if (v == null) continue
            sb.append(',').append(json(k)).append(':').append(json(v))
        }
        sb.append('}')
        return sb.toString()
    }

    /** A JSON object from pairs, for a header; nested values allowed. */
    fun obj(fields: List<Pair<String, Any?>>): String {
        val sb = StringBuilder()
        sb.append('{')
        var first = true
        for ((k, v) in fields) {
            if (v == null) continue
            if (!first) sb.append(',')
            first = false
            sb.append(json(k)).append(':').append(json(v))
        }
        sb.append('}')
        return sb.toString()
    }

    /** A value as JSON: numbers and booleans as they are, strings quoted
     *  and escaped, lists as arrays, maps and pair lists as objects. */
    fun json(v: Any?): String = when (v) {
        null -> "null"
        is Boolean -> v.toString()
        is Int, is Long, is Short, is Byte -> v.toString()
        is UInt, is ULong -> v.toString()
        is Double, is Float -> if (v.toDouble().isFinite()) v.toString() else "null"
        is String -> quote(v)
        is Enum<*> -> quote(v.name)
        is Map<*, *> -> obj(v.entries.map { it.key.toString() to it.value })
        is List<*> -> v.joinToString(",", "[", "]") { json(it) }
        is Array<*> -> v.joinToString(",", "[", "]") { json(it) }
        else -> quote(v.toString())
    }

    private fun quote(s: String): String {
        val sb = StringBuilder(s.length + 2)
        sb.append('"')
        for (c in s) {
            when (c) {
                '"' -> sb.append("\\\"")
                '\\' -> sb.append("\\\\")
                '\n' -> sb.append("\\n")
                '\r' -> sb.append("\\r")
                '\t' -> sb.append("\\t")
                else -> if (c < ' ') sb.append("\\u%04x".format(c.code)) else sb.append(c)
            }
        }
        sb.append('"')
        return sb.toString()
    }

    /** An identity as its first eight hex characters, the plan's
     *  truncation; shorter input is shown whole. */
    fun id8(bytes: ByteArray?): String? =
        bytes?.take(4)?.joinToString("") { "%02x".format(it) }

    /** The same from a hex string. */
    fun id8(hex: String?): String? = hex?.take(8)

    private val longHex = Regex("[0-9a-fA-F]{16,}")

    /** A free-text line with every hex run of sixteen or more characters
     *  cut to its first eight: a note or a reason that quoted an identity
     *  reaches the file truncated as an identity field would be. */
    fun scrub(line: String): String = longHex.replace(line) { it.value.take(8) }
}
