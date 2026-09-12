//! The daemon's entry point.

use rhtn_daemon::config::Config;
use rhtn_daemon::service::Service;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage: rhtnd <configuration> [<peers>]

  <configuration>  the operator's `key = value` file
  <peers>          hex KeyMaterial, one per line, for the peers this node
                   authenticates; defaults to `peers` beside the
                   configuration
";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cfg_path, peers_path) = match args.as_slice() {
        [c] => (PathBuf::from(c), PathBuf::from(c).parent().unwrap_or(std::path::Path::new(".")).join("peers")),
        [c, p] => (PathBuf::from(c), PathBuf::from(p)),
        _ => {
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let cfg = match Config::read(&cfg_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("rhtnd: {}: {e}", cfg_path.display());
            return ExitCode::FAILURE;
        }
    };
    let service = match Service::start(&cfg, &peers_path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rhtnd: {e}");
            return ExitCode::FAILURE;
        }
    };
    // the address it actually bound, which an operator needs when the
    // configuration named port 0, and a test needs to dial it
    println!("rhtnd: serving on {}", service.node.addr);
    match service.run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rhtnd: writing state back: {e}");
            ExitCode::FAILURE
        }
    }
}
