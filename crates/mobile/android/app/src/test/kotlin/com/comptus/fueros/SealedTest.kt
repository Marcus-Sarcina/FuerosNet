package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * **Which sealed packages a node says are closed** ([Sealed],
 * `infra-client-requirements.md` §9.4): the only part of reopening that
 * runs without a node, and the part that decides what this app asks.
 */
class SealedTest {
    private val a = "09".repeat(32)
    private val b = "0a".repeat(32)

    // functional: OPS-034
    @Test
    fun onlyTheClosedOnesAreReopened() {
        assertEquals(listOf(b), Sealed.closed("$a open\n$b closed\n"))
    }

    @Test
    fun nothingSealedAsksNothing() {
        assertEquals(emptyList<String>(), Sealed.closed(""))
    }

    @Test
    fun aLineThatIsNotOneResourceAndItsStateIsPassedOver() {
        assertEquals(
            emptyList<String>(),
            Sealed.closed("closed\n${a.take(16)} closed\n$a closed now\n"),
        )
    }
}
