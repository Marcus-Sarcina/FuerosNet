//! The wake endpoints this node holds for the clients it serves (design
//! §14.1.5; `infra-client-requirements.md` §6.1): at most one per
//! relationship, posted to with a body that says only that there is
//! something to come back for, forgotten when the client withdraws it or
//! the relationship ends, and never logged.
//!
//! What is held is a URL and a key, both the client's to choose.  This
//! node keeps no vendor credential and learns nothing of the service
//! behind the URL beyond its host.

use crate::Keyhash;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// One client's endpoint, as that client gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub url: String,
    /// What the posted body is encrypted to.  Opaque here: this node
    /// encrypts to it and reads nothing.
    pub key: Vec<u8>,
    /// When the client expects the endpoint to stop working, if it said.
    /// A lapsed endpoint is not posted to and is not evidence of anything
    /// about the client.
    pub lapses_at: Option<u64>,
}

impl Endpoint {
    pub fn lapsed(&self, now: u64) -> bool {
        self.lapses_at.is_some_and(|t| now >= t)
    }
}

/// What a registration did, which is what the reply's code carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Registered {
    Held,
    Withdrawn,
    /// The URL or the key is longer than this node accepts, or a
    /// registration named a key without an endpoint to attach it to.
    Refused,
}

/// The endpoints, by client and device (`wire-format.md` §8.2: a
/// registration is the session's device's).  A device holds one;
/// registering again replaces it, which is how a client refreshes.
#[derive(Debug, Default)]
pub struct WakeRegister {
    endpoints: BTreeMap<(Keyhash, [u8; 32]), Endpoint>,
    /// Where they are kept, once an owner has said.  Absent, they live in
    /// memory alone.
    dir: Option<PathBuf>,
    /// The longest URL and key this node will hold.  The wire bounds both
    /// (`wire-format.md` §7.10); an operator may hold less.
    pub max_url: usize,
    pub max_key: usize,
}

impl WakeRegister {
    pub fn new() -> Self {
        WakeRegister {
            max_url: 2048,
            max_key: 256,
            ..Default::default()
        }
    }

    /// Keep the register under `dir`, and read back whatever is there.
    pub fn at(&mut self, dir: impl Into<PathBuf>) {
        let dir = dir.into();
        let d = dir.join("wake");
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let Some((who, dev)) = name.split_once('-') else {
                    continue;
                };
                let (Some(who), Some(dev)) = (unhex(who), unhex(dev)) else {
                    continue;
                };
                let Ok(text) = std::fs::read_to_string(e.path()) else {
                    continue;
                };
                if let Some(ep) = parse(&text) {
                    self.endpoints.insert((who, dev), ep);
                }
            }
        }
        self.dir = Some(dir);
    }

    /// One of the client's endpoints, whichever device.
    pub fn get(&self, client: &Keyhash) -> Option<&Endpoint> {
        self.of_client(client).next().map(|(_, e)| e)
    }

    pub fn get_for(&self, client: &Keyhash, device: &[u8; 32]) -> Option<&Endpoint> {
        self.endpoints.get(&(*client, *device))
    }

    fn of_client<'a>(
        &'a self,
        client: &Keyhash,
    ) -> impl Iterator<Item = ([u8; 32], &'a Endpoint)> + 'a {
        let client = *client;
        self.endpoints
            .range((client, [0; 32])..=(client, [0xff; 32]))
            .map(|((_, d), e)| (*d, e))
    }

    /// Every client with an endpoint, once each.
    pub fn holders(&self) -> Vec<Keyhash> {
        let mut out: Vec<Keyhash> = self.endpoints.keys().map(|(c, _)| *c).collect();
        out.dedup();
        out
    }

    /// Take what `client` registered.  No URL withdraws: opting out is as
    /// sayable as opting in (design §14.1.5).
    pub fn register(
        &mut self,
        client: Keyhash,
        device: [u8; 32],
        url: Option<String>,
        key: Option<Vec<u8>>,
        lapses_at: Option<u64>,
    ) -> Registered {
        let Some(url) = url else {
            self.forget_device(&client, &device);
            return Registered::Withdrawn;
        };
        let key = key.unwrap_or_default();
        if url.is_empty() || url.len() > self.max_url || key.is_empty() || key.len() > self.max_key
        {
            return Registered::Refused;
        }
        let ep = Endpoint {
            url,
            key,
            lapses_at,
        };
        if let Some(dir) = &self.dir {
            let d = dir.join("wake");
            if std::fs::create_dir_all(&d)
                .and_then(|_| std::fs::write(d.join(file_name(&client, &device)), render(&ep)))
                .is_err()
            {
                return Registered::Refused;
            }
        }
        self.endpoints.insert((client, device), ep);
        Registered::Held
    }

    /// Forget one device's endpoint.
    pub fn forget_device(&mut self, client: &Keyhash, device: &[u8; 32]) {
        self.endpoints.remove(&(*client, *device));
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_file(dir.join("wake").join(file_name(client, device)));
        }
    }

    /// Forget one endpoint: on withdrawal, and when the relationship that
    /// justified holding it ends (`infra-client-requirements.md` §6.1).
    pub fn forget(&mut self, client: &Keyhash) {
        let devices: Vec<[u8; 32]> = self.of_client(client).map(|(d, _)| d).collect();
        for d in devices {
            self.forget_device(client, &d);
        }
    }

    /// Where to ring `client`, if anywhere: nothing for a client that
    /// registered none and nothing for an endpoint the client said would
    /// have lapsed by `now`.
    pub fn doorbell(&self, client: &Keyhash, now: u64) -> Option<&Endpoint> {
        self.of_client(client)
            .map(|(_, e)| e)
            .find(|e| !e.lapsed(now))
    }

    /// Where to ring one of the client's devices.
    pub fn doorbell_for(&self, client: &Keyhash, device: &[u8; 32], now: u64) -> Option<&Endpoint> {
        self.endpoints
            .get(&(*client, *device))
            .filter(|e| !e.lapsed(now))
    }
}

/// One endpoint as a file: two or three lines, the key hex.  A format a
/// person can read, since an operator may have to answer for what is held.
fn render(e: &Endpoint) -> String {
    let mut s = format!("url = {}\nkey = {}\n", e.url, hex_bytes(&e.key));
    if let Some(t) = e.lapses_at {
        s.push_str(&format!("lapses = {t}\n"));
    }
    s
}

fn parse(text: &str) -> Option<Endpoint> {
    let (mut url, mut key, mut lapses) = (None, None, None);
    for line in text.lines() {
        let Some((k, v)) = line.split_once(" = ") else {
            continue;
        };
        match k {
            "url" => url = Some(v.to_string()),
            "key" => key = unhex_bytes(v),
            "lapses" => lapses = v.parse().ok(),
            _ => {}
        }
    }
    Some(Endpoint {
        url: url?,
        key: key?,
        lapses_at: lapses,
    })
}

fn hex(k: &Keyhash) -> String {
    hex_bytes(k)
}

fn file_name(client: &Keyhash, device: &[u8; 32]) -> String {
    format!("{}-{}", hex(client), hex(device))
}

fn hex_bytes(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

fn unhex(s: &str) -> Option<Keyhash> {
    unhex_bytes(s)?.try_into().ok()
}
