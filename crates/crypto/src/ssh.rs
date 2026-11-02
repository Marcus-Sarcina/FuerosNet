//! **The key that controls a node this identity launched**
//! (`light-client-requirements.md` §10; `infra-client-requirements.md`
//! §8.2).
//!
//! An operator who rents a machine needs a shell on it: installing a
//! package, reading a log the surface does not carry, taking the machine
//! apart again. Until now a launch installed no key at all, so the shell
//! was whatever the provider offered out of band — and nothing of it
//! survived the loss of the device that did the launching.
//!
//! **So the key is derived from the signing secret** [author,
//! 2026-10-31], under one label and nothing else: the phrase restores the
//! seeds, the seeds derive this, and **a node launched from a lost device
//! is controlled again from the phrase alone**. No new secret is minted,
//! nothing is kept that a backup has to carry, and a device that can
//! speak as this identity can open a shell on the machines it launched —
//! which is the same authority, not a wider one.
//!
//! **It is one key per identity, not per machine.** Every instance an
//! operator runs already names them in its endpoint record, so one
//! authorized key across their own machines links nothing that was not
//! already linked, and a per-machine key would want a label the recovery
//! path would have to carry.
//!
//! **What it is not**: it is not the identity's own key, cannot sign for
//! it, and is no part of the protocol. It is an ordinary Ed25519 ssh key
//! whose seed happens to be derived, so `ssh`, `scp` and any other tool
//! read it as what it is.

use crate::SigningIdentity;
use zeroize::Zeroizing;

/// The label the derivation expands under.  **Pinned to the byte**: a
/// device that derived it differently would be locked out of its own
/// machines.
pub const HOST_CONTROL_TAG: &[u8] = b"rhtn/1:host-control-ssh";

/// What the public line and the private file name themselves, so an
/// operator reading `authorized_keys` on a machine knows what the key is.
pub const COMMENT: &str = "rhtn-host-control";

/// **The second key's label**: the one the operator's client hands to its
/// own instance for one act, which the host's `authorized_keys` binds to
/// the package runner and nothing else.
pub const HOST_RUNNER_TAG: &[u8] = b"rhtn/1:host-runner-ssh";

/// What the runner key names itself.
pub const RUNNER_COMMENT: &str = "rhtn-host-runner";

/// **Where the host's own file binds the runner key** [author,
/// 2026-11-02]: the forced command, so a key handed to an instance can
/// run that program and nothing else.
pub const RUNNER_PROGRAM: &str = "/usr/local/bin/rhtn-package-runner";

/// The options the host's `authorized_keys` entry carries for the runner
/// key: the forced command, and every door ssh offers shut.
pub const RUNNER_OPTIONS: &str =
    "no-pty,no-port-forwarding,no-agent-forwarding,no-X11-forwarding,no-user-rc,restrict";

/// **The ssh key's seed, from the identity's classical seed.**  HKDF-SHA256
/// under [`HOST_CONTROL_TAG`], which is one way: the ssh key cannot be
/// walked back to the seed that made it.
pub fn host_control_seed(ed_seed: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0u8; 32]);
    crate::pqxdh::hkdf_sha256(&[], ed_seed, HOST_CONTROL_TAG, &mut out[..]);
    out
}

/// **The runner key's seed**, under its own label: a different key from
/// the same secret, so one may be given away for an act and the other
/// never is.
pub fn host_runner_seed(ed_seed: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0u8; 32]);
    crate::pqxdh::hkdf_sha256(&[], ed_seed, HOST_RUNNER_TAG, &mut out[..]);
    out
}

impl SigningIdentity {
    /// This identity's node-control ssh key, as the two files ssh reads:
    /// the `authorized_keys` line a launch installs, and the private key
    /// the operator keeps.
    ///
    /// **Derived at every call and never stored**: there is nothing to
    /// back up, and a device that holds the seed holds this.
    pub fn host_control_key(&self) -> HostControlKey {
        let seed = host_control_seed(&self.ed_signing_key().to_bytes());
        HostControlKey::from_seed(&seed)
    }

    /// **The key this identity hands its own instance for one act**
    /// (`infra-client-requirements.md` §9): a second key from the same
    /// secret, which the host's `authorized_keys` binds to
    /// [`RUNNER_PROGRAM`] alone.
    ///
    /// **Why a second one and not the first** [author, 2026-11-02]: the
    /// act is the instance asking its own host to start or stop a
    /// package, and the authority for it arrives with the request rather
    /// than sitting on the machine — so the instance holds a key only
    /// while it uses it. A node already seized would keep what it was
    /// handed, so what it is handed must be worth little: this key opens
    /// the runner's own vocabulary and no shell, and the operator's own
    /// key, which does open a shell, is never given to it.
    pub fn host_runner_key(&self) -> HostControlKey {
        let seed = host_runner_seed(&self.ed_signing_key().to_bytes());
        HostControlKey::from_seed_named(&seed, RUNNER_COMMENT)
    }
}

/// An Ed25519 ssh key in the two shapes ssh wants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostControlKey {
    /// The `authorized_keys` line: `ssh-ed25519 <base64> rhtn-host-control`.
    pub public_line: String,
    /// The private key, in OpenSSH's own format, unencrypted — the shape
    /// `ssh -i` takes.  **It is a secret**: whoever holds it has a shell
    /// on the machines this identity launched.
    pub private_key: String,
    /// The public half's 32 bytes, for anything that wants them raw.
    pub public_bytes: [u8; 32],
}

impl HostControlKey {
    /// The pair a seed expands to, named [`COMMENT`].
    pub fn from_seed(seed: &[u8; 32]) -> HostControlKey {
        HostControlKey::from_seed_named(seed, COMMENT)
    }

    /// The same under a comment of its own, which is how the two keys are
    /// told apart in a host's file and in a fingerprint listing.
    pub fn from_seed_named(seed: &[u8; 32], comment: &str) -> HostControlKey {
        let sk = ed25519_dalek::SigningKey::from_bytes(seed);
        let public = sk.verifying_key().to_bytes();
        HostControlKey {
            public_line: format!("{} {comment}", public_line_bare(&public)),
            private_key: private_key_named(seed, &public, comment),
            public_bytes: public,
        }
    }

    /// **The `authorized_keys` entry a host binds the runner key by**:
    /// the forced command and every door shut, then the key itself.
    /// Nothing but [`RUNNER_PROGRAM`] runs under it, whatever is asked.
    pub fn restricted_line(&self) -> String {
        format!(
            "command=\"{RUNNER_PROGRAM}\",{RUNNER_OPTIONS} {}",
            self.public_line
        )
    }
}

/// The `authorized_keys` line for an Ed25519 public half, named
/// [`COMMENT`].
pub fn public_line(public: &[u8; 32]) -> String {
    format!("{} {COMMENT}", public_line_bare(public))
}

/// The same without a comment: the type and the key.
pub fn public_line_bare(public: &[u8; 32]) -> String {
    let mut blob = Vec::new();
    string(&mut blob, b"ssh-ed25519");
    string(&mut blob, public);
    format!("ssh-ed25519 {}", base64(&blob))
}

/// **The private key as OpenSSH writes one** (the `openssh-key-v1`
/// container, no cipher and no kdf): the magic, the two names and the
/// empty options, one key, the public blob, and the private section with
/// its two equal check words, the key, the comment and the padding that
/// rounds it to eight bytes.
pub fn private_key(seed: &[u8; 32], public: &[u8; 32]) -> String {
    private_key_named(seed, public, COMMENT)
}

/// The same under a comment of its own.
pub fn private_key_named(seed: &[u8; 32], public: &[u8; 32], comment: &str) -> String {
    let mut pub_blob = Vec::new();
    string(&mut pub_blob, b"ssh-ed25519");
    string(&mut pub_blob, public);

    let mut private = Vec::new();
    // **the check words are equal**, which is how ssh tells a wrong
    // passphrase from a right one; with no cipher they are still written
    let check: u32 = u32::from_be_bytes([seed[0], seed[1], seed[2], seed[3]]);
    private.extend_from_slice(&check.to_be_bytes());
    private.extend_from_slice(&check.to_be_bytes());
    string(&mut private, b"ssh-ed25519");
    string(&mut private, public);
    let mut secret = Vec::with_capacity(64);
    secret.extend_from_slice(seed);
    secret.extend_from_slice(public);
    string(&mut private, &secret);
    string(&mut private, comment.as_bytes());
    // the padding is 1, 2, 3 … to the cipher's block size, which for no
    // cipher is eight
    let mut pad = 1u8;
    while !private.len().is_multiple_of(8) {
        private.push(pad);
        pad += 1;
    }

    let mut all = Vec::new();
    all.extend_from_slice(b"openssh-key-v1\0");
    string(&mut all, b"none");
    string(&mut all, b"none");
    string(&mut all, b"");
    all.extend_from_slice(&1u32.to_be_bytes());
    string(&mut all, &pub_blob);
    string(&mut all, &private);

    let body = base64(&all);
    let mut out = String::from("-----BEGIN OPENSSH PRIVATE KEY-----\n");
    for line in body.as_bytes().chunks(70) {
        out.push_str(std::str::from_utf8(line).expect("base64 is ascii"));
        out.push('\n');
    }
    out.push_str("-----END OPENSSH PRIVATE KEY-----\n");
    out
}

/// An ssh `string`: its length, then its bytes.
fn string(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 with padding, which is what both ssh files carry.
fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        let c = [(n >> 18) & 63, (n >> 12) & 63, (n >> 6) & 63, n & 63];
        out.push(ALPHABET[c[0] as usize] as char);
        out.push(ALPHABET[c[1] as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[c[2] as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[c[3] as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // acceptance: PRT-09
    #[test]
    fn the_key_is_the_seeds_and_comes_back_with_them() {
        let ed = [7u8; 32];
        let pq = [9u8; 32];
        let id = SigningIdentity::from_seeds(&ed, &pq);
        let key = id.host_control_key();

        // **the same identity derives the same key**, which is the whole
        // of the recovery: a device restored from the phrase holds these
        // seeds and therefore this key
        let again = SigningIdentity::from_seeds(&ed, &pq).host_control_key();
        assert_eq!(key, again);

        // another identity derives another, and the post-quantum seed is
        // not in it: the classical seed alone makes this key
        let other = SigningIdentity::from_seeds(&[8u8; 32], &pq).host_control_key();
        assert_ne!(key.public_line, other.public_line);
        let same_ed = SigningIdentity::from_seeds(&ed, &[1u8; 32]).host_control_key();
        assert_eq!(key.public_line, same_ed.public_line);

        // and it is not the identity's own key
        assert_ne!(key.public_bytes, *id.public.ed.as_bytes());
    }

    // acceptance: PRT-09
    #[test]
    fn both_files_are_the_shapes_ssh_reads() {
        let key = HostControlKey::from_seed(&[3u8; 32]);
        assert!(
            key.public_line
                .starts_with("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI")
        );
        assert!(key.public_line.ends_with(" rhtn-host-control"));
        assert_eq!(key.public_line.split(' ').count(), 3);

        assert!(
            key.private_key
                .starts_with("-----BEGIN OPENSSH PRIVATE KEY-----\n")
        );
        assert!(
            key.private_key
                .ends_with("-----END OPENSSH PRIVATE KEY-----\n")
        );
        for line in key.private_key.lines() {
            assert!(line.len() <= 70, "{line}");
        }
        // the container's own magic, inside the base64
        let body: String = key
            .private_key
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .collect();
        let raw = unbase64(&body);
        assert!(raw.starts_with(b"openssh-key-v1\0"));
        // one key, no cipher, no kdf
        assert_eq!(&raw[15..23], &[0, 0, 0, 4, b'n', b'o', b'n', b'e']);
        // the private section is a whole number of eight-byte blocks
        let tail = u32::from_be_bytes([raw[raw.len() - 1], 0, 0, 0]);
        let _ = tail;
        // the public half appears in both halves of the file
        assert!(
            raw.windows(32).any(|w| w == key.public_bytes),
            "the public half is in the container"
        );
    }

    /// Base64 back, for the test alone.
    fn unbase64(s: &str) -> Vec<u8> {
        let mut bits = Vec::new();
        for c in s.bytes() {
            if c == b'=' {
                continue;
            }
            let v = ALPHABET.iter().position(|a| *a == c).expect("base64") as u8;
            bits.push(v);
        }
        let mut out = Vec::new();
        for chunk in bits.chunks(4) {
            let mut n = 0u32;
            for (i, v) in chunk.iter().enumerate() {
                n |= (*v as u32) << (18 - 6 * i);
            }
            out.push((n >> 16) as u8);
            if chunk.len() > 2 {
                out.push((n >> 8) as u8);
            }
            if chunk.len() > 3 {
                out.push(n as u8);
            }
        }
        out
    }

    // acceptance: PRT-10
    #[test]
    fn the_runner_key_is_a_second_key_bound_to_one_program() {
        let id = SigningIdentity::from_seeds(&[7u8; 32], &[9u8; 32]);
        let control = id.host_control_key();
        let runner = id.host_runner_key();

        // **two keys from the one secret**: the operator's own, and the
        // one an instance may be handed for an act
        assert_ne!(control.public_bytes, runner.public_bytes);
        assert_ne!(control.public_line, runner.public_line);
        assert!(control.public_line.ends_with(" rhtn-host-control"));
        assert!(runner.public_line.ends_with(" rhtn-host-runner"));
        // each comes back with the identity, which is the recovery
        assert_eq!(
            runner,
            SigningIdentity::from_seeds(&[7u8; 32], &[9u8; 32]).host_runner_key()
        );

        // **and the host binds the runner key to one program**: the
        // forced command first, every door shut, then the key
        let line = runner.restricted_line();
        assert!(
            line.starts_with("command=\"/usr/local/bin/rhtn-package-runner\","),
            "{line}"
        );
        assert!(line.contains(",restrict "), "{line}");
        for shut in [
            "no-pty",
            "no-port-forwarding",
            "no-agent-forwarding",
            "no-X11-forwarding",
        ] {
            assert!(line.contains(shut), "{line} lacks {shut}");
        }
        assert!(line.ends_with(&runner.public_line), "{line}");
        // the operator's own key carries no such binding: it is a shell
        assert!(!control.public_line.contains("command="));

        // the private files are distinct and each is the shape ssh reads
        assert_ne!(control.private_key, runner.private_key);
        for k in [&control, &runner] {
            assert!(
                k.private_key
                    .starts_with("-----BEGIN OPENSSH PRIVATE KEY-----\n")
            );
            assert!(
                k.private_key
                    .ends_with("-----END OPENSSH PRIVATE KEY-----\n")
            );
        }
    }

    #[test]
    fn the_base64_is_the_standard_one() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
