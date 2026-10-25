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
//! providers copy, so it is the first [`Provider`] and the pattern for
//! Hetzner, Vultr and BitLaunch; Njalla speaks it in another dialect; AWS
//! and GCP speak the same primitive under heavier signing.
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

#![warn(missing_docs)]

pub mod oauth;

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
/// The provider's machine image: Debian 12, which carries podman.
pub const MACHINE_IMAGE_DEFAULT: &str = "debian-12-x64";

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
         token = \"{token_hex}\"\n"
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
    /// The provider's machine image.
    pub machine_image: String,
    /// The node image to run.
    pub image: Image,
}

impl Plan {
    /// A plan at the default images.
    pub fn new(region: &str, size: &str) -> Plan {
        Plan {
            region: region.into(),
            size: size.into(),
            machine_image: MACHINE_IMAGE_DEFAULT.into(),
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
        _ => None,
    }
}

/// Every provider's name, in the order a chooser lists them before
/// PRD-11's ordering is applied.
pub fn provider_names() -> Vec<&'static str> {
    vec!["digitalocean"]
}

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
        let body = json!({
            "name": name,
            "region": plan.region,
            "size": plan.size,
            "image": plan.machine_image,
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
        assert_eq!(asked[0].0, "POST http://api.test/v2/droplets");
        assert_eq!(asked[0].1, "Bearer tok");
        let sent: Value = serde_json::from_str(&asked[0].2).unwrap();
        assert_eq!(sent["region"], "nyc3");
        assert_eq!(sent["size"], "s-1vcpu-1gb");
        assert_eq!(sent["image"], "debian-12-x64");
        assert_eq!(sent["user_data"], "#cloud-config\n");
        assert_eq!(asked[1].0, "GET http://api.test/v2/droplets/123456");
    }

    // acceptance: PRT-08
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
