package com.comptus.fueros

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **Every Activity a source names is declared**, which is the one thing
 * about an Activity a JVM test can say.
 *
 * The defect this holds: `CatalogActivity` was written on 2026-10-16,
 * recorded as built, named by Home's own navigation, and declared in no
 * manifest — so tapping *Catalog* threw `ActivityNotFoundException` in both
 * flavours for eleven days. Nothing caught it because every test here is a
 * JVM unit test and none of them touches an Activity; the screen's pure
 * half (`Catalog`) was tested and passed throughout.
 *
 * So this test reads the sources and the manifests as text. It cannot say
 * an Activity works. It can say that one a screen is able to open is one
 * the system knows about, which is the whole of that failure.
 */
class ManifestTest {

    /**
     * Where the tree is, whichever directory the runner started in: the
     * directory holding `app/src/main/AndroidManifest.xml`. Gradle starts a
     * test in the module's own directory, so the usual answer is one above.
     *
     * **Canonical, not absolute**: `File(".").absoluteFile` keeps the dot
     * as a path element, so its parent is the directory itself and the
     * walk climbed nowhere. That is what made the first version of this
     * test read an empty string as the manifests and pass.
     */
    private fun module(): File {
        var at: File? = File(".").canonicalFile
        while (at != null) {
            if (File(at, "app/src/main/AndroidManifest.xml").isFile) return at
            at = at.parentFile
        }
        // deliberately not a silent skip: a test that cannot find the tree
        // it is about says so and fails
        throw AssertionError(
            "no app/src/main/AndroidManifest.xml at or above ${File(".").canonicalPath}: " +
                "this test reads the module's own files, and a runner started elsewhere " +
                "has to be told where",
        )
    }

    private fun manifests(root: File): String {
        val text = listOf("main", "releasable", "fieldtest")
            .map { File(root, "app/src/$it/AndroidManifest.xml") }
            .filter { it.isFile }
            .joinToString("\n") { it.readText() }
        assertTrue("no manifest read under $root", text.contains("<manifest"))
        return text
    }

    private fun sources(root: File): List<File> {
        val found = File(root, "app/src/main/kotlin").walkTopDown()
            .filter { it.extension == "kt" }
            .toList()
        // **a test that found nothing has proved nothing**: an empty list
        // would satisfy every assertion below
        assertTrue("no Kotlin sources under $root", found.size > 10)
        return found
    }

    @Test
    fun every_activity_a_source_can_open_is_declared_in_a_manifest() {
        val root = module()
        val declared = manifests(root)
        val named = Regex("""(\w+Activity)::class\.java""")
        val missing = sortedSetOf<String>()
        for (f in sources(root)) {
            for (m in named.findAll(f.readText())) {
                val name = m.groupValues[1]
                // `android:name=".Foo"`, with the dot, is how this manifest
                // writes its own package's classes
                if (!declared.contains("\".$name\"")) missing.add("$name (${f.name})")
            }
        }
        assertTrue(
            "named by a source and declared in no manifest, so it throws where it is opened: " +
                missing.joinToString(", "),
            missing.isEmpty(),
        )
    }

    /** And the launcher is the shell [author, 2026-10-29]: a manifest that
     *  declared no `MAIN`/`LAUNCHER` filter would install an app with no
     *  way in, and one that gave it to another screen would open on a
     *  navigation the design replaced. */
    /**
     * **Every permission this app asks for is listed here too**, so that
     * one cannot arrive without somebody looking at it.
     *
     * The manifest is where a permission is declared and this is where it
     * is reviewed: adding one means editing this list, which is the whole
     * point. `SEND_SMS` is the restricted one — a patron sends a member
     * the invitation to its node [author, 2026-11-05], chosen over the
     * system composer because the invitation carries out a decision taken
     * at the adoption and a composer would put a tap on each one.
     *
     * **`READ_PHONE_NUMBERS`** [author, 2026-11-10]: the number this
     * handset reports for itself crosses to the counterparty at a meeting,
     * on the local bearer beside the intent, so neither person has to type
     * the other's in (`light-client-requirements.md` §1.5). It reads one
     * value about this device and nothing about anybody else — not the
     * call log, not the contacts, not the subscriber identity — and the
     * number it reads is the one this device is about to hand over
     * deliberately. Install-time grants are not available for it, or for
     * the camera and the radios, on any install path, so it is a one-time
     * ask at the meeting entry and a refusal costs the number alone.
     */
    @Test
    fun the_permissions_asked_for_are_the_ones_reviewed() {
        val declared = Regex("android\\.permission\\.([A-Z_]+)")
            .findAll(manifests(module()))
            .map { it.groupValues[1] }
            .toSortedSet()
        val reviewed = sortedSetOf(
            "ACCESS_COARSE_LOCATION",
            "ACCESS_FINE_LOCATION",
            "ACCESS_NETWORK_STATE",
            "BLUETOOTH_ADVERTISE",
            "BLUETOOTH_CONNECT",
            "BLUETOOTH_SCAN",
            "CAMERA",
            "INTERNET",
            "NFC",
            "POST_NOTIFICATIONS",
            "READ_PHONE_NUMBERS",
            "SEND_SMS",
            "VIBRATE",
        )
        assertEquals(
            "a permission was added or removed without review: edit this list and say why",
            reviewed,
            declared,
        )
    }

    @Test
    fun the_shell_is_the_launcher() {
        val declared = manifests(module())
        val launcher = Regex(
            """<activity\s+android:name="\.(\w+)"[^>]*>\s*<intent-filter>\s*<action android:name="android\.intent\.action\.MAIN" />\s*<category android:name="android\.intent\.category\.LAUNCHER" />""",
        ).find(declared)
        assertTrue("no launcher declared", launcher != null)
        assertTrue("the launcher is ${launcher!!.groupValues[1]}, not the shell", launcher.groupValues[1] == "ShellActivity")
    }

    /**
     * **PRD-13's other half: the tab works with no node, and the door to
     * the instance's own pages opens only once there is one**
     * (`infra-client-requirements.md` §8.3).
     *
     * The first half — that the client ships no administrative page of
     * its own — is below. This is the state half of the row's oracle:
     * *"Before provisioning the tab carries the provisioning pages only,
     * working with no node present; after, what administers the instance
     * arrives from the instance"*. Read from the source, a JVM test being
     * unable to open the tab.
     */
    // acceptance: PRD-13
    @Test
    fun the_operator_tab_provisions_with_no_node_and_opens_no_page_until_there_is_one() {
        val screen = File(
            module(),
            "app/src/main/kotlin/com/comptus/fueros/OperatorActivity.kt",
        ).readText()
        // **there are two doors, not one** [2026-11-09, found by this
        // test]: a standalone control, and one per row of the instance
        // roll. The first starts shut; the second cannot exist with no
        // instance, being drawn inside the roll. So what is asserted is
        // the property both share — **every door names an address that
        // came from an instance**, `pageAt` after a credential was taken
        // or `i.at` off a remembered one, and nothing else can open it.
        val doors = Regex("""NodePageActivity::class\.java\)[\s\S]{0,120}?putExtra\("at", ([A-Za-z.]+)""")
            .findAll(screen)
            .map { it.groupValues[1] }
            .toList()
        assertTrue("no door to the node's pages was found at all", doors.isNotEmpty())
        assertEquals(
            "a door to the node's pages names an address from somewhere other than an instance: $doors",
            setOf("it", "i.at"),
            doors.toSet(),
        )
        // and the standalone one starts shut, `it` being `pageAt`'s value
        assertTrue(
            "the standalone administration-page control is not disabled to begin with",
            screen.contains("text = \"Open its administration page\"\n            isEnabled = false"),
        )
        // and it opens exactly when an instance has taken a credential,
        // which is what makes it an instance rather than a machine
        assertTrue(
            "the door does not open on the enrolment's own result",
            screen.contains("open.isEnabled = pageAt != null"),
        )
        assertTrue(
            "the door does not open after a launch enrols",
            screen.contains("open.isEnabled = true"),
        )
        // **and the provisioning itself reaches no page**: it is native,
        // so it works with no node present (`Provision`, `Launch`)
        for (native in listOf("Provision.fetch", "Launch.providers()")) {
            assertTrue("the tab no longer provisions natively: $native", screen.contains(native))
        }
        // the records the enrolment signs name the chosen subnet, so an
        // operator holding positions in two can enrol in the second
        val kernel = File(
            module(),
            "app/src/main/kotlin/com/comptus/fueros/Kernel.kt",
        ).readText()
        assertTrue(
            "the endpoint record is not signed on the chosen subnet's line",
            kernel.contains("p.endpointRecord(line, listOf(point), 1u)"),
        )
        assertTrue(
            "the anchor entry is not signed on the chosen subnet's line",
            kernel.contains("p.anchorEntryFresh(line2, listOf(point), 1u)"),
        )
    }

    /**
     * **PRD-13: the client ships the provisioning pages and no other
     * administration pages** (`infra-client-requirements.md` §8.3).
     *
     * Read from the sources and the shipped assets, as [InsetSweepTest]
     * reads them: a JVM test cannot open a `WebView`, and what this
     * asserts is not that a page renders but that **the client holds no
     * administrative page of its own to render**. Provisioning is native
     * Kotlin ([Provision]), so it works with no node present; everything
     * that administers an instance is fetched from that instance.
     */
    // acceptance: PRD-13
    @Test
    fun the_client_ships_provisioning_and_no_administrative_page_of_its_own() {
        val root = module()
        val assets = File(root, "app/src/main/assets")
        val pages = assets.walkTopDown()
            .filter { it.isFile && it.extension.lowercase() in setOf("html", "htm", "xhtml") }
            .map { it.name }
            .toList()
        assertEquals(
            "the client ships a page of its own: administration is the instance's to serve",
            emptyList<String>(),
            pages,
        )
        // and the one screen that loads a page loads it from the instance,
        // never from this device: no `file://` and no asset URL
        val page = File(root, "app/src/main/kotlin/com/comptus/fueros/NodePageActivity.kt")
            .readText()
        assertTrue("NodePageActivity is where a served page is shown", page.contains("loadUrl"))
        // **the code and not the prose**: the first draft of this matched
        // `file://` inside a comment that says the frame cannot name one,
        // and called the documentation a violation
        val code = page.lines()
            .filterNot { it.trimStart().startsWith("//") || it.trimStart().startsWith("*") }
            .joinToString("\n")
        for (local in listOf("android_asset", "loadData(")) {
            assertTrue(
                "NodePageActivity reaches for `$local`, which would make a page the client's own",
                !code.contains(local),
            )
        }
        // and what keeps the frame off this device is a setting, not a
        // habit: the page gets no file, no content provider and no script.
        // **The settings live in [Frames] since 2026-11-08** so the
        // instrumentation run can apply the same code to a real `WebView`;
        // what this holds is that the screen reaches for them and that
        // they are what they were.
        assertTrue(
            "NodePageActivity no longer frames through Frames.node",
            code.contains("Frames.node(this)"),
        )
        val frames = File(
            module(),
            "app/src/main/kotlin/com/comptus/fueros/Frames.kt",
        ).readText()
        for (shut in listOf(
            "javaScriptEnabled = false",
            "domStorageEnabled = false",
            "allowFileAccess = false",
            "allowContentAccess = false",
        )) {
            assertTrue("the node's frame no longer sets `$shut`", frames.contains(shut))
        }
    }

}
