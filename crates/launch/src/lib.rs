//! **H1: launching an instance at a hosting provider**
//! (`light-client-requirements.md` §10; `infra-client-requirements.md`
//! §8.2, §8.3; PRD-11; OPS-020).
//!
//! What `packaging/local.sh` does on one machine, done at a provider: mint
//! the enrolment token, compose the configuration the instance cannot
//! supply for itself (the entrypoint invents none), hand both to a fresh
//! virtual machine as the script it runs once, and wait for the address
//! to come back.  The machine installs a container engine, pulls the image
//! from the registry and runs it; the instance mints its transport key and
//! waits; and the enrolment that already exists signs the run over that
//! key once the proof checks.
//!
//! **One shape for every provider that fits**: a machine from an image, a
//! region and a size, given a script, giving back a public IPv4 (design
//! §17.3 demands one).  DigitalOcean's API is the shape the budget
//! providers copy, so it was the first [`Provider`]; [`Hetzner`] is the
//! second and the proof that the shape travels — four of its differences
//! are named in its own documentation, each a place where believing the
//! first provider would have been wrong.  Vultr and BitLaunch copy the
//! same shape; Njalla speaks it in another dialect; AWS and GCP speak the
//! same primitive under heavier signing.
//!
//! **No network of its own.**  Every provider speaks HTTPS, and where a
//! trust root comes from is the platform's: the kernel ships none by
//! design, a phone has the system's, a desktop has the operating system's.
//! So this crate composes requests and reads answers over a [`Transport`]
//! the caller supplies — the shell's own connection class on Android, a
//! TLS client over the system roots on a desktop, a socket in a test — and
//! holds no credential: what a provider is spoken to with is the kernel's
//! to keep, in the backup under the identity (design §13.7.1, ARC-28), and
//! the operator's recourse for a leaked one is the provider's console
//! [author, 2026-10-25].  No broker stands between the client and the
//! provider.
//!
//! **The credential is a token the operator makes at the provider's own
//! console and pastes** [author, 2026-10-25]: no application is
//! registered with any provider on the project's behalf, so the client
//! ships nothing that names the project to a provider and nothing of the
//! project's stands between the operator and the provider.  An
//! authorisation flow was tried and withdrawn the same day: DigitalOcean
//! refuses the exchange without the application's secret, and a secret
//! in a shipped client is public.  What the adapter supplies instead is
//! where the token is made and the steps there
//! ([`Provider::token_page`], [`Provider::token_walkthrough`]), so a
//! client can open the page and show the steps beside it.

#![warn(missing_docs)]

use serde_json::{Value, json};

/// The port the node serves on: `wire-format.md` §4.4's default, so an
/// endpoint record may omit it.
pub const QUIC_PORT: u16 = 7431;
/// The administration surface's port, as the image exposes it.
pub const ADMIN_PORT: u16 = 7449;
/// The image's fixed uid (`packaging/Containerfile`), which the state
/// volume is chowned to before the container runs.
pub const IMAGE_UID: u32 = 10001;
/// **Where the image is pulled from unless the operator names another**
/// [author, 2026-10-25]: the project's registry, as its page names it.
pub const REGISTRY_DEFAULT: &str = "ggcr.io/mzuech/fuerosnet";
/// The image's name under the registry.
pub const IMAGE: &str = "rhtnd";
/// The tag pulled unless another is named.
pub const TAG_DEFAULT: &str = "latest";
/// The distribution a machine is launched from unless the plan names an
/// image: Debian, which carries podman.  Which release is the
/// provider's to say, asked at launch ([`Provider::machine_images`],
/// [`newest_debian`]) rather than named here, since a slug named here
/// outlives the provider's offer of it (DigitalOcean refused
/// `debian-12-x64` as "an invalid image" on 2026-10-25).
pub const MACHINE_DISTRIBUTION: &str = "Debian";

/// A machine image the provider offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineImage {
    /// Its slug, as a plan names it.
    pub slug: String,
    /// The distribution, as the provider names it.
    pub distribution: String,
    /// Its name, for a person.
    pub name: String,
    /// **Whether it runs on the 64-bit x86 machines this launch asks
    /// for.**  Each provider says so in its own way — a suffix on the
    /// slug at DigitalOcean, a field at Hetzner — and the adapter reads
    /// its own; what this flow needs is one answer, because an image of
    /// the wrong architecture is accepted at the plan and refused at the
    /// machine.
    pub amd64: bool,
}

/// The newest Debian among `images` that runs on this flow's machines, by
/// the release number its slug carries after `debian-`; none where there
/// is no such Debian.
///
/// **The slug's spelling is the provider's**, which is why the release is
/// read off the front rather than matched whole: DigitalOcean writes
/// `debian-13-x64` and Hetzner writes `debian-13`, and a pattern fitted
/// to one of them silently finds nothing at the other.
pub fn newest_debian(images: &[MachineImage]) -> Option<String> {
    images
        .iter()
        .filter(|i| i.amd64 && i.distribution.eq_ignore_ascii_case(MACHINE_DISTRIBUTION))
        .filter_map(|i| {
            let rest = i.slug.strip_prefix("debian-")?;
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u32>().ok().map(|v| (v, i.slug.clone()))
        })
        .max()
        .map(|(_, slug)| slug)
}

/// A fresh enrolment token as 64 hex digits, from `random`: written into
/// the instance's configuration and never sent (`rhtn_crypto::enrolment`).
pub fn token_hex(random: &[u8; 32]) -> String {
    random.iter().map(|b| format!("{b:02x}")).collect()
}

/// **The instance's configuration**, the one `local.sh` writes and the
/// daemon requires: the operator it speaks as, the paths under its state
/// directory, the address it serves on, and the administration surface
/// with its token.  Every field is a choice the operator makes
/// (`infra-client-requirements.md` §1); these are this client's.
pub fn node_config(token_hex: &str, quic: u16, admin: u16) -> String {
    format!(
        "operator = \"/var/lib/rhtn/operator.material\"\n\
         transport-key = \"/var/lib/rhtn/transport.key\"\n\
         delegations = \"/var/lib/rhtn/delegations\"\n\
         endpoint-record = \"/var/lib/rhtn/endpoint.record\"\n\
         anchor-entry = \"/var/lib/rhtn/anchor.entry\"\n\
         listen = \"0.0.0.0:{quic}\"\n\
         queue = \"/var/lib/rhtn/queue\"\n\
         prekeys = \"/var/lib/rhtn/prekeys\"\n\
         topology = \"/var/lib/rhtn/topology\"\n\
         archive = \"/var/lib/rhtn/archive\"\n\
         grants = \"/var/lib/rhtn/grants\"\n\
         heartbeat = 30\n\
         ingestion = \"unverified-gossip\"\n\
         [allowance]\n\
         requests = 120\n\
         seconds = 60\n\
         [administration]\n\
         listen = \"0.0.0.0:{admin}\"\n\
         token = \"{token_hex}\"\n\
         [federation]\n\
         store = \"/var/lib/rhtn/federation\"\n"
    )
}

/// Where the image is pulled from and how it is run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// The registry host and path.
    pub registry: String,
    /// The tag pulled.
    pub tag: String,
}

impl Default for Image {
    fn default() -> Self {
        Image {
            registry: REGISTRY_DEFAULT.into(),
            tag: TAG_DEFAULT.into(),
        }
    }
}

impl Image {
    /// The reference the engine pulls.
    pub fn reference(&self) -> String {
        format!(
            "{}/{IMAGE}:{}",
            self.registry.trim_end_matches('/'),
            self.tag
        )
    }
}

/// **The script the machine runs once, as cloud-init user-data**: the
/// engine installed, the configuration and the operator's public material
/// written, the state directory handed to the image's uid, and the
/// container started on the host's network and restarted with the machine.
/// The operator's seed is in none of it (design §23.3).
pub fn user_data(
    operator_material_hex: &str,
    token_hex: &str,
    image: &Image,
    quic: u16,
    admin: u16,
) -> String {
    let conf: String = node_config(token_hex, quic, admin)
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect();
    format!(
        "#cloud-config\n\
         package_update: true\n\
         packages:\n\
         \x20 - podman\n\
         write_files:\n\
         \x20 - path: /var/lib/rhtn/rhtnd.conf\n\
         \x20   permissions: '0600'\n\
         \x20   content: |\n\
         {conf}\
         \x20 - path: /var/lib/rhtn/operator.material\n\
         \x20   permissions: '0644'\n\
         \x20   content: |\n\
         \x20     {operator_material_hex}\n\
         runcmd:\n\
         \x20 - chown -R {IMAGE_UID}:{IMAGE_UID} /var/lib/rhtn\n\
         \x20 - chmod 0700 /var/lib/rhtn\n\
         \x20 - systemctl enable --now podman-restart.service\n\
         \x20 - podman run -d --name rhtnd --restart=always --network=host -v /var/lib/rhtn:/var/lib/rhtn {}\n",
        image.reference()
    )
}

/// One HTTP exchange, as the platform makes it: the method, the full URL,
/// the headers to send, the body where there is one; back come the status
/// and the body's text.  A transport failure is a string in the caller's
/// words.
pub trait Transport {
    /// The exchange: status and body back, or why not.
    fn exchange(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> Result<(u16, String), String>;
}

/// What to launch: where, how big, from which machine image, running
/// which node image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The provider's region slug.
    pub region: String,
    /// The provider's size slug.
    pub size: String,
    /// The provider's machine image, or none for the newest Debian the
    /// provider offers, found at launch.
    pub machine_image: Option<String>,
    /// The node image to run.
    pub image: Image,
}

impl Plan {
    /// A plan at the default images.
    pub fn new(region: &str, size: &str) -> Plan {
        Plan {
            region: region.into(),
            size: size.into(),
            machine_image: None,
            image: Image::default(),
        }
    }
}

/// A region the provider offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    /// Its slug, as the plan names it.
    pub slug: String,
    /// Its name, for a person.
    pub name: String,
    /// Whether it takes new machines now.
    pub available: bool,
}

/// A size the provider offers.
#[derive(Debug, Clone, PartialEq)]
pub struct Size {
    /// Its slug, as the plan names it.
    pub slug: String,
    /// Memory, in megabytes.
    pub memory_mb: u64,
    /// Virtual processors.
    pub vcpus: u64,
    /// The provider's monthly price, in its currency.
    pub monthly: f64,
    /// The regions it is offered in.
    pub regions: Vec<String>,
}

/// A machine the provider created: its id there, for the standing and
/// the destroy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launched {
    /// The provider's id for the machine.
    pub id: String,
}

/// Where a machine stands: the provider's status word and its public IPv4
/// once it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// The provider's status word.
    pub status: String,
    /// The public IPv4, once assigned.
    pub ipv4: Option<String>,
    /// Whether the provider says it is running: what a poll waits for.
    pub active: bool,
}

/// One hosting provider, as this flow drives it.
pub trait Provider {
    /// The provider's name, for a person.
    fn name(&self) -> &'static str;
    /// The provider's autonomous system, for PRD-11's ordering.
    fn asn(&self) -> u64;
    /// Where an operator makes the token this adapter is spoken to with:
    /// the provider's own page, for a client to open.
    fn token_page(&self) -> &'static str;
    /// The steps on that page, for a client to show beside it: what to
    /// choose so the token can do what the launch needs and no more.
    fn token_walkthrough(&self) -> &'static [&'static str];
    /// The machine images offered: the distributions, not an operator's
    /// own.
    fn machine_images(
        &self,
        t: &dyn Transport,
        credential: &str,
    ) -> Result<Vec<MachineImage>, String>;
    /// The regions offered.
    fn regions(&self, t: &dyn Transport, credential: &str) -> Result<Vec<Region>, String>;
    /// The sizes offered.
    fn sizes(&self, t: &dyn Transport, credential: &str) -> Result<Vec<Size>, String>;
    /// Create a machine running `user_data` once.
    fn launch(
        &self,
        t: &dyn Transport,
        credential: &str,
        plan: &Plan,
        name: &str,
        user_data: &str,
    ) -> Result<Launched, String>;
    /// Where a machine stands now.
    fn standing(&self, t: &dyn Transport, credential: &str, id: &str) -> Result<Standing, String>;
    /// Destroy a machine.
    fn destroy(&self, t: &dyn Transport, credential: &str, id: &str) -> Result<(), String>;
}

/// The providers this crate drives, by the name a credential is filed
/// under; `None` for a name it does not know.
pub fn provider(name: &str) -> Option<Box<dyn Provider>> {
    match name.to_ascii_lowercase().as_str() {
        "digitalocean" => Some(Box::new(DigitalOcean::default())),
        "hetzner" => Some(Box::new(Hetzner::default())),
        _ => None,
    }
}

/// Every provider's name, in the order a chooser lists them before
/// PRD-11's ordering is applied.
pub fn provider_names() -> Vec<&'static str> {
    vec!["digitalocean", "hetzner"]
}

/// The steps on DigitalOcean's token page, as a client shows them.  The
/// scopes are the launch's needs and nothing beyond: a droplet made, read
/// while it boots and destroyed when the operator says; the regions and
/// sizes read to choose from.
const DIGITALOCEAN_TOKEN_STEPS: &[&str] = &[
    "Sign in, or sign up; DigitalOcean asks for a payment method before it makes a machine.",
    "Under API, Tokens, choose Generate New Token.",
    "Name the token for this client and give it an expiry: the client holds it until then, and you make another the same way afterwards.",
    "Under Scopes choose Custom Scopes and tick, under droplet: create, read and delete; under image: read; under regions: read; under sizes: read. Full Access also works and gives the token more than the launch needs.",
    "Generate it, copy the token shown (it is shown once), and paste it here.",
];

/// **DigitalOcean**, API v2: a bearer token, JSON in and out, a droplet
/// from an image, a region and a size with `user_data`, and its public
/// IPv4 in `networks.v4` once `status` is `active`.
#[derive(Debug, Clone)]
pub struct DigitalOcean {
    /// The API's base URL; a test's server stands in.
    pub base: String,
}

impl Default for DigitalOcean {
    fn default() -> Self {
        DigitalOcean {
            base: "https://api.digitalocean.com".into(),
        }
    }
}

impl DigitalOcean {
    /// One request, the provider's own `message` surfaced where it
    /// refused: a refusal is an answer with words in it.
    fn ask(
        &self,
        t: &dyn Transport,
        credential: &str,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<Value, String> {
        let auth = format!("Bearer {credential}");
        let mut headers: Vec<(&str, &str)> =
            vec![("Authorization", &auth), ("Accept", "application/json")];
        if body.is_some() {
            headers.push(("Content-Type", "application/json"));
        }
        let (status, text) = t.exchange(method, &format!("{}{path}", self.base), &headers, body)?;
        if !(200..300).contains(&status) {
            let why = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from));
            return Err(match why {
                Some(w) => format!("DigitalOcean answered {status}: {w}"),
                None => format!("DigitalOcean answered {status}"),
            });
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|e| format!("DigitalOcean's answer did not read: {e}"))
    }
}

impl Provider for DigitalOcean {
    fn name(&self) -> &'static str {
        "DigitalOcean"
    }

    fn asn(&self) -> u64 {
        14061
    }

    fn token_page(&self) -> &'static str {
        "https://cloud.digitalocean.com/account/api/tokens/new"
    }

    fn token_walkthrough(&self) -> &'static [&'static str] {
        DIGITALOCEAN_TOKEN_STEPS
    }

    fn machine_images(
        &self,
        t: &dyn Transport,
        credential: &str,
    ) -> Result<Vec<MachineImage>, String> {
        let v = self.ask(
            t,
            credential,
            "GET",
            "/v2/images?type=distribution&per_page=200",
            None,
        )?;
        Ok(v["images"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|i| {
                        Some(MachineImage {
                            amd64: i["slug"].as_str()?.ends_with("-x64"),
                            slug: i["slug"].as_str()?.to_string(),
                            distribution: i["distribution"].as_str().unwrap_or("").to_string(),
                            name: i["name"].as_str().unwrap_or("").to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn regions(&self, t: &dyn Transport, credential: &str) -> Result<Vec<Region>, String> {
        let v = self.ask(t, credential, "GET", "/v2/regions?per_page=200", None)?;
        Ok(v["regions"]
            .as_array()
            .map(|rs| {
                rs.iter()
                    .filter_map(|r| {
                        Some(Region {
                            slug: r["slug"].as_str()?.to_string(),
                            name: r["name"].as_str().unwrap_or("").to_string(),
                            available: r["available"].as_bool().unwrap_or(false),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn sizes(&self, t: &dyn Transport, credential: &str) -> Result<Vec<Size>, String> {
        let v = self.ask(t, credential, "GET", "/v2/sizes?per_page=200", None)?;
        Ok(v["sizes"]
            .as_array()
            .map(|ss| {
                ss.iter()
                    .filter(|s| s["available"].as_bool() != Some(false))
                    .filter_map(|s| {
                        Some(Size {
                            slug: s["slug"].as_str()?.to_string(),
                            memory_mb: s["memory"].as_u64().unwrap_or(0),
                            vcpus: s["vcpus"].as_u64().unwrap_or(0),
                            monthly: s["price_monthly"].as_f64().unwrap_or(0.0),
                            regions: s["regions"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|x| x.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn launch(
        &self,
        t: &dyn Transport,
        credential: &str,
        plan: &Plan,
        name: &str,
        user_data: &str,
    ) -> Result<Launched, String> {
        let image = match &plan.machine_image {
            Some(i) => i.clone(),
            None => newest_debian(&self.machine_images(t, credential)?).ok_or_else(|| {
                "DigitalOcean lists no Debian image to this token; a token with custom scopes needs image: read"
                    .to_string()
            })?,
        };
        let body = json!({
            "name": name,
            "region": plan.region,
            "size": plan.size,
            "image": image,
            "ipv6": false,
            "monitoring": false,
            "tags": ["rhtn"],
            "user_data": user_data,
        });
        let v = self.ask(
            t,
            credential,
            "POST",
            "/v2/droplets",
            Some(&body.to_string()),
        )?;
        let id = v["droplet"]["id"]
            .as_u64()
            .ok_or("the answer named no droplet id")?;
        Ok(Launched { id: id.to_string() })
    }

    fn standing(&self, t: &dyn Transport, credential: &str, id: &str) -> Result<Standing, String> {
        let v = self.ask(t, credential, "GET", &format!("/v2/droplets/{id}"), None)?;
        let d = &v["droplet"];
        let status = d["status"].as_str().unwrap_or("unknown").to_string();
        let ipv4 = d["networks"]["v4"].as_array().and_then(|nets| {
            nets.iter()
                .find(|n| n["type"].as_str() == Some("public"))
                .and_then(|n| n["ip_address"].as_str().map(String::from))
        });
        Ok(Standing {
            active: status == "active",
            status,
            ipv4,
        })
    }

    fn destroy(&self, t: &dyn Transport, credential: &str, id: &str) -> Result<(), String> {
        self.ask(t, credential, "DELETE", &format!("/v2/droplets/{id}"), None)
            .map(|_| ())
    }
}

/// The steps on Hetzner's token page, as a client shows them.  A token
/// is a project's and carries one of two permissions, so the walkthrough
/// names the project and the read-and-write choice rather than scopes
/// Hetzner does not offer.
const HETZNER_TOKEN_STEPS: &[&str] = &[
    "Sign in, or sign up; Hetzner asks for a payment method, and a first-time account can take a while to be cleared for use.",
    "Make a project, or open the one this node belongs in: a token belongs to one project and reaches nothing outside it, which is the narrowest a Hetzner token gets.",
    "In that project open Security, then API tokens, and choose Generate API token.",
    "Name the token for this client and give it Read & Write: the launch creates a machine, reads it while it boots and destroys it when you say, and Read alone cannot.",
    "Generate it, copy the token shown (it is shown once), and paste it here.",
];

/// **Hetzner Cloud**, API v1: a bearer token, JSON in and out, a server
/// from a system image, a location and a server type with `user_data`,
/// and its public IPv4 in `public_net.ipv4.ip` once `status` is
/// `running`.
///
/// **The same shape as DigitalOcean's, in another dialect** — which is
/// what the [`Provider`] trait is for. Four differences are the adapter's
/// to absorb, each a place where believing the first provider would have
/// been wrong: a token is a *project's* rather than an account's; memory
/// is stated in gigabytes and a price in a string of euros, per location;
/// an image's architecture is a field rather than a suffix on its name;
/// and a server type is offered per location, with the ones it has been
/// withdrawn from still listed.
#[derive(Debug, Clone)]
pub struct Hetzner {
    /// The API's base URL; a test's server stands in.
    pub base: String,
}

impl Default for Hetzner {
    fn default() -> Self {
        Hetzner {
            base: "https://api.hetzner.cloud".into(),
        }
    }
}

impl Hetzner {
    /// One request, the provider's own `error.message` surfaced where it
    /// refused.
    fn ask(
        &self,
        t: &dyn Transport,
        credential: &str,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<Value, String> {
        let auth = format!("Bearer {credential}");
        let mut headers: Vec<(&str, &str)> =
            vec![("Authorization", &auth), ("Accept", "application/json")];
        if body.is_some() {
            headers.push(("Content-Type", "application/json"));
        }
        let (status, text) = t.exchange(method, &format!("{}{path}", self.base), &headers, body)?;
        if !(200..300).contains(&status) {
            let why = serde_json::from_str::<Value>(&text).ok().and_then(|v| {
                v["error"]["message"]
                    .as_str()
                    .map(String::from)
                    .or_else(|| v["error"]["code"].as_str().map(String::from))
            });
            return Err(match why {
                Some(w) => format!("Hetzner answered {status}: {w}"),
                None => format!("Hetzner answered {status}"),
            });
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|e| format!("Hetzner's answer did not read: {e}"))
    }
}

impl Provider for Hetzner {
    fn name(&self) -> &'static str {
        "Hetzner"
    }

    fn asn(&self) -> u64 {
        // HETZNER-AS, Hetzner Online GmbH (RIPE, read 2026-10-26)
        24940
    }

    fn token_page(&self) -> &'static str {
        // a token belongs to a project, and the project is chosen on the
        // way: the console's own front door is where the steps start
        "https://console.hetzner.cloud/projects"
    }

    fn token_walkthrough(&self) -> &'static [&'static str] {
        HETZNER_TOKEN_STEPS
    }

    fn machine_images(
        &self,
        t: &dyn Transport,
        credential: &str,
    ) -> Result<Vec<MachineImage>, String> {
        // the distributions, for this flow's architecture, and the page
        // Hetzner's largest: `per_page` tops out at 50 and the system
        // images are a short list
        let v = self.ask(
            t,
            credential,
            "GET",
            "/v1/images?type=system&architecture=x86&status=available&per_page=50",
            None,
        )?;
        Ok(v["images"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|i| {
                        Some(MachineImage {
                            slug: i["name"].as_str()?.to_string(),
                            distribution: i["os_flavor"].as_str().unwrap_or("").to_string(),
                            name: i["description"].as_str().unwrap_or("").to_string(),
                            amd64: i["architecture"].as_str() == Some("x86"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn regions(&self, t: &dyn Transport, credential: &str) -> Result<Vec<Region>, String> {
        let v = self.ask(t, credential, "GET", "/v1/locations", None)?;
        Ok(v["locations"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|l| {
                        let city = l["city"].as_str().unwrap_or("");
                        let country = l["country"].as_str().unwrap_or("");
                        Some(Region {
                            slug: l["name"].as_str()?.to_string(),
                            name: match l["description"].as_str() {
                                Some(d) if !d.is_empty() => format!("{d} ({country})"),
                                _ => format!("{city} ({country})"),
                            },
                            // **Hetzner states availability per server
                            // type, not per location** (each type lists
                            // the locations it is offered in), so a
                            // location is offered and the size listing is
                            // where a choice narrows
                            available: true,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn sizes(&self, t: &dyn Transport, credential: &str) -> Result<Vec<Size>, String> {
        let v = self.ask(t, credential, "GET", "/v1/server_types?per_page=50", None)?;
        Ok(v["server_types"]
            .as_array()
            .map(|a| {
                a.iter()
                    // an architecture this flow's image does not run on,
                    // and a type being withdrawn, are both listed: a
                    // chooser offering either offers a launch that fails
                    .filter(|s| s["architecture"].as_str() == Some("x86"))
                    .filter(|s| {
                        s["deprecated"].as_bool() != Some(true) && !s["deprecation"].is_object()
                    })
                    .filter_map(|s| {
                        // where it is offered now, from the type's own
                        // list; the prices' locations where it carries
                        // none
                        let regions: Vec<String> = s["locations"]
                            .as_array()
                            .map(|ls| {
                                ls.iter()
                                    .filter(|l| l["available"].as_bool() != Some(false))
                                    .filter_map(|l| l["name"].as_str().map(String::from))
                                    .collect()
                            })
                            .filter(|r: &Vec<String>| !r.is_empty())
                            .or_else(|| {
                                s["prices"]
                                    .as_array()
                                    .map(|ps| ps.iter().filter_map(l_name).collect())
                            })
                            .unwrap_or_default();
                        // the net monthly price, as a string of euros, at
                        // the cheapest location it is offered in
                        let monthly = s["prices"]
                            .as_array()
                            .map(|ps| {
                                ps.iter()
                                    .filter_map(|p| p["price_monthly"]["net"].as_str())
                                    .filter_map(|n| n.parse::<f64>().ok())
                                    .fold(f64::INFINITY, f64::min)
                            })
                            .filter(|m| m.is_finite())
                            .unwrap_or(0.0);
                        Some(Size {
                            slug: s["name"].as_str()?.to_string(),
                            // gigabytes, and a float: 0.5 is a size
                            // Hetzner has sold
                            memory_mb: (s["memory"].as_f64().unwrap_or(0.0) * 1024.0).round()
                                as u64,
                            vcpus: s["cores"].as_f64().unwrap_or(0.0).round() as u64,
                            monthly,
                            regions,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn launch(
        &self,
        t: &dyn Transport,
        credential: &str,
        plan: &Plan,
        name: &str,
        user_data: &str,
    ) -> Result<Launched, String> {
        let image = match &plan.machine_image {
            Some(i) => i.clone(),
            None => newest_debian(&self.machine_images(t, credential)?).ok_or_else(|| {
                "Hetzner lists no Debian image to this token: a token with Read alone cannot read \
                 them"
                    .to_string()
            })?,
        };
        let body = json!({
            "name": name,
            "location": plan.region,
            "server_type": plan.size,
            "image": image,
            "start_after_create": true,
            "user_data": user_data,
            "public_net": { "enable_ipv4": true, "enable_ipv6": false },
            "labels": { "rhtn": "" },
        });
        let v = self.ask(
            t,
            credential,
            "POST",
            "/v1/servers",
            Some(&body.to_string()),
        )?;
        let id = v["server"]["id"]
            .as_u64()
            .ok_or("the answer named no server id")?;
        Ok(Launched { id: id.to_string() })
    }

    fn standing(&self, t: &dyn Transport, credential: &str, id: &str) -> Result<Standing, String> {
        let v = self.ask(t, credential, "GET", &format!("/v1/servers/{id}"), None)?;
        let s = &v["server"];
        let status = s["status"].as_str().unwrap_or("unknown").to_string();
        Ok(Standing {
            active: status == "running",
            ipv4: s["public_net"]["ipv4"]["ip"]
                .as_str()
                .filter(|ip| !ip.is_empty())
                .map(String::from),
            status,
        })
    }

    fn destroy(&self, t: &dyn Transport, credential: &str, id: &str) -> Result<(), String> {
        self.ask(t, credential, "DELETE", &format!("/v1/servers/{id}"), None)?;
        Ok(())
    }
}

/// A price row's location, where it names one.
fn l_name(p: &Value) -> Option<String> {
    p["location"].as_str().map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A transport that answers from a script and keeps what it was asked.
    struct Scripted {
        answers: RefCell<Vec<(u16, String)>>,
        asked: RefCell<Vec<(String, String, String)>>,
    }

    impl Scripted {
        fn new(answers: Vec<(u16, &str)>) -> Scripted {
            Scripted {
                answers: RefCell::new(
                    answers
                        .into_iter()
                        .map(|(s, b)| (s, b.to_string()))
                        .rev()
                        .collect(),
                ),
                asked: RefCell::new(Vec::new()),
            }
        }
    }

    impl Transport for Scripted {
        fn exchange(
            &self,
            method: &str,
            url: &str,
            headers: &[(&str, &str)],
            body: Option<&str>,
        ) -> Result<(u16, String), String> {
            let auth = headers
                .iter()
                .find(|(k, _)| *k == "Authorization")
                .map(|(_, v)| v.to_string())
                .unwrap_or_default();
            self.asked.borrow_mut().push((
                format!("{method} {url}"),
                auth,
                body.unwrap_or("").to_string(),
            ));
            self.answers
                .borrow_mut()
                .pop()
                .ok_or("no answer scripted".to_string())
        }
    }

    // acceptance: PRT-08
    #[test]
    fn the_configuration_and_the_script_carry_what_the_instance_needs_and_no_seed() {
        let token = "ab".repeat(32);
        let conf = node_config(&token, QUIC_PORT, ADMIN_PORT);
        assert!(conf.contains(&format!("token = \"{token}\"")));
        assert!(conf.contains("listen = \"0.0.0.0:7431\""));
        assert!(conf.contains("listen = \"0.0.0.0:7449\""));
        let image = Image {
            registry: "reg.example/x".into(),
            tag: "abc123".into(),
        };
        let script = user_data("82a301", &token, &image, QUIC_PORT, ADMIN_PORT);
        assert!(script.starts_with("#cloud-config\n"));
        assert!(script.contains("  - podman\n"));
        assert!(script.contains(&format!("      token = \"{token}\"")));
        assert!(script.contains("      82a301\n"));
        assert!(script.contains("chown -R 10001:10001 /var/lib/rhtn"));
        assert!(script.contains("-v /var/lib/rhtn:/var/lib/rhtn reg.example/x/rhtnd:abc123\n"));
        assert!(!script.contains("seed"));
    }

    // acceptance: PRT-08
    #[test]
    fn a_launch_posts_the_plan_and_reads_the_id_and_the_address_once_active() {
        let t = Scripted::new(vec![
            (
                200,
                r#"{"images":[{"slug":"ubuntu-24-04-x64","distribution":"Ubuntu","name":"24.04 LTS x64"},{"slug":"debian-12-x64","distribution":"Debian","name":"12 x64"},{"slug":"debian-13-x64","distribution":"Debian","name":"13 x64"},{"slug":"debian-13-arm64","distribution":"Debian","name":"13 arm64"}]}"#,
            ),
            (
                202,
                r#"{"droplet":{"id":123456,"name":"rhtn-1","status":"new"}}"#,
            ),
            (
                200,
                r#"{"droplet":{"id":123456,"status":"active","networks":{"v4":[{"ip_address":"10.0.0.2","type":"private"},{"ip_address":"203.0.113.9","type":"public"}]}}}"#,
            ),
        ]);
        let d = DigitalOcean {
            base: "http://api.test".into(),
        };
        let got = d
            .launch(
                &t,
                "tok",
                &Plan::new("nyc3", "s-1vcpu-1gb"),
                "rhtn-1",
                "#cloud-config\n",
            )
            .unwrap();
        assert_eq!(got.id, "123456");
        let s = d.standing(&t, "tok", "123456").unwrap();
        assert!(s.active);
        assert_eq!(s.ipv4.as_deref(), Some("203.0.113.9"));
        let asked = t.asked.borrow();
        // the machine image is the provider's newest Debian, asked for
        // rather than named
        assert_eq!(
            asked[0].0,
            "GET http://api.test/v2/images?type=distribution&per_page=200"
        );
        assert_eq!(asked[1].0, "POST http://api.test/v2/droplets");
        assert_eq!(asked[1].1, "Bearer tok");
        let sent: Value = serde_json::from_str(&asked[1].2).unwrap();
        assert_eq!(sent["region"], "nyc3");
        assert_eq!(sent["size"], "s-1vcpu-1gb");
        assert_eq!(sent["image"], "debian-13-x64");
        assert_eq!(sent["user_data"], "#cloud-config\n");
        assert_eq!(asked[2].0, "GET http://api.test/v2/droplets/123456");
    }

    #[test]
    fn a_named_machine_image_is_sent_as_named_and_no_debian_is_a_refusal_with_the_scope_said() {
        let t = Scripted::new(vec![(
            202,
            r#"{"droplet":{"id":7,"name":"rhtn-1","status":"new"}}"#,
        )]);
        let d = DigitalOcean {
            base: "http://api.test".into(),
        };
        let mut plan = Plan::new("nyc3", "s-1vcpu-1gb");
        plan.machine_image = Some("debian-12-x64".into());
        d.launch(&t, "tok", &plan, "rhtn-1", "#cloud-config\n")
            .unwrap();
        {
            let asked = t.asked.borrow();
            assert_eq!(asked[0].0, "POST http://api.test/v2/droplets");
            let sent: Value = serde_json::from_str(&asked[0].2).unwrap();
            assert_eq!(sent["image"], "debian-12-x64");
        }

        let t = Scripted::new(vec![(
            200,
            r#"{"images":[{"slug":"ubuntu-24-04-x64","distribution":"Ubuntu","name":"24.04"}]}"#,
        )]);
        let e = d
            .launch(
                &t,
                "tok",
                &Plan::new("nyc3", "s-1vcpu-1gb"),
                "rhtn-1",
                "#cloud-config\n",
            )
            .unwrap_err();
        assert!(e.contains("image: read"), "{e}");
        assert_eq!(newest_debian(&[]), None);
    }

    // acceptance: PRT-08
    /// **Hetzner, in its own dialect** (`Hetzner`): gigabytes of memory,
    /// a price as a string of euros per location, an architecture as a
    /// field, and a server type listed where it is no longer offered.
    #[test]
    fn hetzners_regions_and_sizes_parse_out_of_its_own_spellings() {
        let t = Scripted::new(vec![
            (
                200,
                r#"{"locations":[{"id":1,"name":"fsn1","description":"Falkenstein DC Park 1","country":"DE","city":"Falkenstein","network_zone":"eu-central"},{"id":4,"name":"ash","description":"","country":"US","city":"Ashburn, VA","network_zone":"us-east"}]}"#,
            ),
            (
                200,
                r#"{"server_types":[
                    {"name":"cx22","cores":2,"memory":4.0,"architecture":"x86","deprecated":false,
                     "prices":[{"location":"fsn1","price_monthly":{"net":"4.5900000000","gross":"5.46"}},
                               {"location":"ash","price_monthly":{"net":"5.9900000000","gross":"5.99"}}],
                     "locations":[{"name":"fsn1","available":true},{"name":"ash","available":false}]},
                    {"name":"cax11","cores":2,"memory":4.0,"architecture":"arm","deprecated":false,
                     "prices":[{"location":"fsn1","price_monthly":{"net":"3.7900000000"}}],
                     "locations":[{"name":"fsn1","available":true}]},
                    {"name":"cx11","cores":1,"memory":2.0,"architecture":"x86","deprecated":false,
                     "deprecation":{"unavailable_after":"2026-01-01T00:00:00+00:00","announced":"2025-07-01T00:00:00+00:00"},
                     "prices":[{"location":"fsn1","price_monthly":{"net":"2.8900000000"}}],
                     "locations":[{"name":"fsn1","available":true}]},
                    {"name":"cpx11","cores":2,"memory":0.5,"architecture":"x86",
                     "prices":[{"location":"fsn1","price_monthly":{"net":"3.4900000000"}}]}
                ]}"#,
            ),
        ]);
        let h = Hetzner {
            base: "http://api.test".into(),
        };
        let regions = h.regions(&t, "tok").unwrap();
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].slug, "fsn1");
        assert_eq!(regions[0].name, "Falkenstein DC Park 1 (DE)");
        assert_eq!(
            regions[1].name, "Ashburn, VA (US)",
            "a location with no description is named by its city"
        );
        assert!(
            regions.iter().all(|r| r.available),
            "availability is a server type's, per location"
        );

        let sizes = h.sizes(&t, "tok").unwrap();
        let slugs: Vec<&str> = sizes.iter().map(|s| s.slug.as_str()).collect();
        assert_eq!(
            slugs,
            vec!["cx22", "cpx11"],
            "the arm type and the one being withdrawn are not offered"
        );
        assert_eq!(sizes[0].memory_mb, 4096, "gigabytes, stated as such");
        assert_eq!(sizes[0].vcpus, 2);
        assert_eq!(
            sizes[0].monthly, 4.59,
            "the cheapest location's net price, read from its string"
        );
        assert_eq!(
            sizes[0].regions,
            vec!["fsn1".to_string()],
            "where it is offered now, not where it is priced"
        );
        assert_eq!(sizes[1].memory_mb, 512, "half a gigabyte is a size");
        assert_eq!(
            sizes[1].regions,
            vec!["fsn1".to_string()],
            "a type carrying no locations falls back to its prices'"
        );
        let asked = t.asked.borrow();
        assert_eq!(asked[0].1, "Bearer tok");
        assert!(asked[1].0.contains("/v1/server_types"), "{:?}", asked[1].0);
    }

    // acceptance: PRT-08
    #[test]
    fn a_hetzner_launch_posts_its_own_spelling_and_reads_the_address_once_running() {
        let t = Scripted::new(vec![
            (
                200,
                r#"{"images":[{"name":"debian-12","description":"Debian 12","os_flavor":"debian","os_version":"12","architecture":"x86","status":"available"},{"name":"debian-13","description":"Debian 13","os_flavor":"debian","os_version":"13","architecture":"x86","status":"available"},{"name":"ubuntu-24.04","description":"Ubuntu 24.04","os_flavor":"ubuntu","architecture":"x86","status":"available"}]}"#,
            ),
            (
                201,
                r#"{"server":{"id":987654,"status":"initializing","public_net":{"ipv4":null}},"action":{"id":1,"status":"running"}}"#,
            ),
            (
                200,
                r#"{"server":{"id":987654,"status":"running","public_net":{"ipv4":{"id":2,"ip":"203.0.113.40","blocked":false}}}}"#,
            ),
            (
                200,
                r#"{"action":{"id":3,"command":"delete_server","status":"running"}}"#,
            ),
        ]);
        let h = Hetzner {
            base: "http://api.test".into(),
        };
        let got = h
            .launch(
                &t,
                "tok",
                &Plan::new("fsn1", "cx22"),
                "rhtn-1",
                "#cloud-config\n",
            )
            .unwrap();
        assert_eq!(got.id, "987654");
        let s = h.standing(&t, "tok", "987654").unwrap();
        assert!(s.active, "`running` is what this flow waits for");
        assert_eq!(s.ipv4.as_deref(), Some("203.0.113.40"));
        h.destroy(&t, "tok", "987654").unwrap();

        let asked = t.asked.borrow();
        assert!(
            asked[0]
                .0
                .contains("/v1/images?type=system&architecture=x86"),
            "the images are asked for this flow's architecture: {:?}",
            asked[0].0
        );
        assert_eq!(asked[1].0, "POST http://api.test/v1/servers");
        let sent: Value = serde_json::from_str(&asked[1].2).unwrap();
        assert_eq!(sent["location"], "fsn1", "a location, not a region");
        assert_eq!(sent["server_type"], "cx22", "a server type, not a size");
        assert_eq!(
            sent["image"], "debian-13",
            "the newest Debian the provider offers, in the provider's own spelling"
        );
        assert_eq!(sent["start_after_create"], true);
        assert_eq!(sent["public_net"]["enable_ipv4"], true);
        assert_eq!(sent["public_net"]["enable_ipv6"], false);
        assert_eq!(sent["user_data"], "#cloud-config\n");
        assert_eq!(asked[2].0, "GET http://api.test/v1/servers/987654");
        assert_eq!(asked[3].0, "DELETE http://api.test/v1/servers/987654");
    }

    #[test]
    fn a_hetzner_refusal_carries_its_own_words() {
        let t = Scripted::new(vec![(
            401,
            r#"{"error":{"code":"unauthorized","message":"unable to authenticate"}}"#,
        )]);
        let h = Hetzner {
            base: "http://api.test".into(),
        };
        let e = h.regions(&t, "nope").unwrap_err();
        assert_eq!(e, "Hetzner answered 401: unable to authenticate");
    }

    #[test]
    fn the_newest_debian_is_found_in_either_providers_spelling_and_never_for_another_machine() {
        let image = |slug: &str, dist: &str, amd64: bool| MachineImage {
            slug: slug.into(),
            distribution: dist.into(),
            name: slug.into(),
            amd64,
        };
        // DigitalOcean's spelling, with an arm image beside it
        let d = vec![
            image("debian-12-x64", "Debian", true),
            image("debian-13-x64", "Debian", true),
            image("debian-14-arm64", "Debian", false),
            image("ubuntu-24-04-x64", "Ubuntu", true),
        ];
        assert_eq!(newest_debian(&d).as_deref(), Some("debian-13-x64"));
        // Hetzner's
        let h = vec![
            image("debian-12", "debian", true),
            image("debian-13", "debian", true),
            image("debian-13", "debian", false),
            image("ubuntu-24.04", "ubuntu", true),
        ];
        assert_eq!(newest_debian(&h).as_deref(), Some("debian-13"));
        assert_eq!(newest_debian(&[]), None);
        assert_eq!(
            newest_debian(&[image("debian-13", "debian", false)]),
            None,
            "an image this flow's machines cannot run is not a candidate"
        );
    }

    // acceptance: PRT-08
    #[test]
    fn the_token_page_is_the_providers_own_and_the_steps_name_the_scopes_the_launch_needs() {
        let p = provider("digitalocean").unwrap();
        assert!(
            p.token_page()
                .starts_with("https://cloud.digitalocean.com/")
        );
        let steps = p.token_walkthrough().join(" ");
        for scope in [
            "droplet: create, read and delete",
            "image: read",
            "regions: read",
            "sizes: read",
        ] {
            assert!(steps.contains(scope), "{scope}");
        }
        assert!(steps.contains("paste"));
    }

    #[test]
    fn regions_and_sizes_parse_and_a_refusal_carries_the_providers_words() {
        let t = Scripted::new(vec![
            (
                200,
                r#"{"regions":[{"slug":"nyc3","name":"New York 3","available":true},{"slug":"x","name":"gone","available":false}]}"#,
            ),
            (
                200,
                r#"{"sizes":[{"slug":"s-1vcpu-1gb","memory":1024,"vcpus":1,"price_monthly":6.0,"regions":["nyc3"],"available":true},{"slug":"old","available":false}]}"#,
            ),
            (
                401,
                r#"{"id":"unauthorized","message":"Unable to authenticate you"}"#,
            ),
        ]);
        let d = DigitalOcean::default();
        let regions = d.regions(&t, "tok").unwrap();
        assert_eq!(
            regions.iter().map(|r| r.slug.as_str()).collect::<Vec<_>>(),
            ["nyc3", "x"]
        );
        let sizes = d.sizes(&t, "tok").unwrap();
        assert_eq!(sizes.len(), 1);
        assert_eq!(sizes[0].memory_mb, 1024);
        let e = d.regions(&t, "bad").unwrap_err();
        assert!(
            e.contains("401") && e.contains("Unable to authenticate you"),
            "{e}"
        );
    }
}
