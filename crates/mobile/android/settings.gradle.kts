// The Android shell of the light client (`mobile/README.md`): Kotlin over
// `rhtn-ffi`, not a Cargo member, not built by the gate.  Its native
// library and generated binding come from `tools/build-native.sh`.
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
    }
}
rootProject.name = "fueros"
include(":app")
