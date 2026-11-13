package com.comptus.fueros

/**
 * **An identity's conversations, kept** [author, 2026-11-12: a restart had
 * emptied every thread while leaving it listed]. One file per identity in
 * its own scope, JSON, the shell's own: the kernel holds records and
 * payload material, never what a person wrote. Nothing here has a
 * retention window (`network-design.md` §7.5.1).
 */
object Threads {
    fun encode(threads: List<Front.Thread>): ByteArray =
        threads.joinToString(",", "[", "]") { t ->
            val msgs = t.messages.joinToString(",", "[", "]") { m ->
                "{\"id\":${m.id},\"mine\":${m.mine},\"who\":${Json.quote(m.who)},\"text\":${Json.quote(m.text)}," +
                    "\"delivery\":${Json.quote(m.delivery.name)},\"reason\":${m.reason?.let { Json.quote(it) } ?: "null"}}"
            }
            "{\"by\":${Json.quote(t.by)},\"peer\":${Json.quote(t.peerKey)},\"name\":${Json.quote(t.peerName)}," +
                "\"direct\":${t.direct},\"messages\":$msgs}"
        }.toByteArray()

    /** The threads a file holds; a file that is not one, or nothing, is no threads. */
    fun decode(bytes: ByteArray?): List<Front.Thread> {
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
