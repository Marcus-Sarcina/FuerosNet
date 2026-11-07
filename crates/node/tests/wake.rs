//! The wake register across a restart, and the two secrets it holds
//! (design §14.1.5; `infra-client-requirements.md` §6.1;
//! `wire-format.md` §7.10).
//!
//! **Kept rather than held in memory** [author, 2026-11-07]: *"I don't
//! think the security gain over this limited lever is worth the cost of
//! breaking resource functionality if a patron's phone goes offline for a
//! few days."*  So a node that restarts can still ring the doorbells it
//! was given, and what these hold is that it can — and that the files are
//! shut.

use rhtn_node::wake::{Registered, WakeRegister};

fn kh(n: u8) -> [u8; 32] {
    [n; 32]
}

/// A register kept under a directory of this test's own.
fn at(name: &str) -> (WakeRegister, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("rhtn-wake-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a directory");
    let mut w = WakeRegister::new();
    w.at(dir.clone());
    (w, dir)
}

#[test]
fn an_endpoint_and_both_its_keys_come_back_after_a_restart() {
    let (mut w, dir) = at("restart");
    assert_eq!(
        w.register(
            kh(1),
            kh(9),
            Some("https://push.example/rhtn/a3f9".into()),
            Some(vec![7; 32]),
            Some(1_900_000_000),
            Some([3; 32]),
        ),
        Registered::Held
    );

    // a second register over the same directory is the restart
    let mut again = WakeRegister::new();
    again.at(dir.clone());
    let back = again
        .get_for(&kh(1), &kh(9))
        .expect("the endpoint comes back");
    assert_eq!(back.url, "https://push.example/rhtn/a3f9");
    assert_eq!(back.key, vec![7; 32]);
    assert_eq!(
        back.signing_key, [3; 32],
        "the key the post is signed with comes back too, or the node holds \
         an endpoint it cannot ring"
    );
    assert_eq!(back.lapses_at, Some(1_900_000_000));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_files_holding_them_are_shut() {
    // **two secrets a client lent this node** (§6.1), so the default mode
    // would leave them readable by anything else on the operator's machine
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let (mut w, dir) = at("modes");
        w.register(
            kh(2),
            kh(8),
            Some("https://push.example/one".into()),
            Some(vec![1; 32]),
            None,
            Some([4; 32]),
        );
        let d = dir.join("wake");
        let mode =
            |p: &std::path::Path| std::fs::metadata(p).expect("there").permissions().mode() & 0o777;
        assert_eq!(mode(&d), 0o700, "the directory is the owner's alone");
        let file = std::fs::read_dir(&d)
            .expect("readable")
            .flatten()
            .next()
            .expect("one file")
            .path();
        assert_eq!(mode(&file), 0o600, "and so is the file");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn an_endpoint_with_no_signer_is_refused_and_nothing_is_written() {
    let (mut w, dir) = at("nosigner");
    assert_eq!(
        w.register(
            kh(3),
            kh(7),
            Some("https://push.example/two".into()),
            Some(vec![1; 32]),
            None,
            None,
        ),
        Registered::Refused,
        "an endpoint the node cannot sign a post for is one the push service \
         would refuse, and the URL alone would be enough to ring it"
    );
    assert!(w.get(&kh(3)).is_none(), "and nothing is held");
    assert!(
        !dir.join("wake").exists() || std::fs::read_dir(dir.join("wake")).unwrap().count() == 0,
        "nor written"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_withdrawal_takes_the_file_with_it() {
    let (mut w, dir) = at("withdraw");
    w.register(
        kh(4),
        kh(6),
        Some("https://push.example/three".into()),
        Some(vec![1; 32]),
        None,
        Some([5; 32]),
    );
    let d = dir.join("wake");
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
    assert_eq!(
        w.register(kh(4), kh(6), None, None, None, None),
        Registered::Withdrawn
    );
    // **the lent secret goes with the endpoint** (§6.1): keeping it past
    // the reason it was given is a retention decision made by omission
    assert_eq!(
        std::fs::read_dir(&d).unwrap().count(),
        0,
        "the file is gone, and the key in it with it"
    );
    let mut again = WakeRegister::new();
    again.at(dir.clone());
    assert!(
        again.get(&kh(4)).is_none(),
        "and stays gone across a restart"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_signing_key_is_named_and_not_shown() {
    let (mut w, dir) = at("debug");
    w.register(
        kh(5),
        kh(5),
        Some("https://push.example/four".into()),
        Some(vec![1; 32]),
        None,
        Some([0xab; 32]),
    );
    let shown = format!("{:?}", w.get(&kh(5)).expect("held"));
    assert!(shown.contains("signing_key"), "the field is named: {shown}");
    assert!(
        shown.contains("<redacted>"),
        "and its value is not: {shown}"
    );
    assert!(!shown.contains("171"), "no byte of it: {shown}");
    assert!(
        !shown.to_lowercase().contains("ab, ab"),
        "nor any run: {shown}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
