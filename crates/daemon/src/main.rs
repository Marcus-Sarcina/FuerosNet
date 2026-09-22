//! The daemon's entry point.

use rhtn_daemon::config::Config;
use rhtn_daemon::service::Service;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage: rhtnd <configuration> [<peers>]

  <configuration>  the operator's TOML configuration
  <peers>          hex KeyMaterial, one per line, for the peers this node
                   authenticates; defaults to `peers` beside the
                   configuration
";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cfg_path, peers_path) = match args.as_slice() {
        [c] => (
            PathBuf::from(c),
            PathBuf::from(c)
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("peers"),
        ),
        [c, p] => (PathBuf::from(c), PathBuf::from(p)),
        _ => {
            rhtn_daemon::say!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let cfg = match Config::read(&cfg_path) {
        Ok(c) => c,
        Err(e) => {
            rhtn_daemon::say!("rhtnd: {}: {e}", cfg_path.display());
            return ExitCode::FAILURE;
        }
    };
    let service = match Service::start(&cfg, &peers_path).await {
        Ok(s) => s,
        Err(e) => {
            rhtn_daemon::say!("rhtnd: {e}");
            return ExitCode::FAILURE;
        }
    };
    // the address it actually bound, which an operator needs when the
    // configuration named port 0, and a test needs to dial it
    rhtn_daemon::tell!("rhtnd: serving on {}", service.node.addr);
    // §8's disclosure, at the one moment an operator is certainly watching;
    // a stdout that closed between the two lines ends nothing here
    {
        use std::io::Write as _;
        let _ = write!(std::io::stdout(), "{}", service.exposure().render());
        let _ = std::io::stdout().flush();
    }
    match service.run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            rhtn_daemon::say!("rhtnd: writing state back: {e}");
            ExitCode::FAILURE
        }
    }
}
