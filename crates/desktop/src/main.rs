//! **The window** (`rhtn_desktop`): the participant client of a device
//! that holds no seed, and the operator's launch page as one of its
//! screens, each a web view of the system's rather than a browser shipped
//! with this binary.
//!
//! **What is here is only the window.** Every decision — what a page
//! says, what an act means, what is kept and where — is the library's,
//! tested with no desktop libraries present. This file carries the event
//! loop, and the one rule it keeps for itself: **neither the network nor
//! the kernel runs on the thread that draws**, so a pairing that waits on
//! a phone or a provider that takes five seconds to answer does not
//! freeze the window.
//!
//! **The pages are served from the window's own scheme** (`rhtn://`), the
//! current one at `index.html` and its stylesheet and faces beside it
//! (`participant::asset`); nothing the window shows is fetched from
//! elsewhere, except a provider's console, which is their page in a
//! window of ours and which nothing here reads.

use rhtn_cli::launch::TlsTransport;
use rhtn_desktop::custody::Keyring;
use rhtn_desktop::participant::{self, Held};
use rhtn_desktop::platform::{self, Desk};
use rhtn_desktop::{Act, act, code, directory, pairing, said, verify_and_keep};
use rhtn_ffi::client::Participant;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;
use wry::http::{Response, StatusCode};

/// The current page, as the scheme serves it.
const INDEX: &str = "rhtn://localhost/index.html";

/// What reaches the event loop from somewhere else: the page's messages,
/// and what a worker thread brings back.
enum Said {
    /// One message from our own page, as it sent it.
    FromPage(String),
    /// A line for the person, and whether it went through.
    ToPage(bool, String),
    /// A whole page to show in place of the current one.
    Show(String),
    /// The kernel's question for the person, and where the answer goes.
    Ask(String, Sender<bool>),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `rhtn-desktop [--at ADDRESS] [DIRECTORY]`: the local address a
    // pairing offers, where the one the system routes outward from is not
    // the network the phone is on; and the directory this desktop keeps
    // its state in, else the platform's own
    let (at, named) = match arguments(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("rhtn-desktop: {e}\nusage: rhtn-desktop [--at ADDRESS] [DIRECTORY]");
            std::process::exit(2);
        }
    };
    let dir = match directory(named.as_deref()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("rhtn-desktop: {e}");
            std::process::exit(2);
        }
    };

    let event_loop = EventLoopBuilder::<Said>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("FuerosNet")
        .with_inner_size(LogicalSize::new(820.0, 900.0))
        .build(&event_loop)?;

    let current = Arc::new(Mutex::new(participant::stopped_page("Starting…")));
    let served = current.clone();
    let to_loop = event_loop.create_proxy();
    let ours = WebViewBuilder::new()
        .with_custom_protocol("rhtn".into(), move |_, request| {
            let path = request.uri().path();
            let (mime, body): (&str, Vec<u8>) = if path == "/index.html" {
                ("text/html", served.lock().unwrap().clone().into_bytes())
            } else if let Some((mime, bytes)) = participant::asset(path) {
                (mime, bytes.to_vec())
            } else {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new().into())
                    .unwrap();
            };
            Response::builder()
                .header("Content-Type", mime)
                .body(body.into())
                .unwrap()
        })
        .with_url(INDEX)
        .with_ipc_handler(move |request| {
            let _ = to_loop.send_event(Said::FromPage(request.into_body()));
        })
        .build(&window)?;

    // the kernel runs off this thread, held here so it stays up
    let running: Arc<Mutex<Option<Arc<Participant>>>> = Arc::new(Mutex::new(None));
    {
        let (back, dir, running) = (event_loop.create_proxy(), dir.clone(), running.clone());
        std::thread::spawn(move || {
            let page = match start(&dir, at, &back, &running) {
                Ok(page) => page,
                Err(why) => participant::stopped_page(&why),
            };
            let _ = back.send_event(Said::Show(page));
        });
    }

    // the provider's own windows, held so that they stay open; a web view
    // dropped is a window gone
    let mut consoles: Vec<(tao::window::Window, wry::WebView)> = Vec::new();
    // the question the kernel is waiting on, one at a time
    let mut asking: Option<Sender<bool>> = None;
    let worker = event_loop.create_proxy();
    let tokens = Arc::new(dir);

    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
                ..
            } => {
                if window_id == window.id() {
                    // a question unanswered at close is a no
                    if let Some(a) = asking.take() {
                        let _ = a.send(false);
                    }
                    *control_flow = ControlFlow::Exit;
                } else {
                    // a provider's console closed: let it go, keep ours
                    consoles.retain(|(w, _)| w.id() != window_id);
                }
            }
            Event::UserEvent(Said::Show(page)) => {
                *current.lock().unwrap() = page;
                let _ = ours.load_url(INDEX);
            }
            Event::UserEvent(Said::Ask(question, answer)) => {
                if let Some(a) = asking.replace(answer) {
                    let _ = a.send(false);
                }
                let _ = ours.evaluate_script(&participant::ask_script(&question));
            }
            Event::UserEvent(Said::FromPage(message)) => match act(&message) {
                Err(why) => {
                    let _ = ours.evaluate_script(&said(false, &why));
                }
                Ok(Act::Answer { yes }) => {
                    if let Some(a) = asking.take() {
                        let _ = a.send(yes);
                    }
                }
                Ok(Act::Launch) => {
                    *current.lock().unwrap() = rhtn_desktop::page();
                    let _ = ours.load_url(INDEX);
                }
                Ok(Act::Console { provider }) => match open_console(target, &provider) {
                    Ok(open) => {
                        consoles.push(open);
                        let _ = ours.evaluate_script(&said(
                            true,
                            "The console is open in a window of its own. Make the token \
                                 there, then paste it here.",
                        ));
                    }
                    Err(e) => {
                        let _ = ours.evaluate_script(&said(false, &e.to_string()));
                    }
                },
                Ok(Act::Token { provider, token }) => {
                    // **off the drawing thread**: a TLS round trip to a
                    // provider is seconds, and the window must stay alive
                    // while it happens
                    let (back, dir) = (worker.clone(), tokens.clone());
                    std::thread::spawn(move || {
                        let out = TlsTransport::new()
                            .and_then(|t| verify_and_keep(&t, &dir, &provider, &token));
                        let _ = back.send_event(match out {
                            Ok(line) => Said::ToPage(true, line),
                            Err(why) => Said::ToPage(false, why),
                        });
                    });
                }
            },
            Event::UserEvent(Said::ToPage(ok, line)) => {
                let _ = ours.evaluate_script(&said(ok, &line));
            }
            _ => {}
        }
    })
}

/// **Bring the kernel up**: from what is kept where this desktop is paired
/// already, else by pairing it, showing the code and waiting for the
/// phone.  The page to show once it is up, or why it cannot be.
fn start(
    dir: &Path,
    at: Option<std::net::IpAddr>,
    back: &EventLoopProxy<Said>,
    running: &Mutex<Option<Arc<Participant>>>,
) -> Result<String, String> {
    platform::create_private(dir)?;
    let keyring = Arc::new(Keyring::new(dir));
    let asker = {
        let back = back.clone();
        Arc::new(move |question: &str| {
            let (tx, rx) = channel();
            back.send_event(Said::Ask(question.to_string(), tx)).is_ok()
                && rx.recv().unwrap_or(false)
        })
    };
    let teller = {
        let back = back.clone();
        Arc::new(move |told| {
            let _ = back.send_event(Said::ToPage(true, format!("{told:?}")));
        })
    };
    let desk = Desk::new(&state(dir), asker, teller)?;
    let platform = desk.platform(keyring.clone());

    let seed = keyring.seed()?;
    if let (Some(held), Some(seed)) = (Held::load(dir)?, seed.as_ref()) {
        let p = Participant::start_delegated(
            held.material.clone(),
            seed.to_vec(),
            held.run.clone(),
            vec![],
            platform,
        )
        .map_err(|e| custody_said(&keyring, e.to_string()))?;
        let page = participant::home_page(&keyhash(&p), participant::run_days(&held.run), None);
        *running.lock().unwrap() = Some(Arc::new(p));
        return Ok(page);
    }

    // **the seed is kept before the code is shown**: a keyring that will
    // not keep it stops the pairing before the phone signs anything, and
    // a seed kept for a pairing never finished is the one the next shows
    let seed = match seed {
        Some(s) => s,
        None => {
            let s = pairing::fresh_seed();
            keyring.keep_seed(&s)?;
            s
        }
    };
    let at = at.or_else(pairing::lan_address).ok_or(
        "this desktop is on no local network the phone could reach it on; name one with --at",
    )?;
    let offer = pairing::Offer::new(seed, at).map_err(|e| e.to_string())?;
    let _ = back.send_event(Said::Show(participant::pair_page(&code::svg(
        &offer.code(),
        320,
    )?)));
    let paired = offer
        .pair(vec![], platform)
        .map_err(|e| custody_said(&keyring, e))?;
    Held {
        material: paired.material.clone(),
        run: paired.run.clone(),
    }
    .keep(dir)?;
    let page = participant::home_page(
        &keyhash(&paired.participant),
        participant::run_days(&paired.run),
        Some(paired.dataset),
    );
    *running.lock().unwrap() = Some(paired.participant);
    Ok(page)
}

/// The command line: an address named with `--at`, and a directory.
fn arguments(
    mut args: impl Iterator<Item = String>,
) -> Result<(Option<std::net::IpAddr>, Option<String>), String> {
    let (mut at, mut dir) = (None, None);
    while let Some(a) = args.next() {
        if a == "--at" {
            let v = args.next().ok_or("--at names an address")?;
            at = Some(v.parse().map_err(|_| format!("{v} is not an address"))?);
        } else if dir.replace(a).is_some() {
            return Err("one directory at most".into());
        }
    }
    Ok((at, dir))
}

/// The kernel's own state, in a directory of its own beside what the
/// window keeps.
fn state(dir: &Path) -> PathBuf {
    dir.join("state")
}

fn keyhash(p: &Participant) -> Vec<u8> {
    p.me()
}

/// A refusal, with the keyring's reason where custody was what failed.
fn custody_said(keyring: &Keyring, why: String) -> String {
    match keyring.why() {
        Some(k) => format!("{why}\n{k}"),
        None => why,
    }
}

/// A provider's own console, in a window of its own.
fn open_console(
    target: &tao::event_loop::EventLoopWindowTarget<Said>,
    provider: &str,
) -> Result<(tao::window::Window, wry::WebView), Box<dyn std::error::Error>> {
    let url = rhtn_desktop::console_of(provider)
        .ok_or_else(|| format!("{provider} is not a provider this build drives"))?;
    let w = WindowBuilder::new()
        .with_title(format!("{provider} — the provider's own console"))
        .with_inner_size(LogicalSize::new(1100.0, 900.0))
        .build(target)?;
    let v = WebViewBuilder::new().with_url(url).build(&w)?;
    Ok((w, v))
}
