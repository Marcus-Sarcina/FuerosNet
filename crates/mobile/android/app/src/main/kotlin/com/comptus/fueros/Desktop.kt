package com.comptus.fueros

import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.IOException

/**
 * **A desktop of this identity, paired from the code it shows** — the pure
 * part, so a test can hold it, as [Pairing] is.
 *
 * The protocol is the kernel's (`wire-format.md` §14.3.3): a desktop has
 * no camera, so it shows one code and this phone reads it; everything
 * after crosses a session that code keys, sealed and opened by the
 * kernel's `DeviceLink`. What this file owns is the shell's side: the
 * order of the steps, the words, and the bearer.
 *
 * **The bearer is the two shells' and not the protocol's** (§14.3): one
 * TCP connection on the local network to where the code says, each
 * carriage as a four-byte big-endian length and its bytes — the desktop's
 * `pairing.rs` reads and writes the same.
 */
object Desktop {
    /** The steps, in order. */
    enum class Step {
        /** Reading the desktop's code. */
        CODE,

        /** Signing the run and the desktop's bundle. */
        CREDENTIAL,

        /** The dataset: contributions, then the hand-over on a yes. */
        DATASET,

        /** Paired. */
        DONE,

        /** Ended, with the reason shown. */
        STOPPED,
    }

    /** **The run a desktop is provisioned with**: 45 windows of 48 hours,
     *  90 days, as an instance's is (`infra-client-requirements.md` §7). */
    const val RUN: Int = 45

    /** How far before now the run opens, so a desktop whose clock runs a
     *  little behind this phone's is inside the first window already. */
    const val LEAD_S: Long = 60

    /** The largest carriage taken off the connection: a dataset with its
     *  sealed captures is the large one. Matches `pairing.rs`. */
    const val CEILING: Int = 1 shl 30

    /** How long one step waits on the desktop. */
    const val STEP_MS: Int = 180_000

    /** What the person reads at each step. */
    fun narration(step: Step): String = when (step) {
        Step.CODE -> "Open FuerosNet on the desktop and choose to pair it with your phone. Point this camera at the code it shows."
        Step.CREDENTIAL -> "Signing the desktop in as a device of this identity. It can read everything and sign nothing."
        Step.DATASET -> "Bringing the desktop to everything this phone holds. Each device asks its holder first."
        Step.DONE -> "The desktop is paired."
        Step.STOPPED -> "Pairing stopped."
    }

    /** One carriage onto the connection. */
    fun write(out: DataOutputStream, bytes: ByteArray) {
        out.writeInt(bytes.size)
        out.write(bytes)
        out.flush()
    }

    /** One carriage off the connection; refused over [CEILING]. */
    fun read(input: DataInputStream): ByteArray {
        val n = input.readInt()
        if (n < 0 || n > CEILING) throw IOException("a carriage of $n bytes is over the ceiling")
        val b = ByteArray(n)
        input.readFully(b)
        return b
    }

    /** The host and port a reach names, or null: `host:port`, with an
     *  IPv6 host in brackets as the desktop writes it. */
    fun hostPort(reach: String): Pair<String, Int>? {
        val i = reach.lastIndexOf(':')
        if (i <= 0) return null
        val port = reach.substring(i + 1).toIntOrNull()?.takeIf { it in 1..65535 } ?: return null
        val host = reach.substring(0, i).removePrefix("[").removeSuffix("]")
        return if (host.isEmpty()) null else host to port
    }
}
