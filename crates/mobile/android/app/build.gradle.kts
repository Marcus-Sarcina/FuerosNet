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
    compileSdk = 36

    defaultConfig {
        applicationId = "com.comptus.fueros"
        // UWB reached Android at API 31, and the ceremony's channels are
        // the application's reason to exist (`mobile/android/README.md`).
        minSdk = 31
        targetSdk = 36
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

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    // The generated binding speaks JNA and nothing else; the @aar carries
    // libjnidispatch.so for each ABI.
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    // The QR symbol for the optical channel (`wire-format.md` §14.3.1).
    // Pure Java and Apache-2.0, so the whole bytes-to-symbol-and-back path
    // runs in the unit tests below; what needs a camera does not.
    implementation("com.google.zxing:core:3.5.3")
    testImplementation("com.google.zxing:core:3.5.3")
    // The shell's log lines (Apache-2.0). `FuerosApp` plants a tree only in
    // the fieldtest flavour; with none planted every call is a no-op.
    implementation("com.jakewharton.timber:timber:5.0.1")
    // Unit tests hold the shell's own logic on the JVM; what needs a
    // device stays on the device.
    testImplementation("junit:junit:4.13.2")
}
