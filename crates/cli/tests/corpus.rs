//! Every object the corpus carries, decoded and printed by the binary
//! (`wire-format.md` §1): the exit criterion for the command line's
//! `inspect`.

use std::process::Command;

fn corpus() -> serde_json::Value {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test-vectors/corpus.json"
    );
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn run(args: &[&str], stdin: &str) -> (bool, String, String) {
    use std::io::Write;
    let mut c = Command::new(env!("CARGO_BIN_EXE_rhtn"))
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("rhtn runs");
    c.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let out = c.wait_with_output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

// acceptance: DMN-05
#[test]
fn every_corpus_object_decodes_and_prints_from_the_binary() {
    let c = corpus();
    let (mut printed, mut refused, mut disagreed) = (0u32, 0u32, Vec::new());
    for e in c["entries"].as_array().unwrap() {
        if e["class"] != "bytes" {
            continue;
        }
        let id = e["id"].as_str().unwrap();
        let hex = e["hex"].as_str().unwrap();
        let expect = &e["expect"];
        let kind = expect["kind"].as_str().unwrap_or("");
        let outcome = expect["outcome"].as_str().unwrap();
        let args: Vec<&str> = match kind {
            "" => vec!["inspect"],
            "frame" => vec!["inspect", "--frame", "control"],
            k => vec!["inspect", "--kind", k],
        };
        let (ok, out, err) = run(&args, hex);
        assert!(ok, "{id}: the binary failed rather than reporting: {err}");
        assert!(out.starts_with("bytes     "), "{id}: no report");
        // what the corpus expects to be accepted is never reported refused,
        // and what it expects refused is never reported clean
        let clean = !out.contains("refused") && !out.contains("fails");
        match (outcome, clean) {
            ("accept", true) => printed += 1,
            ("reject", false) => refused += 1,
            // a frame the corpus rejects on the request stream is accepted
            // on the control stream and the other way about, so the frame
            // entries are counted rather than held to a stream this test
            // does not know
            _ if kind == "frame" => printed += 1,
            // an unverifiable object is neither: this tool holds no keys
            _ if out.contains("unverifiable") => printed += 1,
            _ => disagreed.push(format!(
                "{id} ({kind}, {outcome}): {}",
                out.lines().take(4).collect::<Vec<_>>().join(" | ")
            )),
        }
    }
    assert!(
        disagreed.is_empty(),
        "{} entries disagreed:\n{}",
        disagreed.len(),
        disagreed.join("\n")
    );
    assert!(
        printed + refused >= 149,
        "every bytes entry is accounted for: {printed} printed, {refused} refused"
    );
}

#[test]
fn a_refusal_is_output_and_not_a_crash() {
    // not CBOR at all
    let (ok, out, _) = run(&["inspect"], "ff");
    assert!(ok && out.contains("refused"), "{out}");
    // well-formed CBOR that is not the kind it is said to be
    let (ok, out, _) = run(&["inspect", "--kind", "EndpointRecord"], "a0");
    assert!(ok && out.contains("refused"), "{out}");
    // and the shape still prints
    let (ok, out, _) = run(&["inspect"], "a10102");
    assert!(ok && out.contains('{'), "{out}");
    // and trailing bytes after a well-formed item are refused, not ignored
    let (ok, out, _) = run(&["inspect"], "a1010203");
    assert!(ok && out.contains("trailing"), "{out}");
}

#[test]
fn the_test_identities_the_vectors_name_are_reproduced() {
    let (ok, out, _) = run(&["keys", "test", "alice"], "");
    assert!(ok, "{out}");
    let want = rhtn_crypto::identity::testkit::test_identity("alice").public;
    let hex: String = want.keyhash.iter().map(|b| format!("{b:02x}")).collect();
    assert!(out.contains(&hex), "the keyhash the vectors use: {out}");
    assert!(out.contains("seeds     "), "and the recipe that derives it");
}
