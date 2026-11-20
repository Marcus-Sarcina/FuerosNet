// **The plugin classpath is locked as well** [author, 2026-11-20]: the
// Android Gradle plugin and what it pulls in are most of what this build
// fetches, and `buildscript-gradle.lockfile` beside this file is where the
// gate's OSV-Scanner step reads their versions. Rewritten, with the app's
// lockfile, by `./gradlew :app:dependencies buildEnvironment --write-locks`
// after a version here changes.
buildscript {
    configurations.classpath {
        resolutionStrategy.activateDependencyLocking()
    }
}

plugins {
    // AGP 9 carries its own Kotlin support; a separate Kotlin plugin is
    // refused, not merely unneeded.
    id("com.android.application") version "9.4.1" apply false
}
