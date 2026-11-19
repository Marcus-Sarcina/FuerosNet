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
use rhtn_desktop::{Act, act, code, directory, pairing, phrase, said, verify_and_keep};
use rhtn_ffi::client::Participant;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;
use wry::http::{Response, StatusCode};
use zeroize::Zeroizing;

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
    // where a phrase typed on the first start goes, while one is wanted
    let phrases: Arc<Mutex<Option<Sender<First>>>> = Arc::new(Mutex::new(None));
    {
        let (back, dir, running) = (event_loop.create_proxy(), dir.clone(), running.clone());
        let phrases = phrases.clone();
        std::thread::spawn(move || {
            let page = match start(&dir, at, &back, &running, &phrases) {
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
                Ok(Act::Restore { phrase }) => {
                    let phrase = Zeroizing::new(phrase);
                    let sent = match phrases.lock().unwrap().as_ref() {
                        Some(tx) => tx.send(First::Phrase(phrase)).is_ok(),
                        None => false,
                    };
                    if !sent {
                        let _ =
                            ours.evaluate_script(&said(false, "this desktop is set up already"));
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

/// **Bring the kernel up**: as it was brought up before, from what is
/// kept; else for the first time, by whichever of the two ways the person
/// takes (design §23.3) — the phone reading the code, or the phrase typed
/// here.  The page to show once it is up, or why it cannot be.
fn start(
    dir: &Path,
    at: Option<std::net::IpAddr>,
    back: &EventLoopProxy<Said>,
    running: &Mutex<Option<Arc<Participant>>>,
    phrases: &Mutex<Option<Sender<First>>>,
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

    match Held::load(dir)? {
        Some(Held::Paired { material, run }) => {
            let seed = keyring
                .seed()?
                .ok_or("this desktop was paired, and the keyring holds no transport seed for it")?;
            let p = Participant::start_delegated(
                material,
                seed.to_vec(),
                run.clone(),
                vec![],
                platform,
            )
            .map_err(|e| custody_said(&keyring, e.to_string()))?;
            let page = participant::home_page(&keyhash(&p), participant::run_days(&run), None);
            *running.lock().unwrap() = Some(Arc::new(p));
            return Ok(page);
        }
        Some(Held::Restored) => {
            let seeds = keyring
                .identity()?
                .ok_or("this desktop was restored, and the keyring holds no seeds for it")?;
            let p = Participant::start(seeds.to_vec(), vec![], platform)
                .map_err(|e| custody_said(&keyring, e.to_string()))?;
            let page = participant::restored_page(&keyhash(&p));
            *running.lock().unwrap() = Some(Arc::new(p));
            return Ok(page);
        }
        None => {}
    }

    // **the transport seed is kept before the code is shown**: a keyring
    // that will not keep it stops the pairing before the phone signs
    // anything, and a seed kept for a pairing never finished is the one
    // the next shows
    let seed = match keyring.seed()? {
        Some(s) => s,
        None => {
            let s = pairing::fresh_seed();
            keyring.keep_seed(&s)?;
            s
        }
    };
    // no local network is no code, and the phrase is still offered
    let offer = match at.or_else(pairing::lan_address) {
        Some(at) => Some(pairing::Offer::new(seed, at).map_err(|e| e.to_string())?),
        None => None,
    };
    let shown = match &offer {
        Some(o) => code::svg(&o.code(), 320)?,
        None => "<p class=\"sub\">This desktop is on no local network a phone could reach \
                 it on; name one with <code>--at</code>, or restore from the phrase.</p>"
            .to_string(),
    };
    let _ = back.send_event(Said::Show(participant::pair_page(&shown)));

    let (tx, rx) = channel();
    *phrases.lock().unwrap() = Some(tx.clone());
    let stop = Arc::new(AtomicBool::new(false));
    let mut waiting = offer.map(|offer| {
        let (platform, stop) = (platform.clone(), stop.clone());
        std::thread::spawn(move || {
            let _ = tx.send(First::Paired(offer.pair(vec![], platform, &stop)));
        })
    });
    loop {
        match rx.recv().map_err(|_| "the window closed".to_string())? {
            First::Paired(Ok(paired)) => {
                *phrases.lock().unwrap() = None;
                return finish_paired(dir, paired, running);
            }
            First::Paired(Err(why)) if why == pairing::STOPPED => {}
            First::Paired(Err(why)) => return Err(custody_said(&keyring, why)),
            First::Phrase(typed) => {
                let seeds = match phrase::seeds(&typed) {
                    Ok(s) => s,
                    Err(why) => {
                        let _ = back.send_event(Said::ToPage(false, why));
                        continue;
                    }
                };
                // the pairing ends first, so one kernel and only one stands
                // on this desktop's state; one that finished meanwhile wins
                stop.store(true, Ordering::Relaxed);
                if let Some(h) = waiting.take() {
                    let _ = h.join();
                }
                *phrases.lock().unwrap() = None;
                while let Ok(m) = rx.try_recv() {
                    if let First::Paired(Ok(paired)) = m {
                        return finish_paired(dir, paired, running);
                    }
                }
                keyring.keep_identity(&seeds)?;
                let p = Participant::start(seeds.to_vec(), vec![], platform)
                    .map_err(|e| custody_said(&keyring, e.to_string()))?;
                Held::Restored.keep(dir)?;
                let page = participant::restored_page(&keyhash(&p));
                *running.lock().unwrap() = Some(Arc::new(p));
                return Ok(page);
            }
        }
    }
}

/// What reaches a first start: the pairing's outcome, or a phrase typed.
enum First {
    Paired(Result<pairing::Paired, String>),
    Phrase(Zeroizing<String>),
}

/// A pairing finished: keep what starts it again, and the page to show.
fn finish_paired(
    dir: &Path,
    paired: pairing::Paired,
    running: &Mutex<Option<Arc<Participant>>>,
) -> Result<String, String> {
    Held::Paired {
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
