//! **What the operator window would reach, without the window**: the
//! tunnel `operate::Tunnel` opens, the declaration it reads, and the
//! instance's own page, against a live instance.
//!
//! ```text
//! cargo run -p rhtn-desktop --example operate_check -- <machine> <control key file>
//! ```
//!
//! The key file is the private half of the host control key, in the form
//! `ssh -i` reads; it is read and not kept. Run by hand against a bench
//! instance, not by the gate, which reaches no machine.

use rhtn_desktop::operate;
use std::time::Duration;
use zeroize::Zeroizing;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (Some(machine), Some(key)) = (args.next(), args.next()) else {
        return Err("operate_check <machine> <control key file>".into());
    };
    let key = Zeroizing::new(std::fs::read_to_string(&key).map_err(|e| format!("{key}: {e}"))?);
    let private = std::env::temp_dir().join(format!("rhtn-operate-check-{}", std::process::id()));
    let tunnel = operate::Tunnel::open(&machine, &key, &private)?;
    drop(key);
    println!(
        "tunnel  127.0.0.1:{} -> {machine}:127.0.0.1:{}",
        tunnel.port,
        operate::SURFACE_PORT
    );
    let (code, body) = tunnel.exchange("GET", "/node/acts", None, b"", Duration::from_secs(15))?;
    println!("acts    {code}, {} bytes", body.len());
    let d = operate::read(&String::from_utf8_lossy(&body))?;
    println!("surface {} (understood: {})", d.surface, d.understood());
    let names = |acts: &[operate::Act]| {
        acts.iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    println!("node    {}", names(&d.node));
    for h in &d.resources {
        println!(
            "res     {} {} installed={} :: {}",
            &h.resource[..16],
            h.authority,
            h.installed,
            names(&h.acts)
        );
    }
    let undrawn: Vec<String> = d
        .node
        .iter()
        .chain(d.resources.iter().flat_map(|h| h.acts.iter()))
        .filter_map(|a| operate::undrawable(a).map(|w| format!("{}: {w}", a.name)))
        .collect();
    println!(
        "undrawn {}",
        if undrawn.is_empty() {
            "none".into()
        } else {
            undrawn.join("; ")
        }
    );
    let page = operate::page("check", &d);
    println!(
        "drawn   {} forms, {} bytes",
        page.matches("<form class=\"act\"").count(),
        page.len()
    );
    let (code, own) = tunnel.exchange("GET", "/", None, b"", Duration::from_secs(15))?;
    let own = String::from_utf8_lossy(&own);
    let bounces = own.matches("href=\"rhtn-act:").count();
    println!("own     {code}, {} bytes, {bounces} act links", own.len());
    if let Some(at) = own.find("href=\"rhtn-act:") {
        let link: String = own[at + 6..].chars().take_while(|c| *c != '"').collect();
        let link = link.replace("&amp;", "&");
        match operate::bounced(&link) {
            Some((name, query)) => {
                let act = d.act(&name, None);
                println!(
                    "bounce  {name}: declared={} hints={:?}",
                    act.is_some(),
                    act.map(|a| operate::hints(a, &query)
                        .into_iter()
                        .map(|(k, _)| k)
                        .collect::<Vec<_>>())
                );
            }
            None => println!("bounce  {link} not read"),
        }
    }
    // **one act sent as the window sends it**, where a runner key is
    // given: `running`, which changes nothing and brings the key
    if let Some(runner) = args.next() {
        let runner =
            Zeroizing::new(std::fs::read_to_string(&runner).map_err(|e| format!("{runner}: {e}"))?);
        let act = d.act("running", None).ok_or("no `running` declared")?;
        let mut fields = operate::body(act, &[])?;
        fields.push(("key".into(), runner.to_string()));
        let sent = operate::form(&fields);
        let (code, said) = tunnel.exchange(
            "POST",
            "/node/act/running",
            Some("application/x-www-form-urlencoded"),
            &sent,
            Duration::from_secs(60),
        )?;
        let said = String::from_utf8_lossy(&said);
        println!(
            "running {code}, first line: {}",
            said.lines().next().unwrap_or("")
        );
    }
    let _ = std::fs::remove_dir_all(&private);
    Ok(())
}
