//! The instrument's entry point: one command a line, one or more lines out.

use rhtn_participant::{HELP, Instrument, Outcome};
use std::io::BufRead;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage: rhtnp <identity> [<peers>]

  <identity>  the two seeds an identity is derived from, owner-readable
              only, as `rhtn keys mint` writes one
  <peers>     hex KeyMaterial, one a line, for the parties this
              participant authenticates; defaults to `peers` beside the
              identity

Commands are read from standard input, one a line:

";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (identity, peers) = match args.as_slice() {
        [i] => (PathBuf::from(i), PathBuf::from(i).parent().unwrap_or(std::path::Path::new(".")).join("peers")),
        [i, p] => (PathBuf::from(i), PathBuf::from(p)),
        _ => {
            eprintln!("{USAGE}{HELP}");
            return ExitCode::from(2);
        }
    };
    let peers = peers.exists().then_some(peers);
    let instrument = match Instrument::start(&identity, peers.as_deref()) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("rhtnp: {e}");
            return ExitCode::FAILURE;
        }
    };
    // the first line names the participant, the way `rhtnd`'s names where
    // it is serving: a harness that started the process knows who it
    // started without being told beforehand
    println!("rhtnp: {}", instrument.me());
    for line in std::io::stdin().lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("rhtnp: {e}");
                return ExitCode::FAILURE;
            }
        };
        match instrument.run(&line) {
            Outcome::Said(lines) => {
                for l in lines {
                    println!("{l}");
                }
                // **every command ends with this**, whatever it printed,
                // so a harness reads until a terminator rather than
                // guessing how many lines an answer has
                println!("end");
            }
            Outcome::Done => return ExitCode::SUCCESS,
        }
    }
    ExitCode::SUCCESS
}
