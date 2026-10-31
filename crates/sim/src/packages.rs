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

/// **Answers with a response of its own**, whose body is the request it
/// was handed.
///
/// [`echo`] hands the request straight back, which is what the sandbox's
/// own test wants — it is asserting that a package sees exactly the
/// message the node built. But a request is not a response, and since
/// 2026-10-28 the gateway reads what a package answers with
/// (`wire-format.md` §11 field 2): `echo` is now the fixture for *a
/// package whose answer cannot be read*, and this is the one for a
/// package that answers. The request is still in the body, so a test can
/// still read what crossed the boundary.
///
/// Neither a length nor an encoding is declared: the host hands the whole
/// answer over as one buffer, so its end is the message's end, and the
/// node writes the length on the way out.
pub fn replies() -> Vec<u8> {
    // the prefix at a high address: the arena bump-allocates upward from
    // 1024, so a test-sized request never reaches it
    bound(
        r#"
    (data (i32.const 40000) "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\n\r\n")
    (func (export "handle")
      (call $request (i32.const 0))
      (memory.copy (i32.const 40045) (i32.load (i32.const 0)) (i32.load (i32.const 4)))
      (call $response (i32.const 40000) (i32.add (i32.const 45) (i32.load (i32.const 4)))))
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

/// The host instance with the block, the log and the clock beside the
/// two bindings every package has, lowered for a module that keeps notes.
const BINDINGS_FULL: &str = r#"
  (import "rhtn:host/bindings@1.0.0" (instance $b
    (export "request" (func (result (list u8))))
    (export "response" (func (param "body" (list u8))))
    (export "block-write" (func (param "name" string) (param "offset" u64) (param "bytes" (list u8)) (result u64)))
    (export "block-read" (func (param "name" string) (param "offset" u64) (param "len" u32) (result (list u8))))
    (export "log" (func (param "line" string)))
    (export "clock" (func (result u64)))
    (export "fetch" (func (param "request" (list u8)) (result (list u8))))
  ))
"#;

const LOWER_FULL: &str = r#"
  (core func $req (canon lower (func $b "request") (memory (core memory $arenai "memory")) (realloc (core func $arenai "cabi_realloc"))))
  (core func $resp (canon lower (func $b "response") (memory (core memory $arenai "memory")) (realloc (core func $arenai "cabi_realloc"))))
  (core func $bw (canon lower (func $b "block-write") (memory (core memory $arenai "memory")) (realloc (core func $arenai "cabi_realloc"))))
  (core func $br (canon lower (func $b "block-read") (memory (core memory $arenai "memory")) (realloc (core func $arenai "cabi_realloc"))))
  (core func $log (canon lower (func $b "log") (memory (core memory $arenai "memory")) (realloc (core func $arenai "cabi_realloc"))))
  (core func $clk (canon lower (func $b "clock")))
  (core func $fetch (canon lower (func $b "fetch") (memory (core memory $arenai "memory")) (realloc (core func $arenai "cabi_realloc"))))
"#;

const WIRE_FULL: &str = r#"
  (core instance $maini (instantiate $main
    (with "host" (instance
      (export "request" (func $req)) (export "response" (func $resp))
      (export "block-write" (func $bw)) (export "block-read" (func $br))
      (export "log" (func $log)) (export "clock" (func $clk)) (export "fetch" (func $fetch))))
    (with "arena" (instance $arenai))))
  (func (export "handle") (canon lift (core func $maini "handle")))
"#;

const PREAMBLE_FULL: &str = r#"
    (import "host" "request" (func $request (param i32)))
    (import "host" "response" (func $response (param i32 i32)))
    (import "host" "block-write" (func $bwrite (param i32 i32 i64 i32 i32) (result i64)))
    (import "host" "block-read" (func $bread (param i32 i32 i64 i32 i32)))
    (import "host" "log" (func $log (param i32 i32)))
    (import "host" "clock" (func $clock (result i64)))
    (import "host" "fetch" (func $fetch (param i32 i32 i32)))
    (import "arena" "memory" (memory 1))
"#;

fn bound_full(body: &str) -> Vec<u8> {
    let src = format!(
        "(component {BINDINGS_FULL}{ARENA}{LOWER_FULL}\n  (core module $main{PREAMBLE_FULL}{body}\n  )\n{WIRE_FULL}\n)"
    );
    wat::parse_str(&src).expect("a component")
}

/// **Keeps what it was handed in its block, logs that it did, and answers
/// with what it reads back**: the request body is written to the file
/// `note` at offset 0, read back, and returned, with the host's clock in
/// the log line.  The round trip through the block is the claim.
pub fn noting() -> Vec<u8> {
    bound_full(
        r#"
    (data (i32.const 2000) "note")
    (data (i32.const 2010) "kept a note")
    (func (export "handle")
      (call $request (i32.const 0))
      (drop (call $bwrite (i32.const 2000) (i32.const 4) (i64.const 0) (i32.load (i32.const 0)) (i32.load (i32.const 4))))
      (call $bread (i32.const 2000) (i32.const 4) (i64.const 0) (i32.load (i32.const 4)) (i32.const 8))
      (drop (call $clock))
      (call $log (i32.const 2010) (i32.const 11))
      (call $response (i32.load (i32.const 8)) (i32.load (i32.const 12))))
"#,
    )
}

/// Writes `bytes` of its request into the block at offset `at`, which is
/// how a test takes a block past its quota.
pub fn hoarding(at: u64) -> Vec<u8> {
    bound_full(&format!(
        r#"
    (data (i32.const 2000) "hoard")
    (func (export "handle")
      (call $request (i32.const 0))
      (drop (call $bwrite (i32.const 2000) (i32.const 5) (i64.const {at}) (i32.load (i32.const 0)) (i32.load (i32.const 4))))
      (call $response (i32.const 0) (i32.const 0)))
"#
    ))
}

/// **Hands its request onward as an outbound one and answers with what
/// came back**: the request body is the HTTP/1.1 request the package
/// sends through the host's fetch, and the host's reply is the answer.
pub fn fetching() -> Vec<u8> {
    bound_full(
        r#"
    (func (export "handle")
      (call $request (i32.const 0))
      (call $fetch (i32.load (i32.const 0)) (i32.load (i32.const 4)) (i32.const 8))
      (call $response (i32.load (i32.const 8)) (i32.load (i32.const 12))))
"#,
    )
}
