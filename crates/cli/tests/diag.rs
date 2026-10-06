//! `rhtn diag merge`: two field-test files into one timeline, placed by
//! their anchors and summarised (`Robot/field-test-diagnostics.md`,
//! section 4, *the bench*).

use rhtn_cli::diag::{self, is_refusal, merge, parse};
use std::process::Command;

const RHTND: &str = include_str!("fixtures/diag/rhtnd.jsonl");
const ALICE: &str = include_str!("fixtures/diag/alice.jsonl");

#[test]
fn two_files_merge_by_their_anchors_into_one_timeline() {
    let d = parse("rhtnd", RHTND);
    let a = parse("alice", ALICE);
    // the daemon's anchor is its first line; the instrument's is its second,
    // after the FFI's first span, and the arithmetic does not mind
    assert_eq!(d.anchor, Some((2, 1_700_000_010_000)));
    assert_eq!(a.anchor, Some((12, 1_700_000_011_000)));
    assert_eq!(d.skipped, 1, "the human line is skipped and counted");
    assert_eq!(d.lines.len(), 8);
    assert_eq!(a.lines.len(), 6);

    let lines = merge(&[d.clone(), a.clone()]);
    assert_eq!(lines.len(), 14);
    // wall = unix_ms + (ms - anchor.ms), so the daemon's lines sit at
    // 1700000009998 + ms and alice's at 1700000010988 + ms, and the order
    // is by wall time across the files, not by each file's own ms: alice's
    // `cer.begin` (ms 150) comes before the daemon's attach (ms 1200)
    let order: Vec<(&str, &str)> = lines
        .iter()
        .map(|l| (l.source.as_str(), l.event.as_str()))
        .collect();
    assert_eq!(
        order,
        [
            ("rhtnd", "diag.anchor"),
            ("rhtnd", "daemon.lifecycle"),
            ("rhtnd", "daemon.lifecycle"),
            ("rhtnd", "daemon.lifecycle"),
            ("alice", "ffi.call"),
            ("alice", "diag.anchor"),
            ("alice", "cer.begin"),
            ("rhtnd", "transport.session"),
            ("rhtnd", "node.object"),
            ("rhtnd", "node.object"),
            ("alice", "cer.id_fixed"),
            ("alice", "cer.capture"),
            ("alice", "cer.abort"),
            ("rhtnd", "daemon.lifecycle"),
        ]
    );
    let serving = lines
        .iter()
        .find(|l| l.ms == 40 && l.source == "rhtnd")
        .unwrap();
    assert_eq!(serving.wall, 1_700_000_010_038);
    let begin = lines.iter().find(|l| l.event == "cer.begin").unwrap();
    assert_eq!(begin.wall, 1_700_000_011_138);
    // a line before its file's anchor is placed by the same arithmetic
    let call = lines.iter().find(|l| l.event == "ffi.call").unwrap();
    assert_eq!(call.wall, 1_700_000_010_998);

    // every Decision is visible, as the milestone's exit criterion asks
    let decisions: Vec<String> = lines
        .iter()
        .filter(|l| l.event == "node.object")
        .filter_map(|l| {
            l.fields
                .iter()
                .find(|(k, _)| k == "decision")
                .map(|(_, v)| v.as_str().unwrap().to_string())
        })
        .collect();
    assert_eq!(decisions, ["OutOfStore", "Refused(NotASlot)"]);

    // refusals: the abort by its level, the refused decision by its field
    let refused: Vec<&str> = lines
        .iter()
        .filter(|l| is_refusal(l))
        .map(|l| l.event.as_str())
        .collect();
    assert_eq!(refused, ["node.object", "cer.abort"]);

    let timeline = diag::render_timeline(&lines);
    assert!(timeline.starts_with("        +0  rhtnd"), "{timeline}");
    assert!(
        timeline.contains("cer.abort  abort=NoProximity  step=capture"),
        "{timeline}"
    );

    let summary = diag::render_summary(&[d, a], &lines);
    assert!(
        summary.contains("anchored: unix_ms 1700000010000 at ms 2"),
        "{summary}"
    );
    assert!(summary.contains("1 skipped"), "{summary}");
    // the steps and the time between them: id_fixed is 250 ms after begin
    assert!(summary.contains("  250 ms  cer.id_fixed"), "{summary}");
    assert!(
        summary.contains("1950 ms from first step to last"),
        "{summary}"
    );
    assert!(summary.contains("refusals and aborts\n"), "{summary}");
    assert!(summary.contains("     4  daemon\n"), "{summary}");
    assert!(summary.contains("     4  daemon.lifecycle\n"), "{summary}");
    assert!(summary.contains("     2  node.object\n"), "{summary}");
    assert!(summary.contains("14 events from 2 sources"), "{summary}");
}

#[test]
fn a_file_without_an_anchor_still_reads_and_the_summary_says_so() {
    let text = ALICE
        .lines()
        .filter(|l| !l.contains("diag.anchor"))
        .collect::<Vec<_>>()
        .join("\n");
    let a = parse("alice", &text);
    assert_eq!(a.anchor, None);
    let d = parse("rhtnd", RHTND);
    let lines = merge(&[d.clone(), a.clone()]);
    // its own ms are its wall time, which sorts it before everything anchored
    assert_eq!(lines[0].source, "alice");
    assert_eq!(lines[4].source, "alice");
    assert_eq!(lines[5].source, "rhtnd");
    let summary = diag::render_summary(&[d, a], &lines);
    assert!(summary.contains("no anchor"), "{summary}");
}

#[test]
fn the_binary_merges_the_fixtures() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/diag/");
    let out = Command::new(env!("CARGO_BIN_EXE_rhtn"))
        .args([
            "diag",
            "merge",
            &format!("{dir}rhtnd.jsonl"),
            &format!("{dir}alice.jsonl"),
        ])
        .output()
        .expect("rhtn runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("node.object"), "{text}");
    assert!(text.contains("counts per event"), "{text}");
    let none = Command::new(env!("CARGO_BIN_EXE_rhtn"))
        .args(["diag", "merge"])
        .output()
        .expect("rhtn runs");
    assert!(!none.status.success(), "no file is a usage error");

    // two files of one name, as every daemon's `diag.jsonl` is, are told
    // apart by their directories
    assert_eq!(
        diag::labels(&[
            "/run/alice/diag.jsonl",
            "/run/bob/diag.jsonl",
            "/run/phone.jsonl"
        ]),
        ["alice/diag.jsonl", "bob/diag.jsonl", "phone.jsonl"]
    );
    let tmp = std::env::temp_dir().join(format!("rhtn-diag-labels-{}", std::process::id()));
    for who in ["alice", "bob"] {
        std::fs::create_dir_all(tmp.join(who)).unwrap();
        std::fs::copy(
            format!("{dir}rhtnd.jsonl"),
            tmp.join(who).join("diag.jsonl"),
        )
        .unwrap();
    }
    let out = Command::new(env!("CARGO_BIN_EXE_rhtn"))
        .args(["diag", "merge"])
        .arg(tmp.join("alice").join("diag.jsonl"))
        .arg(tmp.join("bob").join("diag.jsonl"))
        .output()
        .expect("rhtn runs");
    let _ = std::fs::remove_dir_all(&tmp);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("alice/diag.jsonl") && text.contains("bob/diag.jsonl"),
        "{text}"
    );
}

/// **A line carrying two `ms` sorts by the first one, which is its time.**
/// The emitters wrote a duration under `ms` as well until 2026-10-06
/// (`took_ms` now), and `serde_json` keeps the last of a duplicate key, so
/// the merge sorted such an event by its duration: in the field runs'
/// `summary.txt` that put `cer.capture` eleven seconds in, before the
/// `cer.begin` it followed [reviewer, 2026-10-01]. Every log already on
/// disk is read by this path, so the fix lives here as well as at the
/// emitters.
#[test]
fn an_event_carrying_a_duration_under_the_time_s_key_sorts_by_the_time() {
    let doubled = concat!(
        "{\"ms\":10,\"level\":\"info\",\"layer\":\"shell\",\"event\":\"diag.anchor\",\"unix_ms\":1700000000000}\n",
        "{\"ms\":65032,\"level\":\"info\",\"layer\":\"cer\",\"event\":\"cer.capture\",\"ms\":10851}\n",
        "{\"ms\":20000,\"level\":\"info\",\"layer\":\"cer\",\"event\":\"cer.begin\"}\n",
    );
    let p = parse("phone", doubled);
    assert_eq!(p.lines.len(), 3, "both lines read");
    let merged = merge(&[p]);
    let order: Vec<&str> = merged.iter().map(|l| l.event.as_str()).collect();
    assert_eq!(
        order,
        vec!["diag.anchor", "cer.begin", "cer.capture"],
        "the capture sorts at 65032 and not at its 10851 ms duration"
    );
}

/// And the duration is now a field of its own, so the merge shows it
/// rather than dropping it with the timestamp's key.
#[test]
fn a_duration_in_its_own_field_survives_into_the_timeline() {
    let line = concat!(
        "{\"ms\":10,\"level\":\"info\",\"layer\":\"shell\",\"event\":\"diag.anchor\",\"unix_ms\":1700000000000}\n",
        "{\"ms\":65032,\"level\":\"info\",\"layer\":\"cer\",\"event\":\"cer.finalize\",\"took_ms\":44155}\n",
    );
    let p = parse("phone", line);
    let lines = merge(&[p]);
    let fields = &lines.last().expect("a line").fields;
    assert!(
        fields
            .iter()
            .any(|(k, v)| k == "took_ms" && v.as_u64() == Some(44155)),
        "the duration is a field: {fields:?}"
    );
}
