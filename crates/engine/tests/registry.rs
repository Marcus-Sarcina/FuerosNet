//! **The design's registry entry is this engine's** (design §7.5).
//!
//! The entry names a version number and fixes two lengths, and nothing
//! had ever compared what the document says against what this crate
//! does: the document said version 1 with a 128-byte `int8` template,
//! which was the *profile*'s shape under a number nothing used, and the
//! drift stood until somebody read both [2026-10-18]. **The empirical
//! result from the implementation is what the specification adopts**
//! [author, 2026-10-18], so this test holds the document to the
//! constants rather than the other way about: change a constant here and
//! the entry has to be rewritten in the same commit.

use rhtn_engine::{DIM, PROFILE_LEN, SIDE, TEMPLATE_LEN, TEMPLATE_VERSION};

/// The design's §7.5 registry entry, from its heading to the end of the
/// paragraph that follows its table.
fn entry() -> String {
    let design = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../network-design.md"
    ))
    .expect("the design is beside the workspace");
    let from = design
        .find("**Template version")
        .expect("§7.5's registry entry, which opens `**Template version`");
    let rest = &design[from..];
    let to = rest
        .find("**Modality versioning.**")
        .expect("the entry runs to §7.5's modality-versioning paragraph");
    rest[..to].to_string()
}

#[test]
fn the_design_states_this_engines_version_and_lengths() {
    let e = entry();
    let says = |what: &str| {
        assert!(
            e.contains(what),
            "design §7.5's registry entry does not say {what:?}:\n{e}"
        );
    };
    says(&format!("version {TEMPLATE_VERSION}"));
    says("`face/sface-int8-v1`");
    // the two lengths, which are different objects and were conflated
    says(&format!("{TEMPLATE_LEN} bytes"));
    says(&format!("{PROFILE_LEN} bytes"));
    assert_eq!(TEMPLATE_LEN, DIM * 4, "128 f32, little-endian");
    assert_eq!(PROFILE_LEN, DIM, "128 int8");
    // the detection's geometry and the comparison
    says(&format!("{SIDE}×{SIDE}"));
    says("cosine");
    says(&format!("{DIM} dimensions"));
    // version 1 is the stand-in's and interoperates with nothing, which
    // is why this engine's is 2
    says("Version 1 is the");
    assert_eq!(TEMPLATE_VERSION, 2);
}
