package com.comptus.fueros

import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.SecureRandom
import uniffi.rhtn_ffi.Exchange
import uniffi.rhtn_ffi.Header
import uniffi.rhtn_ffi.ProviderTransport
import uniffi.rhtn_ffi.launchCreate
import uniffi.rhtn_ffi.launchDestroy
import uniffi.rhtn_ffi.launchProviderAsn
import uniffi.rhtn_ffi.launchProviders
import uniffi.rhtn_ffi.launchRegions
import uniffi.rhtn_ffi.launchRegistryDefault
import uniffi.rhtn_ffi.launchSizes
import uniffi.rhtn_ffi.launchStanding
import uniffi.rhtn_ffi.launchTokenPage
import uniffi.rhtn_ffi.launchTokenWalkthrough
import uniffi.rhtn_ffi.launchUserData

/**
 * **H1: launching an instance at a hosting provider** (`light-client-
 * requirements.md` §10; `infra-client-requirements.md` §8.2, §8.3; PRD-11).
 *
 * The composition — the token, the configuration, the once-run script —
 * and the provider adapters are the kernel's (`rhtn-launch`, across the
 * boundary as `launch*`), the same code the desktop's `rhtn node launch`
 * runs. What is this shell's is the one thing the kernel cannot do on a
 * phone: the HTTPS exchange, over the system's trust roots, through the
 * platform's own connection class ([HttpTransport]). This object holds no
 * key: the provider credential is the kernel's to keep (`Kernel.
 * providerCredentials`), carried in the backup under the identity. The
 * credential is a token the operator makes at the provider's own page
 * and pastes; the page and the steps there are the kernel's too.
 */
object Launch {
    /** The port the node serves on: `wire-format.md` §4.4's default. */
    const val QUIC_PORT = 7431

    /** The administration surface's port, as the image exposes it. */
    const val ADMIN_PORT = 7449

    /** The image's tag pulled unless another is named. */
    const val TAG_DEFAULT = "latest"

    /** Where the image is pulled from unless the operator names another. */
    val REGISTRY_DEFAULT: String get() = launchRegistryDefault()

    /** A fresh enrolment token: 32 random bytes as 64 hex digits, written
     *  into the instance's configuration and never sent ([Provision]). */
    fun token(): String {
        val b = ByteArray(32).also { SecureRandom().nextBytes(it) }
        return b.joinToString("") { "%02x".format(it) }
    }

    /** The once-run script, composed by the kernel. */
    fun userData(operatorMaterialHex: String, token: String, registry: String = REGISTRY_DEFAULT, tag: String = TAG_DEFAULT): String =
        launchUserData(operatorMaterialHex, token, registry, tag)

    /** What to launch: where, how big, from which registry. */
    data class Plan(
        val region: String,
        val size: String,
        val registry: String = REGISTRY_DEFAULT,
        val tag: String = TAG_DEFAULT,
    )

    data class Region(val slug: String, val name: String, val available: Boolean)

    data class Size(val slug: String, val memoryMb: Int, val vcpus: Int, val monthly: Double, val regions: List<String>)

    data class Launched(val id: String)

    data class Standing(val status: String, val ipv4: String?)

    /** One hosting provider, as this flow drives it. */
    interface Provider {
        val name: String

        /** The provider's autonomous system, for PRD-11's ordering. */
        val asn: Long

        /** The provider's own page where the operator makes a token. */
        val tokenPage: String

        /** The steps on that page, shown beside it. */
        val tokenWalkthrough: List<String>

        fun regions(credential: String): Result<List<Region>>

        fun sizes(credential: String): Result<List<Size>>

        fun launch(credential: String, plan: Plan, name: String, userData: String): Result<Launched>

        fun standing(credential: String, id: String): Result<Standing>

        fun destroy(credential: String, id: String): Result<Unit>
    }

    /** The providers the kernel drives, in the order a chooser lists them
     *  before PRD-11's ordering is applied. */
    fun providers(transport: ProviderTransport = HttpTransport()): List<Provider> =
        launchProviders().map { Bound(it, transport) }

    /** A provider bound to the kernel's adapter, over the given transport. */
    class Bound(private val key: String, private val transport: ProviderTransport) : Provider {
        override val name: String = key.replaceFirstChar { it.uppercase() }.let { if (it == "Digitalocean") "DigitalOcean" else it }
        override val asn: Long = launchProviderAsn(key)?.toLong() ?: 0L
        override val tokenPage: String = launchTokenPage(key) ?: ""
        override val tokenWalkthrough: List<String> = launchTokenWalkthrough(key)

        override fun regions(credential: String): Result<List<Region>> = runCatching {
            launchRegions(key, transport, credential).map { Region(it.slug, it.name, it.available) }
        }

        override fun sizes(credential: String): Result<List<Size>> = runCatching {
            launchSizes(key, transport, credential).map {
                Size(it.slug, it.memoryMb.toInt(), it.vcpus.toInt(), it.monthly, it.regions)
            }
        }

        override fun launch(credential: String, plan: Plan, name: String, userData: String): Result<Launched> = runCatching {
            Launched(launchCreate(key, transport, credential, plan.region, plan.size, plan.registry, plan.tag, name, userData))
        }

        override fun standing(credential: String, id: String): Result<Standing> = runCatching {
            val s = launchStanding(key, transport, credential, id)
            Standing(if (s.active) "active" else s.status, s.ipv4)
        }

        override fun destroy(credential: String, id: String): Result<Unit> = runCatching {
            launchDestroy(key, transport, credential, id)
        }
    }

    /**
     * **The platform's HTTPS, as the kernel drives it**: one request, the
     * status and the body back, and a request that did not complete
     * answered as status 0 with the failure's words, which is how the
     * kernel's `Transport` reads it. `base` lets a test point a provider at
     * a socket of its own.
     */
    class HttpTransport(private val base: String? = null) : ProviderTransport {
        override fun exchange(method: String, url: String, headers: List<Header>, body: String?): Exchange =
            runCatching {
                val target = base?.let { b -> url.replace(Regex("^https?://[^/]+"), b) } ?: url
                val c = URL(target).openConnection() as HttpURLConnection
                c.requestMethod = method
                c.connectTimeout = 15_000
                c.readTimeout = 30_000
                headers.forEach { c.setRequestProperty(it.name, it.value) }
                if (body != null) {
                    c.doOutput = true
                    c.outputStream.use { it.write(body.toByteArray()) }
                }
                val status = c.responseCode
                val stream = if (status in 200..299) c.inputStream else c.errorStream
                val text = stream?.use { s ->
                    val out = ByteArrayOutputStream()
                    s.copyTo(out)
                    out.toString("UTF-8")
                } ?: ""
                Exchange(status.toUShort(), text)
            }.getOrElse { Exchange(0u, it.message ?: it.javaClass.simpleName) }
    }
}
