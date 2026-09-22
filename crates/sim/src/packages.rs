//! Packages to run in the sandbox, written as component text so a test
//! needs no wasm toolchain to build one.
//!
//! They live here rather than beside one crate's tests because two
//! different harnesses want them: `rhtn-resources` runs them directly, and
//! a daemon scenario writes one to disk for a process to admit.
//!
//! Each is the smallest component that makes one claim testable.  The
//! arena module exists because a lowered `list<u8>` needs a memory and a
//! `cabi_realloc` to land in, and the module that calls the lowered import
//! cannot also be the one that provides them — it would have to be
//! instantiated before itself.

/// A memory and a bump allocator, instantiated before anything that needs
/// them.  The allocator never frees: a component instance lives for one
/// request.
const ARENA: &str = r#"
  (core module $arena
    (memory (export "memory") 1)
    (global $next (mut i32) (i32.const 1024))
    (func (export "cabi_realloc") (param $old i32) (param $oldlen i32) (param $align i32) (param $new i32) (result i32)
      (local $p i32)
      (local.set $p (global.get $next))
      (global.set $next (i32.add (local.get $p) (local.get $new)))
      (local.get $p))
  )
  (core instance $arenai (instantiate $arena))
"#;

/// The two bindings the host offers, imported and lowered into core funcs.
const BINDINGS: &str = r#"
  (import "rhtn:host/bindings@1.0.0" (instance $b
    (export "request" (func (result (list u8))))
    (export "response" (func (param "body" (list u8))))
  ))
"#;

const LOWER: &str = r#"
  (core func $req (canon lower (func $b "request") (memory (core memory $arenai "memory")) (realloc (core func $arenai "cabi_realloc"))))
  (core func $resp (canon lower (func $b "response") (memory (core memory $arenai "memory")) (realloc (core func $arenai "cabi_realloc"))))
"#;

const WIRE: &str = r#"
  (core instance $maini (instantiate $main
    (with "host" (instance (export "request" (func $req)) (export "response" (func $resp))))
    (with "arena" (instance $arenai))))
  (func (export "handle") (canon lift (core func $maini "handle")))
"#;

const PREAMBLE: &str = r#"
    (import "host" "request" (func $request (param i32)))
    (import "host" "response" (func $response (param i32 i32)))
    (import "arena" "memory" (memory 1))
"#;

fn bound(body: &str) -> Vec<u8> {
    let src = format!(
        "(component {BINDINGS}{ARENA}{LOWER}\n  (core module $main{PREAMBLE}{body}\n  )\n{WIRE}\n)"
    );
    wat::parse_str(&src).expect("a component")
}

/// Hands back exactly the message it was given.
pub fn echo() -> Vec<u8> {
    bound(
        r#"
    (func (export "handle")
      (call $request (i32.const 0))
      (call $response (i32.load (i32.const 0)) (i32.load (i32.const 4))))
"#,
    )
}

/// Grows until the host stops it, then reports how many pages it reached.
pub fn greedy() -> Vec<u8> {
    bound(
        r#"
    (func (export "handle")
      (block $done
        (loop $again
          (br_if $done (i32.eq (memory.grow (i32.const 1)) (i32.const -1)))
          (br $again)))
      (i32.store (i32.const 0) (memory.size))
      (call $response (i32.const 0) (i32.const 4)))
"#,
    )
}

/// Answers with more than the host will carry.
pub fn shouting(bytes: u32) -> Vec<u8> {
    bound(&format!(
        r#"
    (func (export "handle")
      (block $done
        (loop $again
          (br_if $done (i32.ge_u (i32.mul (memory.size) (i32.const 65536)) (i32.const {bytes})))
          (br_if $done (i32.eq (memory.grow (i32.const 1)) (i32.const -1)))
          (br $again)))
      (call $response (i32.const 0) (i32.const {bytes})))
"#
    ))
}

/// Never returns.
pub fn spinning() -> Vec<u8> {
    bound(
        r#"
    (func (export "handle")
      (loop $again (br $again)))
"#,
    )
}

/// Breaks.
pub fn broken() -> Vec<u8> {
    bound(
        r#"
    (func (export "handle") (unreachable))
"#,
    )
}

/// Imports nothing at all and answers nothing.  Admitting it is the
/// control: the refusals below are about what a package asked for, not
/// about the sandbox refusing everything.
pub fn silent() -> Vec<u8> {
    wat::parse_str(
        r#"(component
  (core module $main (func (export "handle")))
  (core instance $maini (instantiate $main))
  (func (export "handle") (canon lift (core func $maini "handle")))
)"#,
    )
    .expect("a component")
}

/// Asks for a binding from somewhere else entirely.
pub fn reaching(instance: &str) -> Vec<u8> {
    wat::parse_str(
        format!(
            r#"(component
  (import "{instance}" (instance $x (export "get" (func (result u32)))))
  (core module $main (func (export "handle")))
  (core instance $maini (instantiate $main))
  (func (export "handle") (canon lift (core func $maini "handle")))
)"#
        )
        .as_str(),
    )
    .expect("a component")
}

/// Asks the host's own instance for a hook it does not have.
pub fn overreaching(hook: &str) -> Vec<u8> {
    wat::parse_str(
        format!(
            r#"(component
  (import "rhtn:host/bindings@1.0.0" (instance $b
    (export "request" (func (result (list u8))))
    (export "{hook}" (func (result u32)))
  ))
  (core module $main (func (export "handle")))
  (core instance $maini (instantiate $main))
  (func (export "handle") (canon lift (core func $maini "handle")))
)"#
        )
        .as_str(),
    )
    .expect("a component")
}

/// Exports something, but not the entry the host calls.
pub fn mute() -> Vec<u8> {
    wat::parse_str(
        r#"(component
  (core module $main (func (export "other")))
  (core instance $maini (instantiate $main))
  (func (export "other") (canon lift (core func $maini "other")))
)"#,
    )
    .expect("a component")
}
