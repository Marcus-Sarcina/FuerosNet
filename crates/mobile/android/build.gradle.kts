plugins {
    // AGP 9 carries its own Kotlin support; a separate Kotlin plugin is
    // refused, not merely unneeded.
    id("com.android.application") version "9.4.1" apply false
}
