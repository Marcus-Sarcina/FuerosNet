package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ArrivedTest {

    // functional: MAIL-029
    @Test
    fun a_message_arriving_off_screen_is_told_by_sender_and_never_by_content() {
        val loop = File("src/main/kotlin/com/comptus/fueros/Attachments.kt").readText()
        assertTrue(
            "posted from the arrival, where no screen shows it and the party is neither muted nor blocked",
            loop.contains("if (tell && !Arrived.onScreen()) Arrived.post(it, who, from)"),
        )
        val arrived = File("src/main/kotlin/com/comptus/fueros/Arrived.kt").readText()
        assertTrue(arrived.contains(".setContentTitle(who)"))
        assertTrue(
            "the shade is handed who and whose thread, and no message text",
            arrived.contains("fun post(context: Context, who: String, peerKey: String): Boolean"),
        )
        assertFalse(arrived.contains("e.bytes"))
    }

    // functional: SES-025
    @Test
    fun a_move_told_by_a_record_is_proved_as_it_arrives() {
        val loop = File("src/main/kotlin/com/comptus/fueros/Attachments.kt").readText()
        assertTrue(loop.contains("if (uniffi.rhtn_model.Noun.SUBNETS in e.nouns) Thread { proveMoves(p) }.start()"))
    }
}
