//! The command line's entry point.

/// Subcommand dispatch is not assembled: the argument parser is the first
/// thing the milestone chooses, and choosing it here would fix a
/// dependency ahead of the work. The binary exists now to hold the name.
fn main() -> std::process::ExitCode {
    eprintln!("rhtn {}: no subcommands are assembled", env!("CARGO_PKG_VERSION"));
    std::process::ExitCode::FAILURE
}
