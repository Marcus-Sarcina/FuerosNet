package com.comptus.fueros

import android.app.Activity
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.widget.LinearLayout
import android.widget.TextView

/**
 * A destination that is named on Home but not built in this slice. It says
 * so plainly rather than pretending: the section's name, and one line on
 * what it will hold. An honest gap, not a dead end dressed as a feature.
 */
class PlaceholderActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val section = intent.getStringExtra("section") ?: "Not built"
        val note = intent.getStringExtra("note") ?: ""
        val title = TextView(this).apply {
            text = section
            textSize = 26f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 0, 0, 24)
        }
        val body = TextView(this).apply {
            text = "$note\n\nNot in this build slice."
            textSize = 16f
            setTextColor(Color.GRAY)
        }
        setContentView(
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                fitsSystemWindows = true
                setPadding(48, 48, 48, 48)
                addView(title)
                addView(body)
            },
        )
    }
}
