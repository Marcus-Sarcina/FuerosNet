// The Android shell of the light client (`mobile/README.md`): Kotlin over
// `rhtn-ffi`, not a Cargo member.  The gate runs the shell's JVM unit tests
// where the toolchain exists [author, 2026-09-27]; the APK and the native
// library stay outside it, coming from `tools/build-native.sh`.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
        // The engine a resource's page is framed in (`app/build.gradle.kts`),
        // from its maker's own repository and nothing else from there.
        maven {
            url = uri("https://maven.mozilla.org/maven2/")
            content { includeGroup("org.mozilla.geckoview") }
        }
    }
}
rootProject.name = "fueros"
include(":app")
