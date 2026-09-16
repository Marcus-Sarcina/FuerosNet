//! The operator's configuration file: what it accepts, and what it refuses
//! rather than guessing at (`infra-client-requirements.md` §1).

use rhtn_daemon::config::Config;
use rhtn_node::resolution::Ingestion;

const GOOD: &str = r#"
# a root that attaches to nobody
identity = "/var/lib/rhtn/identity.key"
listen = "127.0.0.1:7431"
queue = "/var/lib/rhtn/queue"
prekeys = "/var/lib/rhtn/prekeys"
topology = "/var/lib/rhtn/topology"
archive = "/var/lib/rhtn/archive"
heartbeat = 30
ingestion = "unverified-gossip"

[allowance]
requests = 120
seconds = 60
"#;

/// `GOOD` with one line replaced, so a test changes one thing and says so.
fn with(from: &str, to: &str) -> String {
    assert!(GOOD.contains(from), "{from} is not in the sample");
    GOOD.replace(from, to)
}

#[test]
fn a_complete_configuration_reads_and_a_missing_key_has_no_default() {
    let c = Config::parse(GOOD).expect("a complete configuration");
    assert_eq!(c.listen, "127.0.0.1:7431".parse().unwrap());
    assert_eq!(c.heartbeat_secs, 30);
    assert_eq!(c.ingestion, Ingestion::UnverifiedGossip);
    assert_eq!(c.request_allowance, (120, 60));
    assert_eq!(c.upstream, None, "a root attaches to nobody");
    assert_eq!(c.queue_cap, None, "absent is uncapped");
    assert_eq!(c.resources, None, "and it hosts nothing");
    assert_eq!(c.resource_limits, None);

    // every required key is required: none of them has a default, because a
    // default here is a policy choice made by omission
    for key in ["identity", "listen", "queue", "prekeys", "topology", "archive", "heartbeat", "ingestion"] {
        let without: String = GOOD.lines().filter(|l| !l.trim_start().starts_with(key)).collect::<Vec<_>>().join("\n");
        let e = Config::parse(&without).unwrap_err();
        assert!(e.what.contains(key) && e.what.contains("no default"), "{key}: {e}");
    }
    let e = Config::parse(&GOOD[..GOOD.find("[allowance]").unwrap()]).unwrap_err();
    assert!(e.what.contains("allowance") && e.what.contains("no default"), "a table with no default either: {e}");
}

#[test]
fn an_unknown_or_repeated_key_is_refused_naming_its_line() {
    let e = Config::parse(&format!("listne = \"127.0.0.1:1\"{GOOD}")).unwrap_err();
    assert!(e.what.contains("listne") && e.what.contains("unknown field"), "{e}");
    assert!(e.what.contains("identity"), "and the keys it did expect: {e}");
    let e = Config::parse(&with("heartbeat = 30", "heartbeat = 30\nheartbeat = 60")).unwrap_err();
    assert!(e.what.contains("duplicate key") && e.what.contains("heartbeat"), "{e}");
    let e = Config::parse(&format!("{GOOD}nonsense\n")).unwrap_err();
    assert!(e.line > 0, "a shape error knows where it is: {e}");
}

#[test]
fn a_value_outside_what_the_wire_allows_is_refused() {
    let heartbeat = |v: &str| Config::parse(&with("heartbeat = 30", &format!("heartbeat = {v}"))).map(|c| c.heartbeat_secs);
    assert_eq!(heartbeat("1").unwrap(), 1);
    assert_eq!(heartbeat("3600").unwrap(), 3600);
    // §8.2 fixes the range: zero makes every session instantly overdue and
    // over an hour detects nothing
    let zero = heartbeat("0").unwrap_err();
    assert!(zero.what.contains("1 to 3600"), "{zero}");
    assert_eq!(zero.line, 9, "the line it was written on");
    assert!(heartbeat("3601").unwrap_err().what.contains("1 to 3600"));
    let word = heartbeat("\"thirty\"").unwrap_err();
    assert!(word.what.contains("invalid type") && word.what.contains("thirty"), "a string is not a count: {word}");

    // an allowance of zero would serve nobody
    let allowance = |r: &str, s: &str| Config::parse(&with("requests = 120", &format!("requests = {r}")).replace("seconds = 60", &format!("seconds = {s}")));
    assert!(allowance("0", "60").unwrap_err().what.contains("serves nobody"));
    assert!(allowance("120", "0").unwrap_err().what.contains("serves nobody"));
    assert_eq!(allowance("8", "30").unwrap().request_allowance, (8, 30));

    // and the ingestion boundary is named, not guessed
    let ing = |v: &str| Config::parse(&with("ingestion = \"unverified-gossip\"", &format!("ingestion = \"{v}\"")));
    assert_eq!(ing("verified-on-acceptance").unwrap().ingestion, Ingestion::VerifiedOnAcceptance);
    let bad = ing("sometimes").unwrap_err();
    assert!(bad.what.contains("verified-on-acceptance"), "{bad}");
    assert_eq!(bad.line, 10, "the line it was written on");

    // a package budget of zero admits and then runs nothing
    let limits = |m: &str, f: &str| Config::parse(&format!("{GOOD}\n[resource-limits]\nmemory = {m}\nfuel = {f}\n"));
    assert_eq!(limits("1024", "5").unwrap().resource_limits, Some((1024, 5)));
    assert!(limits("0", "5").unwrap_err().what.contains("runs none of it"));
    assert!(limits("1024", "0").unwrap_err().what.contains("runs none of it"));
}

#[test]
fn an_upstream_names_a_keyhash_and_the_addresses_it_is_reached_at() {
    let k = "a".repeat(64);
    let up = |body: &str| Config::parse(&format!("{GOOD}\n[upstream]\n{body}"));
    let c = up(&format!("node = \"{k}\"\naddresses = [\"10.0.0.1:7431\", \"10.0.0.2:7431\"]\n")).expect("an upstream");
    let (key, addrs) = c.upstream.expect("set");
    assert_eq!(key, [0xaa; 32]);
    assert_eq!(addrs, vec!["10.0.0.1:7431".parse().unwrap(), "10.0.0.2:7431".parse().unwrap()]);

    // a keyhash that is not 64 lower-case hex digits is refused rather than
    // normalised: an upper-case one is a different string
    let bad = |body: &str| up(body).unwrap_err();
    assert!(bad(&format!("node = \"{}\"\naddresses = [\"10.0.0.1:7431\"]\n", "A".repeat(64))).what.contains("lower-case hex"));
    assert!(bad(&format!("node = \"{}\"\naddresses = [\"10.0.0.1:7431\"]\n", "a".repeat(63))).what.contains("lower-case hex"));
    assert!(bad(&format!("node = \"{k}\"\naddresses = [\"not-an-address\"]\n")).what.contains("not an address"));
    assert!(bad(&format!("node = \"{k}\"\naddresses = []\n")).what.contains("names no address"));
    assert!(bad(&format!("node = \"{k}\"\n")).what.contains("addresses"), "and the field it wanted is named");
}

/// **The interval is the operator's** (`wire-format.md` §10.1.3 asks for
/// the periodic replay and states no interval): absent, a default; zero is
/// an operator saying its links do not lose frames.
// acceptance: DMN-22
#[test]
fn the_reconciliation_interval_defaults_and_bounds_and_can_be_turned_off() {
    let every = |v: &str| Config::parse(&with("heartbeat = 30", &format!("heartbeat = 30\nreconcile = {v}"))).map(|c| c.reconcile_secs);
    assert_eq!(Config::parse(&with("heartbeat = 30", "heartbeat = 30")).unwrap().reconcile_secs, 900, "a default, not a rule any document states");
    assert_eq!(every("60").unwrap(), 60);
    assert_eq!(every("86400").unwrap(), 86_400);
    assert_eq!(every("0").unwrap(), 0, "zero turns the replay off rather than being refused");
    let over = every("86401").unwrap_err();
    assert!(over.what.contains("0 to 86400"), "{over}");
    assert!(over.line > 0, "and it knows where it was written: {over}");
    let word = every("\"often\"").unwrap_err();
    assert!(word.what.contains("invalid type"), "a string is not a count: {word}");
}
