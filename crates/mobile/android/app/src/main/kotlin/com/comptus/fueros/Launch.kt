package com.comptus.fueros

import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.SecureRandom

/**
 * **H1: launching an instance at a hosting provider** (`light-client-
 * requirements.md` §10; `infra-client-requirements.md` §8.2, §8.3; PRD-11).
 *
 * What `packaging/local.sh` does on this machine, done at a provider: mint
 * the enrolment token, compose the configuration the instance cannot
 * supply for itself (the entrypoint "invents no configuration"), hand both
 * to a fresh virtual machine as cloud-init user-data, and wait for the
 * address to come back. The machine installs a container engine, pulls the
 * image from the registry and runs it; the instance mints its transport
 * key and waits; and the enrolment that already exists ([Provision],
 * `Kernel.enrol`) signs the run over that key once the proof checks.
 *
 * **This object holds the exchange and no keys** [author, 2026-10-25].
 * The provider credential it speaks with is the kernel's to keep, carried
 * in the backup under the identity and never in the archive, and the
 * operator's recourse for a compromised one is the provider's own console.
 * No broker stands between this client and the provider.
 *
 * **One shape for every provider that fits**: a machine from an image, a
 * region and a size, given a script, giving back a public IPv4 (design
 * §17.3 demands one). The DigitalOcean API is the shape the budget
 * providers copy, so it is the first adapter and the pattern for Hetzner,
 * Vultr and BitLaunch; AWS and GCP speak the same primitive under heavier
 * signing, and come after.
 */
object Launch {
    /** The port the node serves on: `wire-format.md` §4.4's default, so an
     *  endpoint record may omit it. */
    const val QUIC_PORT = 7431

    /** The administration surface's port, as the image exposes it. */
    const val ADMIN_PORT = 7449

    /** The image's fixed uid (`packaging/Containerfile`), which the state
     *  volume is chowned to before the container runs. */
    const val IMAGE_UID = 10001

    /** **Where the image is pulled from by default**, and the advanced
     *  option is to name another for a custom or third-party node
     *  [author, 2026-10-25]. */
    const val REGISTRY_DEFAULT = "registry.gitgud.io/mzuech/fuerosnet"
    const val IMAGE = "rhtnd"
    const val TAG_DEFAULT = "latest"

    /** The provider's machine image: Debian 12, which carries podman. */
    const val MACHINE_IMAGE_DEFAULT = "debian-12-x64"

    /** A fresh enrolment token: 32 random bytes as 64 hex digits, written
     *  into the instance's configuration and never sent ([Provision]). */
    fun token(): String {
        val b = ByteArray(32).also { SecureRandom().nextBytes(it) }
        return b.joinToString("") { "%02x".format(it) }
    }

    /**
     * **The instance's configuration**, the one `local.sh` writes and the
     * daemon requires: the operator it speaks as, the paths under its
     * state directory, the address it serves on, and the administration
     * surface with its token. Every field is a choice the operator makes
     * (`infra-client-requirements.md` §1); these are this client's.
     */
    fun nodeConfig(token: String, quic: Int = QUIC_PORT, admin: Int = ADMIN_PORT): String = """
        |operator = "/var/lib/rhtn/operator.material"
        |transport-key = "/var/lib/rhtn/transport.key"
        |delegations = "/var/lib/rhtn/delegations"
        |endpoint-record = "/var/lib/rhtn/endpoint.record"
        |anchor-entry = "/var/lib/rhtn/anchor.entry"
        |listen = "0.0.0.0:$quic"
        |queue = "/var/lib/rhtn/queue"
        |prekeys = "/var/lib/rhtn/prekeys"
        |topology = "/var/lib/rhtn/topology"
        |archive = "/var/lib/rhtn/archive"
        |grants = "/var/lib/rhtn/grants"
        |heartbeat = 30
        |ingestion = "unverified-gossip"
        |[allowance]
        |requests = 120
        |seconds = 60
        |[administration]
        |listen = "0.0.0.0:$admin"
        |token = "$token"
        |""".trimMargin()

    /**
     * **The script the machine runs once, as cloud-init user-data**: the
     * engine installed, the configuration and the operator's public
     * material written, the state directory handed to the image's uid, and
     * the container started with the host's network and restarted with the
     * machine. The operator's seed is in none of it (design §23.3).
     */
    fun userData(
        operatorMaterialHex: String,
        token: String,
        registry: String = REGISTRY_DEFAULT,
        tag: String = TAG_DEFAULT,
        quic: Int = QUIC_PORT,
        admin: Int = ADMIN_PORT,
    ): String {
        val conf = nodeConfig(token, quic, admin).lines().joinToString("\n") { "      $it" }
        val image = "${registry.trimEnd('/')}/$IMAGE:$tag"
        return """
            |#cloud-config
            |package_update: true
            |packages:
            |  - podman
            |write_files:
            |  - path: /var/lib/rhtn/rhtnd.conf
            |    permissions: '0600'
            |    content: |
            |$conf
            |  - path: /var/lib/rhtn/operator.material
            |    permissions: '0644'
            |    content: |
            |      $operatorMaterialHex
            |runcmd:
            |  - chown -R $IMAGE_UID:$IMAGE_UID /var/lib/rhtn
            |  - chmod 0700 /var/lib/rhtn
            |  - systemctl enable --now podman-restart.service
            |  - podman run -d --name rhtnd --restart=always --network=host -v /var/lib/rhtn:/var/lib/rhtn $image
            |""".trimMargin()
    }

    /** What to launch: where, how big, from which image and registry. */
    data class Plan(
        val region: String,
        val size: String,
        val machineImage: String = MACHINE_IMAGE_DEFAULT,
        val registry: String = REGISTRY_DEFAULT,
        val tag: String = TAG_DEFAULT,
    )

    data class Region(val slug: String, val name: String, val available: Boolean)

    data class Size(val slug: String, val memoryMb: Int, val vcpus: Int, val monthly: Double, val regions: List<String>)

    /** A machine the provider created: its id there, for the status and
     *  the destroy. */
    data class Launched(val id: String)

    /** Where a machine stands: the provider's status word and its public
     *  IPv4 once it has one. */
    data class Standing(val status: String, val ipv4: String?)

    /** One hosting provider, as this flow drives it. */
    interface Provider {
        val name: String

        /** The provider's autonomous system, for PRD-11's ordering. */
        val asn: Long

        fun regions(credential: String): Result<List<Region>>

        fun sizes(credential: String): Result<List<Size>>

        fun launch(credential: String, plan: Plan, name: String, userData: String): Result<Launched>

        fun standing(credential: String, id: String): Result<Standing>

        fun destroy(credential: String, id: String): Result<Unit>
    }

    /** The providers this client drives, in the order a chooser lists
     *  them before PRD-11's ordering is applied. */
    fun providers(): List<Provider> = listOf(DigitalOcean())

    /**
     * **DigitalOcean**, API v2: a bearer token, JSON in and out, a droplet
     * from an image, a region and a size with `user_data`, and its public
     * IPv4 in `networks.v4` once `status` is `active`.
     */
    class DigitalOcean(private val base: String = "https://api.digitalocean.com") : Provider {
        override val name = "DigitalOcean"
        override val asn = 14061L

        override fun regions(credential: String): Result<List<Region>> =
            exchange(credential, "GET", "/v2/regions?per_page=200", null).map { body ->
                Json.list(Json.obj(Json.parse(body))["regions"]).map { r ->
                    val m = Json.obj(r)
                    Region(
                        Json.string(m["slug"]) ?: "",
                        Json.string(m["name"]) ?: "",
                        Json.bool(m["available"]) ?: false,
                    )
                }.filter { it.slug.isNotEmpty() }
            }

        override fun sizes(credential: String): Result<List<Size>> =
            exchange(credential, "GET", "/v2/sizes?per_page=200", null).map { body ->
                Json.list(Json.obj(Json.parse(body))["sizes"]).mapNotNull { s ->
                    val m = Json.obj(s)
                    val slug = Json.string(m["slug"]) ?: return@mapNotNull null
                    if (Json.bool(m["available"]) == false) return@mapNotNull null
                    Size(
                        slug,
                        Json.number(m["memory"])?.toInt() ?: 0,
                        Json.number(m["vcpus"])?.toInt() ?: 0,
                        Json.number(m["price_monthly"]) ?: 0.0,
                        Json.list(m["regions"]).mapNotNull { Json.string(it) },
                    )
                }
            }

        override fun launch(credential: String, plan: Plan, name: String, userData: String): Result<Launched> {
            val request = "{" +
                "\"name\":${Json.quote(name)}," +
                "\"region\":${Json.quote(plan.region)}," +
                "\"size\":${Json.quote(plan.size)}," +
                "\"image\":${Json.quote(plan.machineImage)}," +
                "\"ipv6\":false,\"monitoring\":false," +
                "\"tags\":[\"rhtn\"]," +
                "\"user_data\":${Json.quote(userData)}" +
                "}"
            return exchange(credential, "POST", "/v2/droplets", request).mapCatching { body ->
                val d = Json.obj(Json.obj(Json.parse(body))["droplet"])
                val id = Json.number(d["id"])?.toLong() ?: error("the answer named no droplet id")
                Launched(id.toString())
            }
        }

        override fun standing(credential: String, id: String): Result<Standing> =
            exchange(credential, "GET", "/v2/droplets/$id", null).mapCatching { body ->
                val d = Json.obj(Json.obj(Json.parse(body))["droplet"])
                val v4 = Json.list(Json.obj(d["networks"])["v4"])
                val public = v4.map { Json.obj(it) }.firstOrNull { Json.string(it["type"]) == "public" }
                Standing(Json.string(d["status"]) ?: "unknown", public?.let { Json.string(it["ip_address"]) })
            }

        override fun destroy(credential: String, id: String): Result<Unit> =
            exchange(credential, "DELETE", "/v2/droplets/$id", null).map { }

        /**
         * One request to the provider, the body JSON where there is one,
         * the answer's text back, or the provider's own `message` where it
         * refused: a refusal is an answer with words in it.
         */
        private fun exchange(credential: String, method: String, path: String, body: String?): Result<String> =
            runCatching {
                val c = URL("$base$path").openConnection() as HttpURLConnection
                c.requestMethod = method
                c.connectTimeout = 15_000
                c.readTimeout = 30_000
                c.setRequestProperty("Authorization", "Bearer $credential")
                c.setRequestProperty("Accept", "application/json")
                if (body != null) {
                    c.doOutput = true
                    c.setRequestProperty("Content-Type", "application/json")
                    c.outputStream.use { it.write(body.toByteArray()) }
                }
                val status = c.responseCode
                val stream = if (status in 200..299) c.inputStream else c.errorStream
                val text = stream?.use { s ->
                    val out = ByteArrayOutputStream()
                    s.copyTo(out)
                    out.toString("UTF-8")
                } ?: ""
                if (status !in 200..299) {
                    val why = runCatching { Json.string(Json.obj(Json.parse(text))["message"]) }.getOrNull()
                    error("$name answered $status${if (why != null) ": $why" else ""}")
                }
                text
            }
    }
}
