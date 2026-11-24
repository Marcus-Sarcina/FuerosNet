package com.comptus.fueros

import android.app.Activity
import android.app.AlertDialog
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.BitmapShader
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Shader
import android.graphics.drawable.Drawable
import android.view.Gravity
import android.widget.TextView
import uniffi.rhtn_model.Contact

/**
 * **The people an identity talks to, as the conversation screens draw
 * them** [author, 2026-11-23]: an orb in their colour or wearing the
 * picture their profile carries, and the menu a press-and-hold on it
 * opens — their colour, mute, notifications, block. What is chosen is
 * [Front.Person], kept sealed beside the threads.
 */
object People {

    /** A person's colour on screen: the one chosen, else the one their key gives them. */
    fun colour(by: String, peer: String): Int =
        Chrome.personColour(Gateway.front.person(by, peer).colour, peer).second

    /**
     * **Their orb**: the picture their profile carries where one has been
     * received and reads, else their initial on their colour.
     */
    fun orb(a: Activity, by: String, peer: String, name: String, contact: Contact?, size: Int): TextView =
        TextView(a).apply {
            val picture = contact?.image?.let { b -> BitmapFactory.decodeByteArray(b, 0, b.size) }
            if (picture != null) {
                background = round(picture)
            } else {
                text = name.trim().take(1).uppercase()
                textSize = 18f
                typeface = Type.bold
                setTextColor(Palette.RAIL_INK)
                gravity = Gravity.CENTER
                background = Crowns.disc(colour(by, peer))
            }
            layoutParams = android.widget.LinearLayout.LayoutParams(size, size)
            contentDescription = "$name: press and hold for their options"
            isLongClickable = true
            setOnLongClickListener {
                menu(a, by, peer, name)
                true
            }
        }

    /** A picture cropped to the orb's circle. */
    private fun round(picture: Bitmap): Drawable = object : Drawable() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        override fun draw(canvas: Canvas) {
            val b = bounds
            val side = minOf(picture.width, picture.height).toFloat()
            val scale = minOf(b.width(), b.height()) / side
            paint.shader = BitmapShader(picture, Shader.TileMode.CLAMP, Shader.TileMode.CLAMP).apply {
                setLocalMatrix(
                    android.graphics.Matrix().apply {
                        setScale(scale, scale)
                        postTranslate(
                            b.left - (picture.width - side) / 2f * scale,
                            b.top - (picture.height - side) / 2f * scale,
                        )
                    },
                )
            }
            canvas.drawCircle(b.exactCenterX(), b.exactCenterY(), minOf(b.width(), b.height()) / 2f, paint)
        }
        override fun setAlpha(alpha: Int) {}
        override fun setColorFilter(f: android.graphics.ColorFilter?) {}
        @Deprecated("the platform's own abstract member")
        override fun getOpacity(): Int = android.graphics.PixelFormat.TRANSLUCENT
    }

    /**
     * **Their options** [author, 2026-11-23]: their colour, mute, mute
     * notifications, block, each a toggle saying what it will do. A screen
     * redraws from the front's change.
     */
    fun menu(a: Activity, by: String, peer: String, name: String) {
        val p = Gateway.front.person(by, peer)
        val rows = arrayOf(
            "Colour: " + Chrome.personColour(p.colour, peer).first,
            if (p.muted) "Unmute" else "Mute",
            if (p.silent) "Unmute notifications" else "Mute notifications",
            if (p.blocked) "Unblock" else "Block",
        )
        AlertDialog.Builder(a)
            .setTitle(name)
            .setItems(rows) { _, which ->
                when (which) {
                    0 -> chooseColour(a, by, peer, name)
                    1 -> Gateway.front.choose(by, peer) { it.copy(muted = !it.muted) }
                    2 -> Gateway.front.choose(by, peer) { it.copy(silent = !it.silent) }
                    3 -> Gateway.front.choose(by, peer) { it.copy(blocked = !it.blocked) }
                }
            }
            .show()
    }

    private fun chooseColour(a: Activity, by: String, peer: String, name: String) {
        val now = Gateway.front.person(by, peer).colour
        val given = Chrome.personColour(null, peer).first
        val rows = (listOf("The one their key gives them ($given)") + Palette.IDENTITY.map { it.first })
            .mapIndexed { i, r -> if ((i == 0 && now == null) || r == now) "● $r" else r }
            .toTypedArray()
        AlertDialog.Builder(a)
            .setTitle(name)
            .setItems(rows) { _, which ->
                val chosen = if (which == 0) null else Palette.IDENTITY.getOrNull(which - 1)?.first
                Gateway.front.choose(by, peer) { it.copy(colour = chosen) }
            }
            .show()
    }
}
