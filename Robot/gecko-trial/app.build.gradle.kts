plugins { id("com.android.application") }
android {
    namespace = "com.comptus.geckotrial"
    compileSdk = 37
    defaultConfig {
        applicationId = "com.comptus.geckotrial"
        minSdk = 31
        targetSdk = 37
        versionCode = 1
        versionName = "0"
        ndk.abiFilters.add("arm64-v8a")
    }
    // as a released app is: not debuggable, so nothing the engine does only for a debug build is counted
    buildTypes { getByName("debug") { isDebuggable = false } }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
dependencies {
    implementation("org.mozilla.geckoview:geckoview-arm64-v8a:161.0.20261118180411")
}
// a throwaway: the library asks for platform 37.2 and this machine has 37.0
tasks.matching { it.name.startsWith("check") && it.name.endsWith("AarMetadata") }.configureEach { enabled = false }
