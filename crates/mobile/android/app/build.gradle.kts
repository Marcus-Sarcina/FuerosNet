plugins {
    id("com.android.application")
}

// The commit the APK was built from, for the diagnostics bundle's header.
// Read at configuration time; a tree without git says so rather than
// failing the build.
val gitCommit: String = try {
    providers.exec {
        commandLine("git", "rev-parse", "--short", "HEAD")
        isIgnoreExitValue = true
    }.standardOutput.asText.get().trim().ifEmpty { "unknown" }
} catch (_: Exception) {
    "unknown"
}

// The six specification pins the gate last accepted (`crates/spec-pins.json`),
// each shortened to twelve hex characters: `name=prefix` pairs joined by
// commas, which `Report` turns back into an object for `header.json`.
val specPins: String = providers.fileContents(
    layout.projectDirectory.file("../../../spec-pins.json"),
).asText.map { text ->
    Regex("\"([^\"]+\\.md)\"\\s*:\\s*\"([0-9a-f]{64})\"").findAll(text)
        .map { "${it.groupValues[1]}=${it.groupValues[2].take(12)}" }
        .joinToString(",")
}.getOrElse("")

android {
    namespace = "com.comptus.fueros"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.comptus.fueros"
        // API 31 is where Bluetooth LE got its own permissions (BLUETOOTH_SCAN,
        // CONNECT, ADVERTISE, with the neverForLocation flag): below it a BLE
        // scan needs the location permission, which this app has no use for
        // and would have to explain. The floor was first set here for UWB;
        // ranging is gone [author, 2026-10-24] and the bearer keeps it.
        minSdk = 31
        targetSdk = 37
        // **the instrumentation run** (PRD-12, RSC-58): frame isolation
        // wants a real `WebView` and a hostile page, which no JVM test
        // reaches. Not part of the gate, which has no device: run it with
        // `./gradlew connectedFieldtestDebugAndroidTest` against a bench
        // phone (`Robot/field-test-sequence.md`).
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        versionCode = 1
        versionName = "0.1"
        // Only the ABIs the kernel is built for: JNA's @aar carries its
        // dispatch library for more, and an APK installable on an ABI the
        // kernel does not cover dies at first load.
        ndk.abiFilters.addAll(listOf("arm64-v8a", "x86_64"))
        buildConfigField("String", "GIT_COMMIT", "\"$gitCommit\"")
        buildConfigField("String", "SPEC_PINS", "\"$specPins\"")
    }

    // The two flavours of one shell (`Robot/field-test-diagnostics.md`,
    // section 4), named as the Cargo features of `rhtn-ffi` are. Each
    // carries its own `librhtn_ffi.so` under `src/<flavour>/jniLibs`,
    // written by `tools/build-native.sh`; the Kotlin binding is one shape
    // for both. `releasable` plants no log tree, writes no event file and
    // shows no Report action; `fieldtest` does all three. The flavour is
    // not named `release` because a flavour may not share a build type's
    // name.
    buildFeatures {
        buildConfig = true
    }
    flavorDimensions += "diag"
    productFlavors {
        create("releasable") {
            dimension = "diag"
            buildConfigField("boolean", "FIELD_TEST", "false")
        }
        create("fieldtest") {
            dimension = "diag"
            buildConfigField("boolean", "FIELD_TEST", "true")
        }
    }

    sourceSets["main"].kotlin.srcDir("src/generated/kotlin")
    // **the typefaces every client shares** (`crates/brand`): kept once there
    // and read from there, so the desktop and this shell draw the same files
    sourceSets["main"].res.srcDir("../../../brand/res")

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

// **The manifests are an input of the unit tests** [2026-10-27], because
// `ManifestTest` reads them as text: it holds that every Activity a source
// names is declared, and the one defect it is written for is a manifest
// that stopped naming one. Without this the task is up to date after a
// manifest-only edit and the stale result reads as a pass -- which it did,
// the first time this was checked by taking a declaration back out.
tasks.withType<Test>().configureEach {
    inputs.files(fileTree("src") { include("**/AndroidManifest.xml") })
        .withPropertyName("androidManifests")
        .withPathSensitivity(PathSensitivity.RELATIVE)
}

// **Every configuration resolves against a lockfile** [author,
// 2026-11-20]: `gradle.lockfile` beside this file names the exact version
// of everything the shell resolves, test classpaths included, so a
// dependency moves only by a commit that rewrites it, and the gate's
// OSV-Scanner step reads the versions the APK is built from. Rewritten by
// `./gradlew :app:dependencies buildEnvironment --write-locks` from
// `mobile/android` after a change below.
dependencyLocking {
    lockAllConfigurations()
}

configurations.all {
    // **the patched releases of what the plugin's tooling pulls in**
    // [author, 2026-11-20: the toolchain is not to be locked to known
    // advisories while there is no release to keep stable]: each is
    // the version the advisory names as fixed, forced over the one
    // the plugin asks for. Build tooling only; none reaches the APK.
    resolutionStrategy.force(
        "com.google.guava:guava:33.7.2-jre",
        "org.apache.commons:commons-lang3:3.18.0",
        "org.apache.httpcomponents:httpclient:4.5.13",
        "org.bouncycastle:bcpkix-jdk18on:1.85",
        "org.bouncycastle:bcprov-jdk18on:1.85",
        "org.bouncycastle:bcutil-jdk18on:1.85",
        "org.bitbucket.b_c:jose4j:0.9.6",
        "org.jdom:jdom2:2.0.6.1",
    )
}

dependencies {
    // The generated binding speaks JNA and nothing else; the @aar carries
    // libjnidispatch.so for each ABI.
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    // The QR symbol for the optical channel (`wire-format.md` §14.3.1).
    // Pure Java and Apache-2.0, so the whole bytes-to-symbol-and-back path
    // runs in the unit tests below; what needs a camera does not.
    implementation("com.google.zxing:core:3.5.4")
    testImplementation("com.google.zxing:core:3.5.4")
    // The shell's log lines (Apache-2.0). `FuerosApp` plants a tree only in
    // the fieldtest flavour; with none planted every call is a no-op.
    implementation("com.jakewharton.timber:timber:5.0.1")
    // Unit tests hold the shell's own logic on the JVM; what needs a
    // device stays on the device.
    testImplementation("junit:junit:4.13.2")
    // The instrumentation run, which needs a device: the frame isolation
    // PRD-12 and RSC-58 rest on is a real `WebView`'s behaviour and not
    // the shell's logic. Apache-2.0, like everything else here.
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test:rules:1.7.0")
}
