package com.comptus.fueros

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.rhtn_model.Connection
import uniffi.rhtn_model.Noun

/**
 * The shell's copy of the nouns tells its watchers exactly which nouns
 * changed, and nothing where nothing did. With no kernel up the reads
 * answer empty, which is what this JVM test can hold.
 */
class ModelTest {
    @Test
    fun a_refresh_tells_the_watchers_which_nouns_and_a_connection_is_one_of_them() {
        val told = mutableListOf<Set<Noun>>()
        val latch = CountDownLatch(1)
        val w = Model.Watcher { nouns -> synchronized(told) { told += nouns }; latch.countDown() }
        Model.watch(w)
        try {
            Model.refresh(setOf(Noun.SUBNETS, Noun.PROFILES))
            assertTrue("the refresh reports back", latch.await(5, TimeUnit.SECONDS))
            assertEquals(listOf(setOf(Noun.SUBNETS, Noun.PROFILES)), synchronized(told) { told.toList() })
            assertTrue("no kernel, no subnets", Model.subnets.isEmpty())

            Model.connection(Connection.Detached)
            assertEquals(setOf(Noun.CONNECTION), synchronized(told) { told.last() })
            assertEquals(Connection.Detached, Model.connection)

            val before = synchronized(told) { told.size }
            Model.refresh(emptySet())
            assertEquals("nothing changed, nobody told", before, synchronized(told) { told.size })
        } finally {
            Model.unwatch(w)
        }
    }
}
