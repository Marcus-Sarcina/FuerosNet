package com.comptus.fueros

import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.BluetoothStatusCodes
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.Build
import android.os.ParcelUuid
import java.util.UUID
import timber.log.Timber

/**
 * Bluetooth LE as a bearer (`wire-format.md` §14.3.1).
 *
 * **First in the hierarchy, and trusted for nothing.** §14.3.1 ranks a
 * direct local radio above a network fetch on **locality, not integrity**:
 * what keeps the exchange honest is the optical anchor, so the reason to
 * prefer this over a fetch is that it keeps the two devices talking across
 * the room rather than through infrastructure, inside the physical-presence
 * property §14.1 names.
 *
 * **Bluetooth here is not Bluetooth as proximity evidence**, and §14.3.1
 * says so in as many words: design §1.3 bars RSSI from the *distance*
 * channel because signal strength is attacker-controllable, and that bar is
 * about evidence of nearness. Data already bound to the optical anchor
 * needs no distance guarantee of its own. **Nothing in this file is ever
 * offered as a proximity channel**, and the `Proximity` implementation that
 * answers the kernel's channel question is elsewhere and says `Unavailable`
 * where there is no UWB or NFC.
 *
 * The shape: one device **advertises** a service with one write
 * characteristic and one notify characteristic; the other **scans** for it
 * and connects. Who does which is the shell's choice and carries no
 * meaning — the initiator of the ceremony is not necessarily the advertiser.
 *
 * **NOT RUN ON HARDWARE.** It compiles against the platform's API and its
 * framing is tested in `BearerTest` against a link that reorders, repeats
 * and drops. Two radios have never run it, and the things that will bite
 * are the ones radios bite with: an MTU smaller than negotiated, a write
 * that reports success and does not arrive, a peer that disappears
 * mid-carriage. Each of those is a message that does not assemble, which
 * `Bearer.Reassembly` answers with nothing rather than with wrong bytes.
 *
 * **The link is ready when the subscription is**, not when the connection
 * is. A notify characteristic carries nothing until the client has written
 * the Client Characteristic Configuration descriptor asking for
 * notifications, so the client does that as soon as discovery finds the
 * service, and neither side's `send` accepts a packet before that write is
 * confirmed: the client by its write callback, the server by the request
 * it answered. A packet notified earlier would go nowhere and report
 * success.
 *
 * Every step of the radio's life is a `ble` event (`Robot/field-test-
 * diagnostics.md`, section 3.6): advertise, scan, connect, the MTU, the
 * discovery, the subscription, the link coming ready, the disconnect, and
 * every refusal by the platform.
 */
class BleBearer(private val context: Context) {

    companion object {
        /**
         * The service and its two characteristics. Random v4 UUIDs, fixed
         * here: the protocol names no bearer and therefore no UUID, so
         * these identify *this shell's* carriage and nothing more. A
         * counterparty running another shell agrees on them or the two use
         * another bearer.
         */
        val SERVICE: UUID = UUID.fromString("7f3a1c64-9b2e-4d51-8a07-1e6f5c2b4d93")
        val INBOUND: UUID = UUID.fromString("7f3a1c65-9b2e-4d51-8a07-1e6f5c2b4d93")
        val OUTBOUND: UUID = UUID.fromString("7f3a1c66-9b2e-4d51-8a07-1e6f5c2b4d93")

        /**
         * The Client Characteristic Configuration descriptor, Bluetooth's
         * own and not this shell's: the one a client writes to say it wants
         * a characteristic's notifications. Its value is a 16-bit
         * little-endian bitfield whose low bit is "notify".
         */
        val CCCD: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")

        /**
         * ATT's own overhead on a write: three bytes of opcode and handle.
         * An MTU of 23 — the default before negotiation — therefore carries
         * twenty, which is what `Bearer`'s header was sized against.
         */
        const val ATT_OVERHEAD = 3
    }

    /** What arrives, packet by packet, for the caller to reassemble. */
    fun interface Inbound {
        fun packet(bytes: ByteArray)
    }

    /**
     * Whether the peer hears `OUTBOUND`: the state both sides' `send` is
     * gated on. The server sets it from the descriptor write it answers,
     * the client from the confirmation of the write it made, and a
     * disconnect clears it. **No Android in this class**, so
     * `BleBearerTest` can hold it; the radio callbacks only feed it.
     */
    class Subscription {
        @Volatile
        var active: Boolean = false
            private set

        /**
         * The server's side: a descriptor write arrived. Returns what it
         * asked, true for notifications on and false for off, or null when
         * the write is to another descriptor or is not a CCCD value, which
         * leaves the state as it was. A value with the indicate bit alone
         * asks for something this characteristic does not offer, and is
         * "off".
         */
        fun requested(descriptor: UUID, value: ByteArray?): Boolean? {
            if (descriptor != CCCD) return null
            if (value == null || value.size != 2) return null
            val on = (value[0].toInt() and 0x01) != 0
            active = on
            return on
        }

        /** The client's side: the descriptor write's callback, with the
         *  GATT status it carried. Success is zero on every API level. */
        fun confirmed(status: Int): Boolean {
            active = status == 0
            return active
        }

        fun reset() {
            active = false
        }
    }

    private var server: BluetoothGattServer? = null
    private var gatt: BluetoothGatt? = null
    private var peer: BluetoothDevice? = null
    private var negotiated = 23
    private var inbound: Inbound? = null
    private var advertiser: AdvertiseCallback? = null
    private var scanner: ScanCallback? = null
    private val subscription = Subscription()

    private fun manager(): BluetoothManager? =
        context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager

    /** Whether a packet given to [link] now would cross: a peer is
     *  connected and its subscription to `OUTBOUND` is confirmed. */
    fun ready(): Boolean = subscription.active

    /** The link this bearer presents, once a peer is connected and
     *  subscribed; before that every `send` is refused. */
    fun link(): Bearer.Link = object : Bearer.Link {
        override fun mtu(): Int = negotiated - ATT_OVERHEAD

        override fun send(packet: ByteArray): Boolean {
            if (!subscription.active) {
                Diag.warn("ble", "op" to "send", "state" to "refused", "reason" to "the link is not ready")
                return false
            }
            // the client side writes; the server side notifies. One of the
            // two is set, never both.
            gatt?.let { g ->
                val c = g.getService(SERVICE)?.getCharacteristic(INBOUND) ?: return false
                return try {
                    @Suppress("DEPRECATION")
                    val status = g.writeCharacteristic(c, packet, BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT)
                    val ok = status == BluetoothGatt.GATT_SUCCESS || status == 0
                    if (!ok) Diag.warn("ble", "op" to "write", "status" to status)
                    ok
                } catch (e: SecurityException) {
                    refused("write")
                    false
                }
            }
            val s = server ?: return false
            val d = peer ?: return false
            val c = s.getService(SERVICE)?.getCharacteristic(OUTBOUND) ?: return false
            return try {
                @Suppress("DEPRECATION")
                val status = s.notifyCharacteristicChanged(d, c, false, packet)
                val ok = status == BluetoothGatt.GATT_SUCCESS
                if (!ok) Diag.warn("ble", "op" to "notify", "status" to status)
                ok
            } catch (e: SecurityException) {
                refused("notify")
                false
            }
        }
    }

    /**
     * Advertise, and take what a peer writes. The returned string is why it
     * could not start, or null.
     */
    fun offer(into: Inbound): String? = offerOrSeek("advertise", into) {
        val m = manager() ?: return "this device exposes no Bluetooth service"
        val adapter: BluetoothAdapter = m.adapter ?: return "no Bluetooth adapter"
        if (!adapter.isEnabled) return "Bluetooth is off"
        return try {
            val s = m.openGattServer(context, serverCallback) ?: return "no GATT server"
            server = s
            val service = BluetoothGattService(SERVICE, BluetoothGattService.SERVICE_TYPE_PRIMARY)
            service.addCharacteristic(
                BluetoothGattCharacteristic(
                    INBOUND,
                    BluetoothGattCharacteristic.PROPERTY_WRITE or
                        BluetoothGattCharacteristic.PROPERTY_WRITE_NO_RESPONSE,
                    BluetoothGattCharacteristic.PERMISSION_WRITE,
                ),
            )
            val outbound = BluetoothGattCharacteristic(
                OUTBOUND,
                BluetoothGattCharacteristic.PROPERTY_NOTIFY,
                BluetoothGattCharacteristic.PERMISSION_READ,
            )
            // the descriptor the client writes to subscribe: without it
            // there is nothing to write, and a notify characteristic
            // nobody can subscribe to carries nothing
            outbound.addDescriptor(
                BluetoothGattDescriptor(
                    CCCD,
                    BluetoothGattDescriptor.PERMISSION_READ or BluetoothGattDescriptor.PERMISSION_WRITE,
                ),
            )
            service.addCharacteristic(outbound)
            s.addService(service)
            val cb = object : AdvertiseCallback() {
                override fun onStartSuccess(settings: AdvertiseSettings?) {
                    Diag.event("ble", "op" to "advertise", "state" to "started")
                }

                override fun onStartFailure(error: Int) {
                    Timber.w("ble advertise failed: %d", error)
                    Diag.warn("ble", "op" to "advertise", "state" to "failed", "error" to error)
                }
            }
            advertiser = cb
            adapter.bluetoothLeAdvertiser?.startAdvertising(
                AdvertiseSettings.Builder()
                    .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
                    .setConnectable(true)
                    .build(),
                AdvertiseData.Builder()
                    // the service UUID and NOTHING ELSE: an advertisement
                    // is broadcast to the room, and a name or an identifier
                    // in it would be a presence beacon this ceremony has no
                    // use for (design §19.4's shape of disclosure)
                    .addServiceUuid(ParcelUuid(SERVICE))
                    .setIncludeDeviceName(false)
                    .build(),
                cb,
            ) ?: return "this adapter does not advertise"
            null
        } catch (e: SecurityException) {
            "the Bluetooth permissions are not held"
        }
    }

    /** Scan for a peer offering the service, and connect to the first. */
    fun seek(into: Inbound): String? = offerOrSeek("scan", into) {
        val m = manager() ?: return "this device exposes no Bluetooth service"
        val adapter = m.adapter ?: return "no Bluetooth adapter"
        if (!adapter.isEnabled) return "Bluetooth is off"
        return try {
            val cb = object : ScanCallback() {
                override fun onScanResult(type: Int, result: ScanResult) {
                    val d = result.device ?: return
                    Diag.event("ble", "op" to "scan", "state" to "found", "rssi" to result.rssi)
                    stopScan(adapter)
                    peer = d
                    Diag.event("ble", "op" to "connect", "state" to "dialling")
                    gatt = d.connectGatt(context, false, clientCallback)
                }

                override fun onScanFailed(error: Int) {
                    Timber.w("ble scan failed: %d", error)
                    Diag.warn("ble", "op" to "scan", "state" to "failed", "error" to error)
                }
            }
            scanner = cb
            adapter.bluetoothLeScanner?.startScan(
                listOf(ScanFilter.Builder().setServiceUuid(ParcelUuid(SERVICE)).build()),
                ScanSettings.Builder()
                    .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
                    .build(),
                cb,
            ) ?: return "this adapter does not scan"
            null
        } catch (e: SecurityException) {
            "the Bluetooth permissions are not held"
        }
    }

    /** Start the radio one way or the other, and say how it went: the
     *  refusal returned to the caller is the one the event carries. */
    private inline fun offerOrSeek(op: String, into: Inbound, start: () -> String?): String? {
        inbound = into
        val why = start()
        if (why == null) {
            Diag.event("ble", "op" to op, "state" to "starting")
        } else {
            Diag.warn("ble", "op" to op, "state" to "refused", "reason" to why)
        }
        return why
    }

    private fun refused(op: String) {
        Timber.w("ble %s refused", op)
        Diag.warn("ble", "op" to op, "state" to "refused", "reason" to "the Bluetooth permissions are not held")
    }

    private fun subscribeRefused(why: String) {
        Timber.w("ble subscribe refused: %s", why)
        Diag.warn("ble", "op" to "subscribe", "side" to "client", "state" to "refused", "reason" to why)
    }

    /**
     * The client's subscription to `OUTBOUND`: tell the local stack to
     * deliver the characteristic's notifications, then ask the peer to send
     * them by writing its CCCD. The write is asynchronous and the link is
     * ready only when `onDescriptorWrite` confirms it; a stack that will
     * not take the write is a refusal here and now.
     */
    private fun subscribe(g: BluetoothGatt, outbound: BluetoothGattCharacteristic) {
        val cccd = outbound.getDescriptor(CCCD)
        if (cccd == null) {
            subscribeRefused("the notify characteristic has no configuration descriptor")
            return
        }
        try {
            if (!g.setCharacteristicNotification(outbound, true)) {
                subscribeRefused("the stack would not route the notifications")
                return
            }
            val taken = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                g.writeDescriptor(cccd, BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE) == BluetoothStatusCodes.SUCCESS
            } else {
                writeDescriptorBefore33(g, cccd)
            }
            if (taken) {
                Diag.event("ble", "op" to "subscribe", "side" to "client", "state" to "requested")
            } else {
                subscribeRefused("the stack would not take the descriptor write")
            }
        } catch (e: SecurityException) {
            refused("subscribe")
        }
    }

    /** API 31 and 32, which the minSdk admits: the descriptor carries its
     *  own value and the one-argument write is the only one there is. Both
     *  are deprecated from 33, where [subscribe] takes the other branch. */
    @Suppress("DEPRECATION")
    private fun writeDescriptorBefore33(g: BluetoothGatt, cccd: BluetoothGattDescriptor): Boolean {
        cccd.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
        return g.writeDescriptor(cccd)
    }

    private fun connection(side: String, status: Int, newState: Int) {
        val state = when (newState) {
            BluetoothProfile.STATE_CONNECTED -> "connected"
            BluetoothProfile.STATE_DISCONNECTED -> "disconnected"
            else -> newState.toString()
        }
        Diag.event("ble", "op" to "connect", "side" to side, "state" to state, "status" to status)
    }

    private val serverCallback = object : BluetoothGattServerCallback() {
        override fun onConnectionStateChange(d: BluetoothDevice, status: Int, newState: Int) {
            connection("server", status, newState)
            peer = if (newState == BluetoothProfile.STATE_CONNECTED) d else null
            // a subscription belongs to a connection: a peer that comes
            // back subscribes again
            if (newState != BluetoothProfile.STATE_CONNECTED) subscription.reset()
        }

        override fun onDescriptorWriteRequest(
            d: BluetoothDevice,
            requestId: Int,
            descriptor: BluetoothGattDescriptor,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray?,
        ) {
            val asked = subscription.requested(descriptor.uuid, value)
            when (asked) {
                null -> Diag.warn("ble", "op" to "subscribe", "side" to "server", "state" to "refused", "reason" to "not a CCCD write")
                true -> Diag.event("ble", "op" to "link", "side" to "server", "state" to "ready", "mtu" to negotiated)
                false -> Diag.event("ble", "op" to "subscribe", "side" to "server", "state" to "withdrawn")
            }
            if (responseNeeded) {
                // the client's write callback is this response: left
                // unanswered, its stack times the request out and drops the
                // link
                try {
                    val status = if (asked != null) BluetoothGatt.GATT_SUCCESS else BluetoothGatt.GATT_FAILURE
                    server?.sendResponse(d, requestId, status, offset, value)
                } catch (e: SecurityException) {
                    refused("response")
                }
            }
        }

        override fun onMtuChanged(d: BluetoothDevice, mtu: Int) {
            negotiated = mtu
            Diag.event("ble", "op" to "mtu", "side" to "server", "mtu" to mtu)
        }

        override fun onCharacteristicWriteRequest(
            d: BluetoothDevice,
            requestId: Int,
            c: BluetoothGattCharacteristic,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray,
        ) {
            if (c.uuid == INBOUND) inbound?.packet(value)
            if (responseNeeded) {
                try {
                    server?.sendResponse(d, requestId, BluetoothGatt.GATT_SUCCESS, offset, null)
                } catch (e: SecurityException) {
                    refused("response")
                }
            }
        }
    }

    private val clientCallback = object : BluetoothGattCallback() {
        override fun onConnectionStateChange(g: BluetoothGatt, status: Int, newState: Int) {
            connection("client", status, newState)
            if (newState != BluetoothProfile.STATE_CONNECTED) subscription.reset()
            if (newState == BluetoothProfile.STATE_CONNECTED) {
                try {
                    // ask for the largest MTU the stack will give: every
                    // byte of it is slice (`Bearer.HEADER` aside), and a
                    // bundle carriage is where that tells
                    g.requestMtu(517)
                } catch (e: SecurityException) {
                    refused("mtu")
                }
            }
        }

        override fun onMtuChanged(g: BluetoothGatt, mtu: Int, status: Int) {
            negotiated = mtu
            Diag.event("ble", "op" to "mtu", "side" to "client", "mtu" to mtu, "status" to status)
            try {
                g.discoverServices()
            } catch (e: SecurityException) {
                refused("discovery")
            }
        }

        override fun onServicesDiscovered(g: BluetoothGatt, status: Int) {
            val service = g.getService(SERVICE)
            Diag.event("ble", "op" to "discovery", "status" to status, "service" to (service != null))
            val outbound = service?.getCharacteristic(OUTBOUND)
            if (outbound == null) {
                if (service != null) subscribeRefused("the service has no notify characteristic")
                return
            }
            subscribe(g, outbound)
        }

        override fun onDescriptorWrite(g: BluetoothGatt, descriptor: BluetoothGattDescriptor, status: Int) {
            if (descriptor.uuid != CCCD) return
            if (subscription.confirmed(status)) {
                Diag.event("ble", "op" to "link", "side" to "client", "state" to "ready", "mtu" to negotiated)
            } else {
                Timber.w("ble subscribe refused: %d", status)
                Diag.warn("ble", "op" to "subscribe", "side" to "client", "state" to "refused", "status" to status)
            }
        }

        @Suppress("DEPRECATION")
        override fun onCharacteristicChanged(
            g: BluetoothGatt,
            c: BluetoothGattCharacteristic,
            value: ByteArray,
        ) {
            if (c.uuid == OUTBOUND) inbound?.packet(value)
        }
    }

    private fun stopScan(adapter: BluetoothAdapter) {
        scanner?.let {
            try {
                adapter.bluetoothLeScanner?.stopScan(it)
                Diag.event("ble", "op" to "scan", "state" to "stopped")
            } catch (e: SecurityException) {
                refused("stopScan")
            }
        }
        scanner = null
    }

    /** Stop everything and give the radio back. Safe to call twice. */
    fun close() {
        if (gatt != null || server != null) Diag.event("ble", "op" to "close")
        manager()?.adapter?.let { stopScan(it) }
        runCatching {
            advertiser?.let { manager()?.adapter?.bluetoothLeAdvertiser?.stopAdvertising(it) }
        }
        runCatching { gatt?.close() }
        runCatching { server?.close() }
        advertiser = null
        gatt = null
        server = null
        peer = null
        inbound = null
        negotiated = 23
        subscription.reset()
    }
}
