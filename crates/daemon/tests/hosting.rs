//! What an operator's hosting file may say, and what it may not.

use rhtn_daemon::hosting::apply;
use rhtn_node::resources::Gateway;
use rhtn_resources::Limits;
use std::path::{Path, PathBuf};

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Dir {
        let d = std::env::temp_dir().join(format!("rhtn-hosting-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("a directory");
        Dir(d)
    }
    fn put(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).expect("written");
        p
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const RES: &str = "0909090909090909090909090909090909090909090909090909090909090909";
const WHO: &str = "0101010101010101010101010101010101010101010101010101010101010101";

/// One `[[host]]` entry naming `manifest`, which most of these start from.
fn host(manifest: &Path) -> String {
    format!("[[host]]\nresource = \"{RES}\"\nowner = \"{WHO}\"\nauthority = \"shop.internal\"\nmanifest = \"{}\"\n", manifest.display())
}

fn run(d: &Dir, text: &str) -> Result<usize, String> {
    let f = d.put("hosting", text);
    let mut g = Gateway::default();
    apply(&mut g, Path::new(&f), Limits::default()).map_err(|e| e.to_string())
}

// acceptance: DMN-20
#[test]
fn a_manifest_that_does_not_describe_its_component_is_refused() {
    let d = Dir::new("manifest");
    d.put("echo.wasm", rhtn_sim::packages::echo());
    let full = d.put("full.manifest", "roles = reader\nimports = rhtn/1:request,rhtn/1:response\ncomponent = echo.wasm\n");
    let quiet = d.put("quiet.manifest", "roles = reader\ncomponent = echo.wasm\n");
    let extra = d.put("extra.manifest", "roles = reader\nimports = rhtn/1:request,rhtn/1:response,rhtn/1:topology\ncomponent = echo.wasm\n");

    assert_eq!(run(&d, &host(&full)), Ok(1), "a manifest that says what its component does");

    // under-declaring is the case that matters: a manifest an operator
    // reads and believes reaches nothing, over a component that reaches
    let under = run(&d, &host(&quiet)).expect_err("refused");
    assert!(under.contains("declares []"), "the operator is shown both sides: {under}");
    assert!(under.contains("rhtn/1:request"), "including what the component actually reaches: {under}");

    // and over-declaring is refused by the host having no such binding,
    // before the component is even read
    let over = run(&d, &host(&extra)).expect_err("refused");
    assert!(over.contains("rhtn/1:topology"), "the binding it asked for is named: {over}");

    for (name, text, wrong) in [
        ("no-component.manifest", "roles = reader\n", "`component` is not set"),
        ("twice.manifest", "roles = reader\nroles = writer\ncomponent = echo.wasm\n", "already set"),
        ("unknown.manifest", "storage = 1G\ncomponent = echo.wasm\n", "not a manifest key"),
        ("reserved.manifest", "roles = connect\ncomponent = echo.wasm\n", "reserved"),
    ] {
        let m = d.put(name, text);
        let e = run(&d, &host(&m)).expect_err("refused");
        assert!(e.contains(wrong), "{name}: expected {wrong:?}, got {e}");
    }
}

// acceptance: DMN-21
#[test]
fn a_grant_is_checked_against_the_package_before_anything_is_bound() {
    let d = Dir::new("grants");
    d.put("echo.wasm", rhtn_sim::packages::echo());
    let m = d.put("echo.manifest", "roles = reader,writer\nimports = rhtn/1:request,rhtn/1:response\ncomponent = echo.wasm\n");
    let one = host(&m);
    let granting = |roles: &str| format!("{one}\n[[host.grant]]\nmember = \"{WHO}\"\nroles = [{roles}]\n");

    assert_eq!(run(&d, &granting("\"connect\", \"reader\", \"writer\"")), Ok(1), "roles the package declared");
    assert_eq!(run(&d, &granting("\"reader\"")), Ok(1), "and a row that cannot connect is a row an operator may write");

    let e = run(&d, &granting("\"connect\", \"admin\"")).expect_err("refused");
    assert!(e.contains("`admin` is not a role that package declared"), "{e}");
    let e = run(&d, &granting("\"connect\", \"discover\"")).expect_err("refused");
    assert!(e.contains("reserved"), "{e}");

    // nothing is bound out of a file that is refused anywhere in it
    let f = d.put("hosting", granting("\"connect\", \"admin\""));
    let mut g = Gateway::default();
    assert!(apply(&mut g, Path::new(&f), Limits::default()).is_err());
    assert!(g.binding(&[9u8; 32]).is_none(), "a file refused at its last line binds nothing from its first");

    // **a grant naming a resource nothing hosts is not expressible.**  It
    // was an error the line format could write down; nesting the grant
    // inside the package it grants on removes the case rather than
    // catching it.
    for (text, wrong) in [
        ("[[serve]]\nresource = \"x\"\n", "unknown field `serve`"),
        (&format!("[[host]]\nresource = \"{RES}\"\nowner = \"{WHO}\"\nauthority = \"a\"\n"), "missing field `manifest`"),
        (&format!("[[host]]\nresource = \"XX\"\nowner = \"{WHO}\"\nauthority = \"a\"\nmanifest = \"m\"\n"), "64 lower-case hex digits"),
        (&format!("{one}{one}"), "is hosted twice"),
    ] {
        let e = run(&d, text).expect_err("refused");
        assert!(e.contains(wrong), "expected {wrong:?}, got {e}");
    }
}
