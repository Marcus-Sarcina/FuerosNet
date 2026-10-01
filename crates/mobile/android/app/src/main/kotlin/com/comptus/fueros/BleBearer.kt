package com.comptus.fueros

import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.ParcelUuid
import android.util.Log
import java.util.UUID

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

    private var server: BluetoothGattServer? = null
    private var gatt: BluetoothGatt? = null
    private var peer: BluetoothDevice? = null
    private var negotiated = 23
    private var inbound: Inbound? = null
    private var advertiser: AdvertiseCallback? = null
    private var scanner: ScanCallback? = null

    private fun manager(): BluetoothManager? =
        context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager

    /** The link this bearer presents, once a peer is connected. */
    fun link(): Bearer.Link = object : Bearer.Link {
        override fun mtu(): Int = negotiated - ATT_OVERHEAD

        override fun send(packet: ByteArray): Boolean {
            // the client side writes; the server side notifies. One of the
            // two is set, never both.
            gatt?.let { g ->
                val c = g.getService(SERVICE)?.getCharacteristic(INBOUND) ?: return false
                return try {
                    @Suppress("DEPRECATION")
                    g.writeCharacteristic(c, packet, BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT)
                        .let { it == BluetoothGatt.GATT_SUCCESS || it == 0 }
                } catch (e: SecurityException) {
                    false
                }
            }
            val s = server ?: return false
            val d = peer ?: return false
            val c = s.getService(SERVICE)?.getCharacteristic(OUTBOUND) ?: return false
            return try {
                @Suppress("DEPRECATION")
                s.notifyCharacteristicChanged(d, c, false, packet) == BluetoothGatt.GATT_SUCCESS
            } catch (e: SecurityException) {
                false
            }
        }
    }

    /**
     * Advertise, and take what a peer writes. The returned string is why it
     * could not start, or null.
     */
    fun offer(into: Inbound): String? {
        inbound = into
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
            service.addCharacteristic(
                BluetoothGattCharacteristic(
                    OUTBOUND,
                    BluetoothGattCharacteristic.PROPERTY_NOTIFY,
                    BluetoothGattCharacteristic.PERMISSION_READ,
                ),
            )
            s.addService(service)
            val cb = object : AdvertiseCallback() {
                override fun onStartFailure(error: Int) {
                    Log.w("fueros", "ble advertise failed: $error")
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
    fun seek(into: Inbound): String? {
        inbound = into
        val m = manager() ?: return "this device exposes no Bluetooth service"
        val adapter = m.adapter ?: return "no Bluetooth adapter"
        if (!adapter.isEnabled) return "Bluetooth is off"
        return try {
            val cb = object : ScanCallback() {
                override fun onScanResult(type: Int, result: ScanResult) {
                    val d = result.device ?: return
                    stopScan(adapter)
                    peer = d
                    gatt = d.connectGatt(context, false, clientCallback)
                }

                override fun onScanFailed(error: Int) {
                    Log.w("fueros", "ble scan failed: $error")
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

    private val serverCallback = object : BluetoothGattServerCallback() {
        override fun onConnectionStateChange(d: BluetoothDevice, status: Int, newState: Int) {
            peer = if (newState == BluetoothProfile.STATE_CONNECTED) d else null
        }

        override fun onMtuChanged(d: BluetoothDevice, mtu: Int) {
            negotiated = mtu
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
                    Log.w("fueros", "ble response refused")
                }
            }
        }
    }

    private val clientCallback = object : BluetoothGattCallback() {
        override fun onConnectionStateChange(g: BluetoothGatt, status: Int, newState: Int) {
            if (newState == BluetoothProfile.STATE_CONNECTED) {
                try {
                    // ask for the largest MTU the stack will give: every
                    // byte of it is slice (`Bearer.HEADER` aside), and a
                    // bundle carriage is where that tells
                    g.requestMtu(517)
                } catch (e: SecurityException) {
                    Log.w("fueros", "ble mtu request refused")
                }
            }
        }

        override fun onMtuChanged(g: BluetoothGatt, mtu: Int, status: Int) {
            negotiated = mtu
            try {
                g.discoverServices()
            } catch (e: SecurityException) {
                Log.w("fueros", "ble discovery refused")
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
            } catch (e: SecurityException) {
                Log.w("fueros", "ble stopScan refused")
            }
        }
        scanner = null
    }

    /** Stop everything and give the radio back. Safe to call twice. */
    fun close() {
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
    }
}
