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
}
