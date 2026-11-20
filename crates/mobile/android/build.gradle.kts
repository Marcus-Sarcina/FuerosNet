// **The plugin classpath is locked as well** [author, 2026-11-20]: the
// Android Gradle plugin and what it pulls in are most of what this build
// fetches, and `buildscript-gradle.lockfile` beside this file is where the
// gate's OSV-Scanner step reads their versions. Rewritten, with the app's
// lockfile, by `./gradlew :app:dependencies buildEnvironment --write-locks`
// after a version here changes.
buildscript {
    configurations.classpath {
        resolutionStrategy.activateDependencyLocking()
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
            "org.jetbrains.kotlin:kotlin-gradle-plugin:2.4.20",
        )
    }
}

plugins {
    // AGP 9 carries its own Kotlin support; a separate Kotlin plugin is
    // refused, not merely unneeded.
    id("com.android.application") version "9.5.2" apply false
}
