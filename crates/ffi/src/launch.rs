//! **The launch across the boundary** (`light-client-requirements.md` §10;
//! `rhtn-launch`): the composition and the provider adapters are the
//! crate's, the HTTP exchange is the shell's — a phone has the system's
//! trust roots and its own connection class, and the kernel ships no
//! bundle by design — so the shell hands in a [`ProviderTransport`] and
//! gets back values to draw.  No credential is held here: the shell reads
//! and writes the kernel's provider credentials through
//! [`crate::client::Participant::provider_credentials`].

use crate::types::Refused;
use std::sync::Arc;

/// One HTTP exchange's outcome, as the shell made it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Exchange {
    /// The status, or 0 where the request did not complete.
    pub status: u16,
    /// The body's text, or the failure's words where `status` is 0.
    pub body: String,
}

/// One header to send.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Header {
    /// The header's name.
    pub name: String,
    /// Its value.
    pub value: String,
}

/// The shell's HTTP client, as the crate drives it.
#[uniffi::export(with_foreign)]
pub trait ProviderTransport: Send + Sync {
    /// Make the request and answer with its status and body; a request
    /// that did not complete answers status 0 and the failure's words.
    fn exchange(
        &self,
        method: String,
        url: String,
        headers: Vec<Header>,
        body: Option<String>,
    ) -> Exchange;
}

struct Over(Arc<dyn ProviderTransport>);

impl rhtn_launch::Transport for Over {
    fn exchange(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> Result<(u16, String), String> {
        let hs = headers
            .iter()
            .map(|(n, v)| Header {
                name: (*n).to_string(),
                value: (*v).to_string(),
            })
            .collect();
        let e = self.0.exchange(
            method.to_string(),
            url.to_string(),
            hs,
            body.map(String::from),
        );
        if e.status == 0 {
            return Err(e.body);
        }
        Ok((e.status, e.body))
    }
}

/// A region the provider offers.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LaunchRegion {
    /// Its slug, as a plan names it.
    pub slug: String,
    /// Its name, for a person.
    pub name: String,
    /// Whether it takes new machines now.
    pub available: bool,
}

/// A size the provider offers.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LaunchSize {
    /// Its slug, as a plan names it.
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

/// Where a machine stands.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LaunchStanding {
    /// The provider's status word.
    pub status: String,
    /// The public IPv4, once assigned.
    pub ipv4: Option<String>,
    /// Whether the provider says it is running.
    pub active: bool,
}

/// The providers this build drives, by the names credentials are filed
/// under.
#[uniffi::export]
pub fn launch_providers() -> Vec<String> {
    rhtn_launch::provider_names()
        .into_iter()
        .map(String::from)
        .collect()
}

/// A provider's display name and autonomous system, or none for a name
/// this build does not drive.
#[uniffi::export]
pub fn launch_provider_asn(provider: String) -> Option<u64> {
    rhtn_launch::provider(&provider).map(|p| p.asn())
}

/// Where an operator makes a provider's token: its own page, for the
/// shell to open; none for a name this build does not drive.
#[uniffi::export]
pub fn launch_token_page(provider: String) -> Option<String> {
    rhtn_launch::provider(&provider).map(|p| p.token_page().to_string())
}

/// The steps on that page, for the shell to show beside it.
#[uniffi::export]
pub fn launch_token_walkthrough(provider: String) -> Vec<String> {
    rhtn_launch::provider(&provider)
        .map(|p| {
            p.token_walkthrough()
                .iter()
                .map(|s| (*s).to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// The registry pulled from unless the operator names another.
#[uniffi::export]
pub fn launch_registry_default() -> String {
    rhtn_launch::REGISTRY_DEFAULT.to_string()
}

/// The once-run script for a machine: the operator's public material as
/// hex, the token as 64 hex digits, the registry and the tag.
#[uniffi::export]
pub fn launch_user_data(
    material_hex: String,
    token_hex: String,
    registry: String,
    tag: String,
) -> String {
    let image = rhtn_launch::Image { registry, tag };
    rhtn_launch::user_data(
        &material_hex,
        &token_hex,
        &image,
        rhtn_launch::QUIC_PORT,
        rhtn_launch::ADMIN_PORT,
    )
}

fn provider_named(provider: &str) -> Result<Box<dyn rhtn_launch::Provider>, Refused> {
    rhtn_launch::provider(provider)
        .ok_or_else(|| Refused::new(format!("{provider} is not a provider this build drives")))
}

/// The regions a provider offers.
#[uniffi::export]
pub fn launch_regions(
    provider: String,
    transport: Arc<dyn ProviderTransport>,
    credential: String,
) -> Result<Vec<LaunchRegion>, Refused> {
    let p = provider_named(&provider)?;
    let rs = p
        .regions(&Over(transport), &credential)
        .map_err(Refused::new)?;
    Ok(rs
        .into_iter()
        .map(|r| LaunchRegion {
            slug: r.slug,
            name: r.name,
            available: r.available,
        })
        .collect())
}

/// The sizes a provider offers.
#[uniffi::export]
pub fn launch_sizes(
    provider: String,
    transport: Arc<dyn ProviderTransport>,
    credential: String,
) -> Result<Vec<LaunchSize>, Refused> {
    let p = provider_named(&provider)?;
    let ss = p
        .sizes(&Over(transport), &credential)
        .map_err(Refused::new)?;
    Ok(ss
        .into_iter()
        .map(|s| LaunchSize {
            slug: s.slug,
            memory_mb: s.memory_mb,
            vcpus: s.vcpus,
            monthly: s.monthly,
            regions: s.regions,
        })
        .collect())
}

/// Create the machine: the provider's id for it comes back.
#[uniffi::export]
#[allow(clippy::too_many_arguments)]
pub fn launch_create(
    provider: String,
    transport: Arc<dyn ProviderTransport>,
    credential: String,
    region: String,
    size: String,
    registry: String,
    tag: String,
    name: String,
    user_data: String,
) -> Result<String, Refused> {
    let p = provider_named(&provider)?;
    let mut plan = rhtn_launch::Plan::new(&region, &size);
    plan.image = rhtn_launch::Image { registry, tag };
    p.launch(&Over(transport), &credential, &plan, &name, &user_data)
        .map(|l| l.id)
        .map_err(Refused::new)
}

/// Where a machine stands now.
#[uniffi::export]
pub fn launch_standing(
    provider: String,
    transport: Arc<dyn ProviderTransport>,
    credential: String,
    id: String,
) -> Result<LaunchStanding, Refused> {
    let p = provider_named(&provider)?;
    p.standing(&Over(transport), &credential, &id)
        .map(|s| LaunchStanding {
            status: s.status,
            ipv4: s.ipv4,
            active: s.active,
        })
        .map_err(Refused::new)
}

/// Destroy a machine at the provider.
#[uniffi::export]
pub fn launch_destroy(
    provider: String,
    transport: Arc<dyn ProviderTransport>,
    credential: String,
    id: String,
) -> Result<(), Refused> {
    let p = provider_named(&provider)?;
    p.destroy(&Over(transport), &credential, &id)
        .map_err(Refused::new)
}
