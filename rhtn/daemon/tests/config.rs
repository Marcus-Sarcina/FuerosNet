//! The operator's configuration file: what it accepts, and what it refuses
//! rather than guessing at (`infra-client-requirements.md` §1).

use rhtn_daemon::config::Config;
use rhtn_node::resolution::Ingestion;

const GOOD: &str = "\
# a root that attaches to nobody
identity = /var/lib/rhtn/identity.key
listen = 127.0.0.1:7431
queue = /var/lib/rhtn/queue
prekeys = /var/lib/rhtn/prekeys
topology = /var/lib/rhtn/topology
archive = /var/lib/rhtn/archive
heartbeat = 30
ingestion = unverified-gossip
allowance = 120/60
";

#[test]
fn a_complete_configuration_reads_and_a_missing_key_has_no_default() {
    let c = Config::parse(GOOD).expect("a complete configuration");
    assert_eq!(c.listen, "127.0.0.1:7431".parse().unwrap());
    assert_eq!(c.heartbeat_secs, 30);
    assert_eq!(c.ingestion, Ingestion::UnverifiedGossip);
    assert_eq!(c.request_allowance, (120, 60));
    assert_eq!(c.upstream, None, "a root attaches to nobody");
    assert_eq!(c.queue_cap, None, "absent is uncapped");
    // every required key is required: none of them has a default, because a
    // default here is a policy choice made by omission
    for key in ["identity", "listen", "queue", "prekeys", "topology", "archive", "heartbeat", "ingestion", "allowance"] {
        let without: String = GOOD.lines().filter(|l| !l.starts_with(key)).collect::<Vec<_>>().join("\n");
        let e = Config::parse(&without).unwrap_err();
        assert!(e.what.contains(key) && e.what.contains("no default"), "{key}: {e}");
    }
}

#[test]
fn an_unknown_or_repeated_key_is_refused_naming_its_line() {
    let e = Config::parse(&format!("{GOOD}listne = 127.0.0.1:1\n")).unwrap_err();
    assert!(e.what.contains("listne") && e.what.contains("not a configuration key"), "{e}");
    assert_eq!(e.line, 11, "the line it was on");
    let e = Config::parse(&format!("{GOOD}heartbeat = 60\n")).unwrap_err();
    assert!(e.what.contains("already set on line 8"), "{e}");
    let e = Config::parse(&format!("{GOOD}nonsense\n")).unwrap_err();
    assert!(e.what.contains("not `key = value`"), "{e}");
}

#[test]
fn a_value_outside_what_the_wire_allows_is_refused() {
    let heartbeat = |v: &str| Config::parse(&GOOD.replace("heartbeat = 30", &format!("heartbeat = {v}"))).map(|c| c.heartbeat_secs);
    assert_eq!(heartbeat("1").unwrap(), 1);
    assert_eq!(heartbeat("3600").unwrap(), 3600);
    // §8.2 fixes the range: zero makes every session instantly overdue and
    // over an hour detects nothing
    assert!(heartbeat("0").unwrap_err().what.contains("1 to 3600"));
    assert!(heartbeat("3601").unwrap_err().what.contains("1 to 3600"));
    assert!(heartbeat("thirty").unwrap_err().what.contains("not a count"));
    // an allowance of zero would serve nobody
    let allowance = |v: &str| Config::parse(&GOOD.replace("allowance = 120/60", &format!("allowance = {v}")));
    assert!(allowance("0/60").unwrap_err().what.contains("serves nobody"));
    assert!(allowance("120/0").unwrap_err().what.contains("serves nobody"));
    assert!(allowance("120").unwrap_err().what.contains("requests/seconds"));
    assert_eq!(allowance("8/30").unwrap().request_allowance, (8, 30));
    // and the ingestion boundary is named, not guessed
    let ing = |v: &str| Config::parse(&GOOD.replace("ingestion = unverified-gossip", &format!("ingestion = {v}")));
    assert_eq!(ing("verified-on-acceptance").unwrap().ingestion, Ingestion::VerifiedOnAcceptance);
    assert!(ing("sometimes").unwrap_err().what.contains("verified-on-acceptance"));
}

#[test]
fn an_upstream_names_a_keyhash_and_the_addresses_it_is_reached_at() {
    let k = "a".repeat(64);
    let c = Config::parse(&format!("{GOOD}upstream = {k} 10.0.0.1:7431, 10.0.0.2:7431\n")).expect("an upstream");
    let (key, addrs) = c.upstream.expect("set");
    assert_eq!(key, [0xaa; 32]);
    assert_eq!(addrs, vec!["10.0.0.1:7431".parse().unwrap(), "10.0.0.2:7431".parse().unwrap()]);
    // a keyhash that is not 64 lower-case hex digits is refused rather than
    // normalised: an upper-case one is a different string
    let bad = |v: &str| Config::parse(&format!("{GOOD}upstream = {v}\n")).unwrap_err();
    assert!(bad(&format!("{} 10.0.0.1:7431", "A".repeat(64))).what.contains("lower-case hex"));
    assert!(bad(&format!("{} 10.0.0.1:7431", "a".repeat(63))).what.contains("lower-case hex"));
    assert!(bad(&format!("{k} not-an-address")).what.contains("not an address"));
    assert!(bad(&k).what.contains("keyhash then its addresses"));
}
