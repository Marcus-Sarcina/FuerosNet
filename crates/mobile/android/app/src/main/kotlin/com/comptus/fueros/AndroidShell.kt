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
import uniffi.rhtn_ffi.Custody
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
    Proximity, Camera, Clock, Random, Operator, Notices, Storage, Custody {

    private val dir: File = File(context.filesDir, "kernel").apply { mkdirs() }
    private val rng = SecureRandom()

    override fun supported(): List<Channel> = listOf()
    override fun run(channel: Channel, peer: ByteArray): ChannelOutcome = ChannelOutcome.UNAVAILABLE
    override fun resolutionM(channel: Channel): ULong? = null

    override fun capture(ask: Ask): ByteArray = ByteArray(0)

    override fun nowMs(): ULong = System.currentTimeMillis().toULong()
    override fun waitMs(ms: ULong) = Thread.sleep(ms.toLong())

    override fun fill(n: UInt): ByteArray = ByteArray(n.toInt()).also { rng.nextBytes(it) }

    // the person is asked, on whatever screen is foreground; a question
    // nobody is shown is answered no (`light-client-requirements.md` §1.4)
    override fun ask(question: String): Boolean = Consent.ask(question)

    // a notice has an audience, and the kernel knows which: a query about
    // this person belongs on the ceremony they are standing in (design
    // §7.4.2), the rest on the conversation's notice line.  Logged as well,
    // because a notice nobody was looking at is still evidence
    override fun told(notice: Told) {
        Log.i("fueros", "notice: $notice")
        Kernel.told(notice)
    }

    // **The kernel seals what it persists; this shell keeps the key**
    // (`light-client-requirements.md` §9).  What arrives through this seam
    // is sealed already, so it is stored as written; what leaves goes back
    // as written, except files this shell Keystore-wrapped before the
    // kernel sealed its own — those still open here, so a device from
    // before the seam reads on.  The shell's cryptography is custody
    // alone: the storage key, and its own small secrets (the seeds), are
    // wrapped under an Android Keystore key that **never enters this
    // process** — hardware-backed where the device has one, and not
    // recoverable from a backup or from the app's files on their own.
    override fun read(name: String): ByteArray? {
        val f = File(dir, name)
        if (!f.isFile) return null
        val whole = f.readBytes()
        // written by this shell before the kernel sealed, or by [seal]:
        // Keystore-wrapped, and opened here.  Anything else is the
        // kernel's, as written; a wrapped blob that fails to open falls
        // through as bytes the kernel will refuse for what they are
        return unwrap(whole) ?: whole
    }

    override fun write(name: String, bytes: ByteArray): Boolean = land(name, bytes)

    /** The kernel's storage key, where one is kept (`Custody`). */
    override fun key(): ByteArray? = unseal(CUSTODY)?.takeIf { it.size == 32 }

    /** Keep the key the kernel minted, wrapped under the Keystore. */
    override fun keep(key: ByteArray): Boolean = seal(CUSTODY, key)

    /** This platform keeps keys; unsealed is for platforms that cannot. */
    override fun unsealed(): Boolean = false

    /** Write `bytes` under `name` Keystore-wrapped: the shell's own secrets. */
    fun seal(name: String, bytes: ByteArray): Boolean {
        return try {
            val cipher = Cipher.getInstance(TRANSFORM)
            cipher.init(Cipher.ENCRYPT_MODE, atRestKey())
            land(name, cipher.iv + cipher.doFinal(bytes))
        } catch (_: Exception) {
            false
        }
    }

    /** What [seal] wrote under `name`, or nothing. */
    fun unseal(name: String): ByteArray? {
        val f = File(dir, name)
        if (!f.isFile) return null
        return unwrap(f.readBytes())
    }

    private fun unwrap(whole: ByteArray): ByteArray? {
        return try {
            if (whole.size <= IV_BYTES) return null
            val cipher = Cipher.getInstance(TRANSFORM)
            cipher.init(
                Cipher.DECRYPT_MODE,
                atRestKey(),
                GCMParameterSpec(TAG_BITS, whole, 0, IV_BYTES),
            )
            cipher.doFinal(whole, IV_BYTES, whole.size - IV_BYTES)
        } catch (_: Exception) {
            null
        }
    }

    private fun land(name: String, bytes: ByteArray): Boolean {
        return try {
            val tmp = File(dir, "$name.tmp")
            tmp.writeBytes(bytes)
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
        const val CUSTODY = "custody"
        const val KEYSTORE = "AndroidKeyStore"
        const val KEY_ALIAS = "fueros.kernel.at-rest"
        const val TRANSFORM = "AES/GCM/NoPadding"
        const val IV_BYTES = 12
        const val TAG_BITS = 128
    }
}

/**
 * The platform, from the shell the caller already holds.
 *
 * The kernel reads its own provision and seeds through one before it starts,
 * and one shell answering every interface is one answer per question.
 */
fun platformOf(shell: AndroidShell): Platform =
    Platform(shell, shell, shell, shell, shell, shell, shell, shell)
