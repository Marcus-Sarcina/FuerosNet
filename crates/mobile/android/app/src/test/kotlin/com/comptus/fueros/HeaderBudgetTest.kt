package com.comptus.fueros

import org.junit.Test

/**
 * **What a header byte costs at the chosen symbol.**
 *
 * The 29-module code holds **25 bytes however they are split**, so every
 * byte of header is paid for in parts: five today for `version, which,
 * index, count, received`, which is 102 parts of a 2,022-byte object. The
 * tracking-bitmap design moves `received` out to unprotected rows,
 * derives `count` from the version and carries the version and `which`
 * there too, leaving `index` alone — one byte, **85 parts**.
 */
class HeaderBudgetTest {
    private val contribution = ByteArray(2022) { (it * 31 % 251).toByte() }

    /** The two figures the format work rests on: what the symbol holds,
     *  and what the header reduces to. */
    @Test
    fun the_symbol_holds_twenty_five_bytes_and_one_header_byte_is_eighty_five_parts() {
        fun partsAt(header: Int): Int? {
            var best = -1
            for (chunk in 4..60) {
                val part = ByteArray(header + chunk) { i ->
                    if (i < header) (i * 37 + 11).toByte() else contribution[(i - header) % contribution.size]
                }
                if (Optical.matrix(part).width <= 29) best = chunk else break
            }
            if (best < 0) return null
            return (contribution.size + best - 1) / best
        }
        org.junit.Assert.assertEquals("five header bytes, as it ships", 102, partsAt(5))
        org.junit.Assert.assertEquals("one header byte: index alone", 85, partsAt(1))
        // the total the symbol takes is what makes those two trade
        for (header in 1..8) {
            val part = ByteArray(25) { (it * 13 + 5).toByte() }
            org.junit.Assert.assertEquals(29, Optical.matrix(part).width)
        }
    }

    @Test
    fun what_a_header_byte_costs_at_the_chosen_symbol() {
        println("header\tchunk\tparts\tmodules\tsymbol bytes")
        for (header in 1..8) {
            // the largest chunk still at 29 modules for this header size
            var best = -1
            for (chunk in 4..60) {
                val part = ByteArray(header + chunk) { i ->
                    if (i < header) (i * 37 + 11).toByte() else contribution[(i - header) % contribution.size]
                }
                if (Optical.matrix(part).width <= 29) best = chunk else break
            }
            if (best < 0) continue
            val parts = (contribution.size + best - 1) / best
            println("$header\t$best\t$parts\t29\t${header + best}")
        }
    }
}
