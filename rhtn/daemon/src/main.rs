//! The daemon's entry point.

/// The lifecycle is not assembled: the daemon will take a configuration
/// path, run a node until it is signalled, and persist what it accepted.
/// The binary exists now to fix the shape and to hold the name every
/// operator-facing document uses.
fn main() -> std::process::ExitCode {
    eprintln!("rhtnd {}: the service lifecycle is not assembled", env!("CARGO_PKG_VERSION"));
    std::process::ExitCode::FAILURE
}
