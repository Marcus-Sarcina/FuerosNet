//! The field-test flavour of `rhtnd` (`Robot/field-test-diagnostics.md`,
//! milestone M2): two daemon processes with a `[log]` table each write
//! their events as JSON lines, and `rhtn diag merge` reads the two files
//! into one timeline with every storage decision visible.
//!
//! Built and run with `--no-default-features --features fieldtest`; the
//! releasable flavour compiles the hooks out, so this file is empty there.
#![cfg(feature = "fieldtest")]

use rhtn_cli::diag;
use rhtn_sim::daemons::Daemons;
use std::time::{Duration, Instant};

const CAST: [&str; 2] = ["alice", "bob"];

/// Wait until `path` holds a line naming `event`, or give up.
fn awaits(path: &std::path::Path, event: &str, limit: Duration) -> bool {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        if std::fs::read_to_string(path).is_ok_and(|t| t.contains(event)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[test]
fn two_daemons_with_a_log_table_write_their_lifecycle_and_decisions_and_the_files_merge() {
    let mut set = Daemons::new(env!("CARGO_BIN_EXE_rhtnd"), "fieldtest");
    let alice_log = set.log_to("alice", "debug");
    let bob_log = set.log_to("bob", "debug");
    // alice is a root; bob runs under it and attaches on the way up, which
    // has bob originate its endpoint record (a decision at bob) and push
    // it to alice (a decision at alice, out of its store since nothing
    // binds the two)
    set.start("alice", &CAST, None);
    set.start("bob", &CAST, Some("alice"));
    assert!(
        awaits(&bob_log, "node.object", Duration::from_secs(30)),
        "bob decided about an object: {}",
        std::fs::read_to_string(&bob_log).unwrap_or_default()
    );
    assert!(
        awaits(&alice_log, "node.object", Duration::from_secs(30)),
        "alice decided about bob's push: {}",
        std::fs::read_to_string(&alice_log).unwrap_or_default()
    );
    // a stop is a SIGTERM, and the shutdown is the last thing written
    set.stop("bob");
    set.stop("alice");
    let alice_text = std::fs::read_to_string(&alice_log).expect("alice's file");
    let bob_text = std::fs::read_to_string(&bob_log).expect("bob's file");

    for (who, text) in [("alice", &alice_text), ("bob", &bob_text)] {
        let first = text.lines().next().unwrap_or_default();
        assert!(
            first.contains("\"event\":\"diag.anchor\"") && first.contains("\"unix_ms\":"),
            "{who}'s first line is the anchor: {first}"
        );
        for step in [
            "config_read",
            "start",
            "identity_loaded",
            "replayed",
            "serving",
            "signalled",
            "persist",
            "shutdown",
        ] {
            assert!(
                text.contains(&format!(
                    "\"event\":\"daemon.lifecycle\",\"step\":\"{step}\""
                )),
                "{who} wrote the lifecycle step {step}:\n{text}"
            );
        }
        assert!(
            text.contains("\"event\":\"node.object\""),
            "{who} wrote a decision"
        );
        assert!(
            text.contains("\"event\":\"transport.session\""),
            "{who}'s session log reached the file"
        );
    }
    assert!(
        bob_text.contains("\"step\":\"upstream\",\"patron8\":"),
        "bob said whether it attached upstream:\n{bob_text}"
    );

    // and the two files merge into one timeline, placed by their anchors,
    // with every decision visible
    let sources = [
        diag::parse("alice", &alice_text),
        diag::parse("bob", &bob_text),
    ];
    assert!(sources.iter().all(|s| s.anchor.is_some()), "both anchored");
    assert_eq!(
        sources[0].skipped + sources[1].skipped,
        0,
        "every line parsed"
    );
    let lines = diag::merge(&sources);
    let decisions: Vec<(&str, String)> = lines
        .iter()
        .filter(|l| l.event == "node.object")
        .map(|l| {
            let d = l
                .fields
                .iter()
                .find(|(k, _)| k == "decision")
                .and_then(|(_, v)| v.as_str())
                .expect("a decision field");
            (l.source.as_str(), d.to_string())
        })
        .collect();
    assert!(
        decisions.iter().any(|(s, d)| *s == "bob" && d == "Stored"),
        "bob stored its own endpoint record: {decisions:?}"
    );
    assert!(
        decisions.iter().any(|(s, _)| *s == "alice"),
        "alice's decision is in the timeline: {decisions:?}"
    );
    // alice started first, and the merge says so by the wall clock
    assert_eq!(
        lines[0].source,
        "alice",
        "{}",
        diag::render_timeline(&lines[..4])
    );
    let summary = diag::render_summary(&sources, &lines);
    assert!(summary.contains("node.object"), "{summary}");
    assert!(summary.contains("2 sources"), "{summary}");
}
