package com.comptus.fueros

import android.app.Activity
import android.content.Context
import android.nfc.NfcAdapter
import android.nfc.tech.IsoDep
import android.util.Log
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import uniffi.rhtn_ffi.Channel
import uniffi.rhtn_ffi.ChannelOutcome

/**
 * The proximity channels this shell can actually run (D3; design §7.6.3,
 * `light-client-requirements.md` §1.3).
 *
 * **What is listed is what can be RUN.** The kernel's trait says a channel
 * the hardware lacks is not listed, and this shell extends that honestly to
 * channels it cannot drive: UWB hardware may be present, but driving
 * 802.15.4z ranging needs the androidx UWB stack and an out-of-band
 * parameter exchange this shell has not built — so UWB is **not listed**,
 * and the strongest channel recorded is the strongest this shell can
 * attempt, which is the rule's own wording. Recorded as owed, 2026-10-01.
 *
 * Two channels are offered:
 *
 *  - **NFC**, a tap (reader mode on one side, host-card emulation on the
 *    other, by the bootstrap's own asymmetry). *Physical-range friction,
 *    not a distance guarantee* (design §7.6.3), and the exchange carries
 *    only the public ceremony-id.
 *  - **OPTICAL**, which at D3 is a statement about D2: the anchor exchange
 *    ran on the two selfie cameras and agreed, which IS the optical
 *    channel's evidence (design §7.6.3's third rank). It is reported from
 *    what happened rather than re-run, because the thing it attests
 *    already happened on this ceremony's own anchor.
 *
 * **One tap per ceremony.** The kernel runs the ladder twice — once
 * building its own outcomes, once weighing the counterparty's — and a
 * second run must not demand a second tap, so the outcome is cached per
 * ceremony-id and the radio runs once.
 */
object ProximityChannels {

    @Volatile private var host: Activity? = null
    @Volatile private var ceremonyId: ByteArray? = null
    @Volatile private var reader = false
    @Volatile private var anchorAgreed = false
    private val cachedTap = AtomicReference<ChannelOutcome?>(null)

    fun host(a: Activity) {
        host = a
    }

    fun release(a: Activity) {
        if (host === a) host = null
    }

    /** The ceremony this device is in, and which side of the tap it is. */
    fun ceremony(id: ByteArray?, readerSide: Boolean, agreed: Boolean) {
        ceremonyId = id
        reader = readerSide
        anchorAgreed = agreed
        if (id == null) cachedTap.set(null)
    }

    /** What the HCE service answers with: this device's ceremony-id. */
    fun current(): ByteArray? = ceremonyId

    /** The HCE side passed: a reader in this ceremony exchanged with us. */
    fun tapServed() {
        cachedTap.set(ChannelOutcome.PASS)
    }

    fun supported(context: Context): List<Channel> {
        val out = mutableListOf<Channel>()
        if (NfcAdapter.getDefaultAdapter(context)?.isEnabled == true) {
            out.add(Channel.NFC)
        }
        out.add(Channel.OPTICAL)
        return out
    }

    fun run(context: Context, channel: Channel): ChannelOutcome = when (channel) {
        Channel.NFC -> tap(context)
        Channel.OPTICAL ->
            if (anchorAgreed) ChannelOutcome.PASS else ChannelOutcome.UNAVAILABLE
        // not listed, so the kernel does not ask; answered honestly anyway
        else -> ChannelOutcome.UNAVAILABLE
    }

    private fun tap(context: Context): ChannelOutcome {
        cachedTap.get()?.let { return it }
        val cid = ceremonyId ?: return ChannelOutcome.UNAVAILABLE
        val adapter = NfcAdapter.getDefaultAdapter(context)
            ?: return ChannelOutcome.UNAVAILABLE
        if (!adapter.isEnabled) return ChannelOutcome.UNAVAILABLE
        val outcome = if (reader) read(adapter, cid) else serve()
        cachedTap.set(outcome)
        return outcome
    }

    /**
     * The reader's side: reader mode until one tag answers or the window
     * closes. The window is interactive — two people touching phones — so
     * it is generous, and a timeout is a FAIL rather than UNAVAILABLE: the
     * hardware was there and the tap did not happen.
     */
    private fun read(adapter: NfcAdapter, cid: ByteArray): ChannelOutcome {
        val a = host ?: return ChannelOutcome.UNAVAILABLE
        val done = CountDownLatch(1)
        val result = AtomicReference(ChannelOutcome.FAIL)
        val cb = NfcAdapter.ReaderCallback { tag ->
            try {
                val iso = IsoDep.get(tag) ?: return@ReaderCallback
                iso.connect()
                iso.transceive(NfcApdu.select())
                val resp = iso.transceive(NfcApdu.exchange(cid))
                if (NfcApdu.passed(resp, cid)) {
                    result.set(ChannelOutcome.PASS)
                    done.countDown()
                }
                iso.close()
            } catch (e: Exception) {
                Log.w("fueros", "nfc read: $e")
            }
        }
        return try {
            a.runOnUiThread {
                adapter.enableReaderMode(
                    a,
                    cb,
                    NfcAdapter.FLAG_READER_NFC_A or NfcAdapter.FLAG_READER_NFC_B or
                        NfcAdapter.FLAG_READER_SKIP_NDEF_CHECK,
                    null,
                )
            }
            done.await(30, TimeUnit.SECONDS)
            result.get()
        } catch (e: Exception) {
            Log.w("fueros", "nfc reader mode: $e")
            ChannelOutcome.UNAVAILABLE
        } finally {
            a.runOnUiThread { runCatching { adapter.disableReaderMode(a) } }
        }
    }

    /** The card's side: wait for the service to be read in this ceremony. */
    private fun serve(): ChannelOutcome {
        val deadline = System.currentTimeMillis() + 30_000
        while (System.currentTimeMillis() < deadline) {
            cachedTap.get()?.let { return it }
            Thread.sleep(200)
        }
        return ChannelOutcome.FAIL
    }
}
