plugins {
    id("com.android.application")
}

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
    // Unit tests hold the shell's own logic on the JVM; what needs a
    // device stays on the device.
    testImplementation("junit:junit:4.13.2")
}
