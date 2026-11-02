package com.comptus.fueros

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **The client's half of a declared act** (`infra-client-requirements.md`
 * §8.3; [Acts]).
 *
 * What cannot be exercised on the JVM is the drawing and the socket. What
 * can, and what matters most:
 *
 *  - **that a declaration is read as it was sent**, including a kind this
 *    client does not know, which it must name rather than guess at;
 *  - **that a bounce from the node's page is a hint and nothing more** —
 *    a field the act does not declare is dropped rather than carried, so
 *    a page cannot smuggle one past the screen;
 *  - **that the declared bounds are checked here**, so a refusal reaches
 *    the person on the screen they are looking at;
 *  - **that a set of roles reaches the instance as a field per value**,
 *    which is the shape the node's own checkbox group had.
 *
 * The declaration below is what `rhtn_daemon::acts` sends, abbreviated.
 */
class ActsTest {

    private val declaration = """
        {"surface":"rhtn/administration@1",
         "node":[
           {"name":"reload","label":"Re-read the hosting file","help":"","about":"node",
            "brings":"nothing","parameters":[]},
           {"name":"install","label":"Install a package","help":"the image first","about":"node",
            "brings":"host-key","parameters":[
              {"name":"resource","label":"the resource","kind":"keyhash"},
              {"name":"port","label":"where it listens","kind":"number","low":1,"high":65535},
              {"name":"manifest","label":"its manifest","kind":"text","max":4096}]}],
         "resources":[
           {"resource":"aa","authority":"shop.internal","installed":true,"acts":[
             {"name":"grant","label":"Grant a role","help":"","about":"resource",
              "brings":"nothing","parameters":[
                {"name":"resource","label":"the resource","kind":"keyhash"},
                {"name":"roles","label":"the roles","kind":"several","of":["reader","writer"]},
                {"name":"of","label":"who","kind":"choice","of":["everyone","clients"]}]}]}]}
    """.trimIndent()

    private fun read() = Acts.read(declaration).getOrThrow()

    // acceptance: PRD-22
    @Test
    fun a_declaration_is_read_as_it_was_sent() {
        val d = read()
        assertTrue("the vocabulary is one this client reads", d.understood)
        assertEquals(2, d.node.size)
        assertEquals(1, d.resources.size)
        assertEquals("shop.internal", d.resources[0].authority)
        assertTrue(d.resources[0].installed)

        // an act is found wherever it was declared
        assertNotNull(d.act("reload"))
        assertNotNull(d.act("grant"))
        assertNull(d.act("fly"))

        // **what the client must bring**, which is why the acts are here
        assertEquals(Acts.Brings.NOTHING, d.act("reload")!!.brings)
        assertEquals(Acts.Brings.HOST_KEY, d.act("install")!!.brings)

        // the kinds, each with its own bounds
        val install = d.act("install")!!
        assertEquals(Acts.Kind.Keyhash, install.parameters[0].kind)
        assertEquals(Acts.Kind.Number(1, 65535), install.parameters[1].kind)
        assertEquals(Acts.Kind.Text(4096), install.parameters[2].kind)
        val grant = d.act("grant")!!
        assertEquals(
            Acts.Kind.Several(listOf("reader", "writer")),
            grant.parameters[1].kind,
        )
        assertEquals(Acts.Kind.Choice(listOf("everyone", "clients")), grant.parameters[2].kind)
    }

    // acceptance: PRD-22
    @Test
    fun a_vocabulary_or_a_kind_this_client_does_not_know_is_named_rather_than_guessed_at() {
        // **a version this client does not read**: the fields may mean
        // something else in it, so nothing of it is drawn
        val other = declaration.replace("rhtn/administration@1", "rhtn/administration@7")
        assertFalse(Acts.read(other).getOrThrow().understood)

        // a kind it does not draw: named, and the act refused as a whole
        val odd = declaration.replace("\"kind\":\"keyhash\"", "\"kind\":\"colour\"")
        val a = Acts.read(odd).getOrThrow().act("install")!!
        assertEquals(Acts.Kind.Unknown("colour"), a.parameters[0].kind)
        val why = Acts.undrawable(a)
        assertNotNull("an act with a field it cannot draw is refused whole", why)
        assertTrue(why!!, why.contains("newer client"))

        // and a credential it does not know how to bring is a refusal and
        // not a field left empty
        val needs = declaration.replace("\"brings\":\"host-key\"", "\"brings\":\"a-badge\"")
        val b = Acts.read(needs).getOrThrow().act("install")!!
        assertEquals(Acts.Brings.UNKNOWN, b.brings)
        assertNotNull(Acts.undrawable(b))

        // what it does understand is still drawable
        assertNull(Acts.undrawable(read().act("reload")!!))
        assertNull(Acts.undrawable(read().act("grant")!!))
    }

    // acceptance: PRD-22
    @Test
    fun a_bounce_names_an_act_and_carries_hints_and_nothing_else() {
        assertEquals(
            "grant" to "resource=aa&at=0",
            Acts.bounced("rhtn-act:grant?resource=aa&at=0"),
        )
        assertEquals("reload" to null, Acts.bounced("rhtn-act:reload"))
        // not a bounce at all
        assertNull(Acts.bounced("http://10.0.0.1:7449/"))
        assertNull(Acts.bounced("rhtn-act:"))
        // a name that is not a name: a page cannot put a path or a scheme
        // where an act's name goes
        assertNull(Acts.bounced("rhtn-act:../../etc"))
        assertNull(Acts.bounced("rhtn-act:Grant"))

        // **a hint fills a field the act declares, and nothing else**: the
        // query came from the node's own page, and a seized node must not
        // be able to put a value where this client was not going to ask
        val grant = read().act("grant")!!
        val hints = Acts.hints(grant, "resource=aa&roles=reader&key=secret&unknown=1")
        assertEquals(setOf("resource", "roles"), hints.keys)
        assertEquals("aa", hints["resource"])
        assertFalse("a credential field is never filled from a page", hints.containsKey("key"))

        // and it is decoded as a query is
        assertEquals(
            "shop internal",
            Acts.hints(read().act("install")!!, "manifest=shop%20internal")["manifest"],
        )
        assertTrue(Acts.hints(grant, null).isEmpty())
    }

    // acceptance: PRD-22
    @Test
    fun the_declared_bounds_are_checked_where_the_person_is_standing() {
        val install = read().act("install")!!
        val keyhash = install.parameters[0]
        val port = install.parameters[1]
        val manifest = install.parameters[2]

        assertNull(Acts.refused(keyhash, "ab".repeat(32)))
        assertNotNull(Acts.refused(keyhash, "AB".repeat(32)))
        assertNotNull(Acts.refused(keyhash, "ab"))

        assertNull(Acts.refused(port, "7460"))
        assertNotNull(Acts.refused(port, "0"))
        assertNotNull(Acts.refused(port, "70000"))
        assertNotNull(Acts.refused(port, "seven"))

        assertNull(Acts.refused(manifest, "roles = [\"reader\"]"))
        assertNotNull(Acts.refused(manifest, "x".repeat(4097)))

        val grant = read().act("grant")!!
        assertNull(Acts.refused(grant.parameters[1], "reader,writer"))
        assertNotNull(Acts.refused(grant.parameters[1], "reader,admin"))
        assertNull(Acts.refused(grant.parameters[2], "everyone"))
        assertNotNull(Acts.refused(grant.parameters[2], "anybody"))
    }

    // acceptance: PRD-22
    @Test
    fun a_set_of_roles_reaches_the_instance_as_one_field_per_value() {
        val grant = read().act("grant")!!
        val body = Acts.body(
            grant,
            mapOf("resource" to "aa", "roles" to "reader,writer", "of" to "everyone"),
        )
        assertEquals(
            listOf(
                "resource" to "aa",
                "roles" to "reader",
                "roles" to "writer",
                "of" to "everyone",
            ),
            body,
        )
        // an empty field is left out rather than sent empty: what an act
        // needs, the instance names
        assertEquals(
            listOf("resource" to "aa"),
            Acts.body(grant, mapOf("resource" to "aa", "roles" to "", "of" to "")),
        )
        // and a field the act does not declare never reaches the body,
        // whatever put it in the answers
        assertEquals(
            listOf("resource" to "aa"),
            Acts.body(grant, mapOf("resource" to "aa", "key" to "secret")),
        )
    }
}
