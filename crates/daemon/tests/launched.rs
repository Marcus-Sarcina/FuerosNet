//! **The configuration a light client writes is one this daemon reads.**
//!
//! `rhtn_launch::node_config` composes the configuration an instance is
//! launched with, and `rhtn_daemon::config` is what reads it on the
//! machine. **Nothing checked that the two agreed**, and twice now they
//! have not:
//!
//!  - 2026-10-31, the `resources` and `state` keys: a launched instance
//!    could host nothing, found by trying to install on one.
//!  - 2026-11-02, the same shape from the other side: the configuration
//!    is strict by design — *"an unknown key, a repeated key, a missing
//!    one or a value that does not parse is an error naming its line"* —
//!    so a client that writes a key the image's daemon does not know has
//!    an instance that **refuses to start at all**, and the operator sees
//!    a machine that came up and a node that did not.
//!
//! The second is the dangerous one, because it fails on a rented machine
//! at the moment of launch with the image already pulled. This test is
//! the pairing, checked at every commit instead.

use rhtn_daemon::config::Config;

/// The ports the client's own page uses, which is what the configuration
/// it writes carries.
const QUIC: u16 = 7431;
const ADMIN: u16 = 7449;

// acceptance: DMN-41
#[test]
fn the_configuration_a_client_writes_for_an_instance_is_one_this_daemon_reads() {
    let token = "ab".repeat(32);
    let text = rhtn_launch::node_config(&token, QUIC, ADMIN);
    let cfg = Config::parse(&text).unwrap_or_else(|e| {
        panic!(
            "the daemon refuses the configuration its own client writes: {e}\n\n{text}\n\n\
             A key one side knows and the other does not is an instance that will not start, \
             and the operator meets it on a rented machine."
        )
    });

    // **it is an instance and not a node holding a seed** (design §23.3):
    // the operator's material and a transport key it mints, no identity
    assert!(cfg.identity.is_none());
    assert!(cfg.operator.is_some());
    assert!(cfg.transport_key.is_some());
    assert!(cfg.delegations.is_some());

    // **and it can host a package and be asked to install one**, which is
    // the whole of what the two gaps above were about
    assert!(
        cfg.resources.is_some(),
        "a launched instance that names no hosting file can host nothing"
    );
    assert!(
        cfg.state.is_some(),
        "a package wanting a block or a log cannot be hosted without one"
    );
    let a = cfg
        .administration
        .as_ref()
        .expect("the surface its operator reaches it over");
    assert_eq!(a.listen.port(), ADMIN);
    assert!(
        a.token_file.is_some(),
        "the token that travelled in the provisioning data must be replaceable"
    );
    assert!(
        a.host_shell.is_some(),
        "an instance that cannot reach its own host installs nothing"
    );
    assert!(
        a.host_shell.is_some_and(|s| s.ip().is_loopback()),
        "and it asks its own host and no other machine"
    );
    assert_eq!(cfg.listen.port(), QUIC);
}

// acceptance: DMN-41
#[test]
fn the_script_that_carries_that_configuration_carries_it_whole() {
    // **the indentation is part of the pairing**: the configuration goes
    // into cloud-init as a block, and a line that lost its prefix would
    // be a file the daemon reads differently from the one the test above
    // checked. So the script's own copy is lifted back out and read.
    let token = "cd".repeat(32);
    let script = rhtn_launch::user_data(
        "82a301",
        &token,
        &rhtn_launch::Image::default(),
        QUIC,
        ADMIN,
        None,
        None,
    );
    let begins = script
        .find("- path: /var/lib/rhtn/rhtnd.conf")
        .expect("the configuration is written by the script");
    let after = &script[begins..];
    let body = after.split_once("content: |\n").expect("a block follows").1;
    let lifted: String = body
        .lines()
        .take_while(|l| l.starts_with("     ") || l.trim().is_empty())
        .map(|l| format!("{}\n", l.strip_prefix("     ").unwrap_or(l)))
        .collect();
    Config::parse(&lifted).unwrap_or_else(|e| {
        panic!("the configuration the script carries does not read: {e}\n\n{lifted}")
    });
}
