package com.comptus.fueros

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Log
import java.io.File
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
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

    // What the kernel hands over is its archive, its payload sessions and
    // its ratchet state — key material included — so it is encrypted here
    // before it reaches the disk (`light-client-requirements.md` §9).  The
    // key lives in the Android Keystore and **never enters this process**:
    // hardware-backed where the device has it, and not recoverable from a
    // backup or from the app's files on their own.
    override fun read(name: String): ByteArray? {
        val f = File(dir, name)
        return try {
            if (!f.isFile) return null
            val whole = f.readBytes()
            if (whole.size <= IV_BYTES) return null
            val cipher = Cipher.getInstance(TRANSFORM)
            cipher.init(
                Cipher.DECRYPT_MODE,
                atRestKey(),
                GCMParameterSpec(TAG_BITS, whole, 0, IV_BYTES),
            )
            cipher.doFinal(whole, IV_BYTES, whole.size - IV_BYTES)
        } catch (_: Exception) {
            // an unreadable state is no state: the kernel is told nothing is
            // there rather than handed something it cannot trust
            null
        }
    }

    override fun write(name: String, bytes: ByteArray): Boolean {
        return try {
            val cipher = Cipher.getInstance(TRANSFORM)
            cipher.init(Cipher.ENCRYPT_MODE, atRestKey())
            val sealed = cipher.iv + cipher.doFinal(bytes)
            val tmp = File(dir, "$name.tmp")
            tmp.writeBytes(sealed)
            tmp.renameTo(File(dir, name))
        } catch (_: Exception) {
            false
        }
    }

    /** The at-rest key, made once and thereafter only referred to. */
    private fun atRestKey(): SecretKey {
        val ks = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (ks.getEntry(KEY_ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        gen.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return gen.generateKey()
    }

    private companion object {
        const val KEYSTORE = "AndroidKeyStore"
        const val KEY_ALIAS = "fueros.kernel.at-rest"
        const val TRANSFORM = "AES/GCM/NoPadding"
        const val IV_BYTES = 12
        const val TAG_BITS = 128
    }
}

fun platformOf(context: Context): Platform {
    val s = AndroidShell(context)
    return Platform(s, s, s, s, s, s, s)
}
