package com.comptus.fueros

import com.google.zxing.BinaryBitmap
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader

/**
 * **Three ordinary QR symbols in one image, one per colour channel**
 * [author, 2026-10-06].
 *
 * Each channel carries its own standard symbol, so the encoder and the
 * decoder are ZXing's on both sides and nothing here invents a symbology:
 * what is new is only that a frame carries three parts instead of one, and
 * that **the channel is the part's index** — red is the frame's first part,
 * green its second, blue its third — so a channel lost to the camera is a
 * *known* missing part the lockstep re-shows rather than an unidentifiable
 * fragment.
 *
 * **The compressed header is two bytes**, against the monochrome format's
 * five [author, 2026-10-06]: the frame index and how many of the other
 * side's parts this device holds. Version, `which` and the part count are
 * what the probe exchange has already established, and at the symbol sizes
 * this format exists for the saving is most of the payload — at a
 * 13-byte symbol a five-byte header is 38% of it. Measured over the
 * 2,022-byte contribution: 85 frames with a header in every channel
 * against 62 with two bytes in every channel, where one shared header
 * would give 60 and leave a channel's bytes unaddressable if the channel
 * carrying it failed.
 *
 * **A compressed header cannot be mistaken for a monochrome one**: its
 * first byte has the high bit set and the monochrome format's first byte
 * is its version, 1. A receiver reads either from the bytes alone.
 *
 * **What this is an experiment about, stated before it was built** — a
 * `YUV_420_888` camera frame carries chroma at half the linear resolution
 * of luminance, and the separation of three colour channels is a chroma
 * question while a monochrome symbol's modules are a luminance one. So
 * colour may need modules about twice as large to separate as monochrome
 * needs to resolve, which would cancel the three-times gain and more. The
 * probe exchange is what settles it on hardware rather than here, and the
 * cost of being wrong is one frame and a fall back to monochrome.
 */
object Polychrome {

    /** How many parts one frame carries: one per colour channel. */
    const val CHANNELS = 3

    /** The compressed header's length: frame index, then received. */
    const val HEADER = 2

    /** The high bit of the compressed header's first byte, which the
     *  monochrome format's version byte never has. */
    const val MARK = 0x80

    /** The largest frame index a compressed header can name. */
    const val MAX_FRAME = 0x7f

    /**
     * The compressed header for `frame`, carrying `got` — how many of the
     * other side's parts this device holds, which is what lets the other
     * side advance.
     */
    fun header(frame: Int, got: Int): ByteArray {
        require(frame in 0..MAX_FRAME) { "a compressed frame index is seven bits" }
        require(got in 0..255) { "received is one byte" }
        return byteArrayOf((MARK or frame).toByte(), got.toByte())
    }

    /** Whether `bytes` opens with a compressed header rather than a
     *  monochrome one. */
    fun compressed(bytes: ByteArray): Boolean =
        bytes.size >= HEADER && (bytes[0].toInt() and MARK) != 0

    /** The frame index a compressed header names, or -1. */
    fun frameOf(bytes: ByteArray): Int =
        if (compressed(bytes)) bytes[0].toInt() and MAX_FRAME else -1

    /** The `got` a compressed header carries, or -1. */
    fun gotOf(bytes: ByteArray): Int =
        if (compressed(bytes)) bytes[1].toInt() and 0xff else -1

    /**
     * Three symbols composed into one image, `channels[0]` in red,
     * `[1]` in green, `[2]` in blue, each scaled by `scale`.
     *
     * **A module is on where its channel's symbol is on.** The three
     * symbols are the same version by construction — the parts are the
     * same length — so they share a module grid and compose without
     * resampling. A pixel is black where all three are dark and white
     * where none is.
     *
     * **A composed frame is not readable in monochrome**, and nothing
     * should be built on the hope that it is: luminance is a weighted
     * blend of the three channels, so a device reading the Y plane of this
     * image sees neither of the three symbols but a mixture of them, and
     * decodes nothing. The consequence for the exchange is that a reader
     * whose camera cannot separate the channels reports holding **none**
     * of the sender's parts, not some of them — which is what the fall
     * back to monochrome is triggered by.
     *
     * Returns ARGB pixels, `size` on a side, for the caller to put in
     * whatever a platform calls a bitmap.
     */
    fun compose(channels: List<ByteArray>, scale: Int): Pair<IntArray, Int> {
        require(channels.size == CHANNELS) { "a frame is ${CHANNELS} channels" }
        require(scale >= 1) { "a module is at least a pixel" }
        val mats = channels.map { Optical.matrix(it) }
        val modules = mats[0].width
        require(mats.all { it.width == modules }) {
            "the channels' symbols differ in size: ${mats.map { it.width }}"
        }
        val w = modules * scale
        val px = IntArray(w * w)
        for (y in 0 until w) {
            val my = y / scale
            for (x in 0 until w) {
                val mx = x / scale
                // a channel's bit set means that channel is DARK there, so
                // the channel's intensity is removed from the pixel
                var argb = 0xff000000.toInt()
                if (!mats[0].get(mx, my)) argb = argb or 0x00ff0000
                if (!mats[1].get(mx, my)) argb = argb or 0x0000ff00
                if (!mats[2].get(mx, my)) argb = argb or 0x000000ff
                px[y * w + x] = argb
            }
        }
        return px to w
    }

    /**
     * The three channels read out of one ARGB frame: each channel's plane
     * decoded on its own, `null` where that channel carried no symbol this
     * frame.
     *
     * **One channel's failure is one part's**, which is the property the
     * whole format rests on: the other two are returned and the lockstep
     * advances by what arrived.
     */
    fun separate(argb: IntArray, w: Int, h: Int): List<ByteArray?> {
        val shifts = listOf(16, 8, 0)
        return shifts.map { shift ->
            // one channel as a greyscale image: ZXing's RGB source reads
            // luminance from a pixel, so the channel is written into all
            // three of its components
            val plane = IntArray(argb.size) { i ->
                val v = (argb[i] ushr shift) and 0xff
                (0xff shl 24) or (v shl 16) or (v shl 8) or v
            }
            decode(plane, w, h)
        }
    }

    /**
     * The three channels as luminance planes, straight from a camera's
     * `YUV_420_888` frame: one byte a pixel a channel, for a decoder that
     * wants luminance.
     *
     * **This is where the format's one real risk lives.** Y is full
     * resolution and U and V are not — a 4:2:0 frame carries one chroma
     * sample per 2×2 block — so the colour that separates the channels is
     * band-limited to half the linear resolution of the luminance that
     * resolves a monochrome module. Whether three channels separate at
     * module scale is therefore a property of the camera and not of this
     * code, and the frame that decides it is what measures it.
     *
     * `into` is reused across frames: three planes of `w * h` bytes is
     * eight megabytes at a 1920×1440 frame, which is not an allocation to
     * make ten times a second.
     */
    fun planesFromYuv(
        y: ByteArray,
        u: ByteArray,
        v: ByteArray,
        yRow: Int,
        uvRow: Int,
        uvPixel: Int,
        w: Int,
        h: Int,
        into: Array<ByteArray>,
    ) {
        require(into.size == CHANNELS) { "a frame is $CHANNELS channels" }
        require(into.all { it.size >= w * h }) { "a plane holds one byte a pixel" }
        for (py in 0 until h) {
            val yBase = py * yRow
            val uvBase = (py / 2) * uvRow
            val outBase = py * w
            for (px in 0 until w) {
                val luma = (y[yBase + px].toInt() and 0xff)
                val uvAt = uvBase + (px / 2) * uvPixel
                val cb = (u[uvAt].toInt() and 0xff) - 128
                val cr = (v[uvAt].toInt() and 0xff) - 128
                // BT.601, which is what a camera's YUV is
                val r = luma + (1.402f * cr).toInt()
                val g = luma - (0.344f * cb).toInt() - (0.714f * cr).toInt()
                val b = luma + (1.772f * cb).toInt()
                into[0][outBase + px] = clamp(r)
                into[1][outBase + px] = clamp(g)
                into[2][outBase + px] = clamp(b)
            }
        }
    }

    private fun clamp(v: Int): Byte = when {
        v < 0 -> 0
        v > 255 -> -1 // 0xff
        else -> v.toByte()
    }

    /** One luminance plane to bytes, or null where there was no symbol. */
    fun decodePlane(plane: ByteArray, w: Int, h: Int): ByteArray? {
        val source = com.google.zxing.PlanarYUVLuminanceSource(plane, w, h, 0, 0, w, h, false)
        return try {
            val text = QRCodeReader().decode(BinaryBitmap(HybridBinarizer(source))).text
            Optical.bytes(text)
        } catch (e: Exception) {
            null
        }
    }

    /** One plane to bytes, or null where there was no symbol in it. */
    fun decode(plane: IntArray, w: Int, h: Int): ByteArray? {
        val source = RGBLuminanceSource(w, h, plane)
        return try {
            val text = QRCodeReader().decode(BinaryBitmap(HybridBinarizer(source))).text
            Optical.bytes(text)
        } catch (e: Exception) {
            // no symbol in this channel this frame, or one that failed its
            // own error correction: the next frame is the answer
            null
        }
    }
}
