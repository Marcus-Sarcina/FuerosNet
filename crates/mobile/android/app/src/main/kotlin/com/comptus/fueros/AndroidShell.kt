package com.comptus.fueros

import android.content.Context
import android.util.Log
import java.io.File
import java.security.SecureRandom
import uniffi.rhtn_ffi.Ask
import uniffi.rhtn_ffi.Channel
import uniffi.rhtn_ffi.ChannelOutcome
import uniffi.rhtn_ffi.Camera
import uniffi.rhtn_ffi.Clock
import uniffi.rhtn_ffi.Notices
import uniffi.rhtn_ffi.Operator
import uniffi.rhtn_ffi.Platform
import uniffi.rhtn_ffi.Proximity
import uniffi.rhtn_ffi.Random
import uniffi.rhtn_ffi.Storage
import uniffi.rhtn_ffi.Told

/**
 * The platform the kernel runs on, as this device provides it.
 *
 * Proximity, capture and the operator's question are the ceremony's, and
 * the ceremony is not in this slice: a channel is reported unavailable
 * rather than absent from the list, a capture yields no frame, and a
 * question nobody was shown is answered no.  Storage is the application's
 * private files directory; the kernel's state file names contain no path.
 */
class AndroidShell(context: Context) :
    Proximity, Camera, Clock, Random, Operator, Notices, Storage {

    private val dir: File = File(context.filesDir, "kernel").apply { mkdirs() }
    private val rng = SecureRandom()

    override fun supported(): List<Channel> = listOf()
    override fun run(channel: Channel, peer: ByteArray): ChannelOutcome = ChannelOutcome.UNAVAILABLE
    override fun resolutionM(channel: Channel): ULong? = null

    override fun capture(ask: Ask): ByteArray = ByteArray(0)

    override fun nowMs(): ULong = System.currentTimeMillis().toULong()
    override fun waitMs(ms: ULong) = Thread.sleep(ms.toLong())

    override fun fill(n: UInt): ByteArray = ByteArray(n.toInt()).also { rng.nextBytes(it) }

    override fun ask(question: String): Boolean = false

    override fun told(notice: Told) {
        Log.i("fueros", "notice: $notice")
    }

    override fun read(name: String): ByteArray? {
        val f = File(dir, name)
        return try { if (f.isFile) f.readBytes() else null } catch (_: Exception) { null }
    }

    override fun write(name: String, bytes: ByteArray): Boolean {
        return try {
            val tmp = File(dir, "$name.tmp")
            tmp.writeBytes(bytes)
            tmp.renameTo(File(dir, name))
        } catch (_: Exception) {
            false
        }
    }
}

fun platformOf(context: Context): Platform {
    val s = AndroidShell(context)
    return Platform(s, s, s, s, s, s, s)
}
