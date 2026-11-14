package com.comptus.fueros

/**
 * **An identity's conversations at rest** (`light-client-requirements.md`
 * §9, APP-015) [author, 2026-11-14]: the shell's own files, sealed through
 * the kernel under a key the shell never holds. One file per thread in the
 * identity's scope, each message appended as it accumulates rather than
 * the history resealed whole; an index naming the threads for the list
 * screen, rewritten when a thread is added or renamed and never on a
 * message; and a thread read **lazily**, backwards, a page at a time, so
 * nothing of a thread not opened is decrypted at start. Nothing here has a
 * retention window (`network-design.md` §7.5.1).
 *
 * A record is `[len][sealed][len]`, the trailing length so the file is
 * walked backwards; the label a record is sealed under names the identity,
 * the thread and the record's own offset, so a record cannot be moved from
 * one place to another. A record's offset is the message's id: unique in
 * its thread, monotonic, and what a delivery change names.
 */
class Threads(private val files: Files, private val sealer: Sealer, private val by: String) {
    /** The shell's files: whole reads and writes, and appends and ranged reads for the thread files. */
    interface Files {
        fun read(name: String): ByteArray?
        fun write(name: String, bytes: ByteArray): Boolean
        fun append(name: String, bytes: ByteArray): Boolean
        fun size(name: String): Long
        fun readAt(name: String, at: Long, len: Int): ByteArray?
    }

    /** The kernel's sealing service, as the shell reaches it. */
    interface Sealer {
        fun seal(label: String, bytes: ByteArray): ByteArray
        fun unseal(label: String, bytes: ByteArray): ByteArray?
    }

    /** A thread as the index lists it: enough for the list screen, nothing of what was said. */
    data class Entry(val peer: String, val name: String, val direct: Boolean)

    /** One record of a thread file: a message, or a delivery change to one. */
    sealed class Record {
        data class Said(val mine: Boolean, val who: String, val text: String, val delivery: Front.Delivery, val reason: String?) : Record()
        data class Settled(val id: Long, val delivery: Front.Delivery, val reason: String?) : Record()
    }

    /**
     * A page read backwards: the messages in file order, where the page
     * before it ends (null: the file's start was reached), delivery changes
     * met whose message lies earlier still, and whether a record refused to
     * open — the walk stops there and what opened is returned.
     */
    data class Page(
        val messages: List<Front.Message>,
        val before: Long?,
        val pending: Map<Long, Record.Settled>,
        val broken: Boolean,
    )

    /** The threads the index names; none where there is no index or it does not open. */
    fun index(): List<Entry> {
        val bytes = files.read(INDEX) ?: return emptyList()
        val open = sealer.unseal("$by/$INDEX", bytes) ?: return emptyList()
        val v = runCatching { Json.parse(String(open)) }.getOrNull() ?: return emptyList()
        return Json.list(v).mapNotNull { e ->
            val o = Json.obj(e)
            val peer = Json.string(o["peer"]) ?: return@mapNotNull null
            Entry(peer, Json.string(o["name"]) ?: peer, Json.bool(o["direct"]) ?: false)
        }
    }

    fun writeIndex(entries: List<Entry>): Boolean {
        val json = entries.joinToString(",", "[", "]") { e ->
            "{\"peer\":${Json.quote(e.peer)},\"name\":${Json.quote(e.name)},\"direct\":${e.direct}}"
        }
        return files.write(INDEX, sealer.seal("$by/$INDEX", json.toByteArray()))
    }

    /** Append one record to the thread with `peer`; the record's id (its offset), or null where the file did not take it. */
    fun append(peer: String, record: Record): Long? {
        val name = fileOf(peer)
        val at = files.size(name)
        val sealed = sealer.seal(label(peer, at), encode(record).toByteArray())
        val len = sealed.size
        val framed = ByteArray(4 + len + 4)
        putLen(framed, 0, len)
        System.arraycopy(sealed, 0, framed, 4, len)
        putLen(framed, 4 + len, len)
        return if (files.append(name, framed)) at else null
    }

    /**
     * The newest `n` messages before `before` (the file's end where null),
     * delivery changes folded in — `pending` carries the ones an earlier
     * page met for messages that lie here.
     */
    fun page(peer: String, before: Long?, n: Int, pending: Map<Long, Record.Settled> = emptyMap()): Page {
        val name = fileOf(peer)
        var at = before ?: files.size(name)
        val settles = HashMap(pending)
        val out = ArrayList<Front.Message>()
        var broken = false
        while (out.size < n && at >= 8) {
            val tail = files.readAt(name, at - 4, 4)
            val len = tail?.let { getLen(it, 0) } ?: -1
            val start = at - 4 - len - 4
            if (len < 0 || start < 0) { broken = true; break }
            val sealed = files.readAt(name, start + 4, len)
            val head = files.readAt(name, start, 4)
            if (sealed == null || head == null || getLen(head, 0) != len) { broken = true; break }
            val open = sealer.unseal(label(peer, start), sealed)
            val rec = open?.let { decode(String(it)) }
            if (rec == null) { broken = true; break }
            when (rec) {
                is Record.Settled -> settles[rec.id] = rec
                is Record.Said -> {
                    val s = settles.remove(start)
                    out.add(Front.Message(start, rec.mine, rec.who, rec.text, s?.delivery ?: rec.delivery, s?.reason ?: rec.reason))
                }
            }
            at = start
        }
        out.reverse()
        return Page(out, if (at > 0 && !broken) at else null, settles, broken)
    }

    private fun label(peer: String, at: Long) = "$by/${fileOf(peer)}/@$at"

    companion object {
        /** How many messages a page holds. */
        const val PAGE = 50
        const val DIR = "dm"
        const val INDEX = "$DIR/index"
        fun fileOf(peer: String) = "$DIR/$peer"

        private fun putLen(b: ByteArray, at: Int, len: Int) {
            b[at] = (len ushr 24).toByte(); b[at + 1] = (len ushr 16).toByte(); b[at + 2] = (len ushr 8).toByte(); b[at + 3] = len.toByte()
        }
        private fun getLen(b: ByteArray, at: Int): Int {
            if (b.size < at + 4) return -1
            return ((b[at].toInt() and 0xff) shl 24) or ((b[at + 1].toInt() and 0xff) shl 16) or ((b[at + 2].toInt() and 0xff) shl 8) or (b[at + 3].toInt() and 0xff)
        }

        fun encode(r: Record): String = when (r) {
            is Record.Said -> "{\"mine\":${r.mine},\"who\":${Json.quote(r.who)},\"text\":${Json.quote(r.text)}," +
                "\"delivery\":${Json.quote(r.delivery.name)},\"reason\":${r.reason?.let { Json.quote(it) } ?: "null"}}"
            is Record.Settled -> "{\"settle\":${r.id},\"delivery\":${Json.quote(r.delivery.name)},\"reason\":${r.reason?.let { Json.quote(it) } ?: "null"}}"
        }

        fun decode(text: String): Record? {
            val o = runCatching { Json.obj(Json.parse(text)) }.getOrNull() ?: return null
            val delivery = Json.string(o["delivery"])?.let { d -> Front.Delivery.entries.firstOrNull { it.name == d } } ?: return null
            val settle = Json.number(o["settle"])?.toLong()
            return if (settle != null) {
                Record.Settled(settle, delivery, Json.string(o["reason"]))
            } else {
                Record.Said(Json.bool(o["mine"]) ?: false, Json.string(o["who"]) ?: "", Json.string(o["text"]) ?: "", delivery, Json.string(o["reason"]))
            }
        }

        /** The one JSON file of 2026-11-12, read once at the migration and never written again. */
        fun decodeLegacy(bytes: ByteArray?): List<Front.Thread> {
            val v = bytes?.let { runCatching { Json.parse(String(it)) }.getOrNull() } ?: return emptyList()
            return Json.list(v).mapNotNull { t ->
                val o = Json.obj(t)
                val by = Json.string(o["by"]) ?: return@mapNotNull null
                val peer = Json.string(o["peer"]) ?: return@mapNotNull null
                val messages = Json.list(o["messages"]).mapNotNull { m ->
                    val mo = Json.obj(m)
                    val delivery = Json.string(mo["delivery"])?.let { d -> Front.Delivery.entries.firstOrNull { it.name == d } }
                        ?: return@mapNotNull null
                    Front.Message(
                        id = Json.number(mo["id"])?.toLong() ?: return@mapNotNull null,
                        mine = Json.bool(mo["mine"]) ?: false,
                        who = Json.string(mo["who"]) ?: "",
                        text = Json.string(mo["text"]) ?: "",
                        delivery = delivery,
                        reason = Json.string(mo["reason"]),
                    )
                }
                Front.Thread(by, peer, Json.string(o["name"]) ?: peer, Json.bool(o["direct"]) ?: false, messages)
            }
        }
    }
}
