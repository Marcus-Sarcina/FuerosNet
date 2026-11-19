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
use rhtn_desktop::participant::{self, Device, Held};
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
    let Args {
        at,
        dir: named,
        phrase: given,
    } = match arguments(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("rhtn-desktop: {e}\nusage: rhtn-desktop {USAGE}");
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
    let builder = WebViewBuilder::new()
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
        });
    let ours = web_view(builder, &window)?;

    // the kernel runs off this thread, held here so it stays up
    let running: Arc<Mutex<Option<Arc<Participant>>>> = Arc::new(Mutex::new(None));
    // where a phrase typed on the first start goes, while one is wanted
    let phrases: Arc<Mutex<Option<Sender<First>>>> = Arc::new(Mutex::new(None));
    // what this desktop is to its identity, once it is up: the home page
    // is drawn from it and from the kernel, so it says where things stand
    // whenever it is come back to
    let device: Arc<Mutex<Option<Device>>> = Arc::new(Mutex::new(None));
    {
        let (back, dir, running) = (event_loop.create_proxy(), dir.clone(), running.clone());
        let (phrases, device) = (phrases.clone(), device.clone());
        std::thread::spawn(
            move || match start(&dir, at, given, &back, &running, &phrases) {
                Ok(d) => {
                    *device.lock().unwrap() = Some(d);
                    show_home(&back, &running, &device);
                    // **the nodes agreed to are attached at every start**, as
                    // the phone's are: the first that answers
                    let p = running.lock().unwrap().clone();
                    if let Some(p) = p.filter(|p| !p.agreed().is_empty()) {
                        reattach(&p);
                        show_home(&back, &running, &device);
                    }
                }
                Err(why) => {
                    let _ = back.send_event(Said::Show(participant::stopped_page(&why)));
                }
            },
        );
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
                    // the session closed cleanly, so the node queues for
                    // this desktop from now and not after missed intervals
                    if let Some(p) = running.lock().unwrap().as_ref() {
                        let _ = p.detach();
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
                Ok(Act::Home) => {
                    let (back, running, device) = (worker.clone(), running.clone(), device.clone());
                    std::thread::spawn(move || show_home(&back, &running, &device));
                }
                Ok(Act::Attach { node, address }) => {
                    let p = running.lock().unwrap().clone();
                    let Some(p) = p else {
                        let _ =
                            ours.evaluate_script(&said(false, "this desktop is not set up yet"));
                        return;
                    };
                    // **off the drawing thread**: an attach dials and waits
                    let (back, running, device) = (worker.clone(), running.clone(), device.clone());
                    std::thread::spawn(move || match serve_from(&p, node, address) {
                        Ok(()) => show_home(&back, &running, &device),
                        Err(why) => {
                            let _ = back.send_event(Said::ToPage(false, why));
                        }
                    });
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
    given: Option<Zeroizing<String>>,
    back: &EventLoopProxy<Said>,
    running: &Mutex<Option<Arc<Participant>>>,
    phrases: &Mutex<Option<Sender<First>>>,
) -> Result<Device, String> {
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

    let held = Held::load(dir)?;
    if held.is_some() && given.is_some() {
        return Err(
            "this desktop is set up already, and a phrase given is not taken over \
                    it: start it on another directory"
                .into(),
        );
    }
    match held {
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
            *running.lock().unwrap() = Some(Arc::new(p));
            return Ok(Device::Paired {
                days: participant::run_days(&run),
                dataset: None,
            });
        }
        Some(Held::Restored) => {
            let seeds = keyring
                .identity()?
                .ok_or("this desktop was restored, and the keyring holds no seeds for it")?;
            let p = Participant::start(seeds.to_vec(), vec![], platform)
                .map_err(|e| custody_said(&keyring, e.to_string()))?;
            *running.lock().unwrap() = Some(Arc::new(p));
            return Ok(Device::Restored);
        }
        None => {}
    }

    // **a phrase given on the command line restores at once** (a
    // field-test build's bench), with no code shown and nothing typed
    if let Some(typed) = given {
        let seeds = phrase::seeds(&typed)?;
        return restore(dir, &keyring, platform, &seeds, running);
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
                return restore(dir, &keyring, platform, &seeds, running);
            }
        }
    }
}

/// What reaches a first start: the pairing's outcome, or a phrase typed.
enum First {
    Paired(Result<pairing::Paired, String>),
    Phrase(Zeroizing<String>),
}

/// **Restore from the identity's seeds**: kept by the keyring first, so a
/// keyring that will not keep them stops it before the kernel stands on
/// them, then the kernel, then the note that says how to start again.
fn restore(
    dir: &Path,
    keyring: &Keyring,
    platform: Arc<rhtn_ffi::device::Platform>,
    seeds: &[u8],
    running: &Mutex<Option<Arc<Participant>>>,
) -> Result<Device, String> {
    keyring.keep_identity(seeds)?;
    let p = Participant::start(seeds.to_vec(), vec![], platform)
        .map_err(|e| custody_said(keyring, e.to_string()))?;
    Held::Restored.keep(dir)?;
    *running.lock().unwrap() = Some(Arc::new(p));
    Ok(Device::Restored)
}

/// A pairing finished: keep what starts it again, and say what it made.
fn finish_paired(
    dir: &Path,
    paired: pairing::Paired,
    running: &Mutex<Option<Arc<Participant>>>,
) -> Result<Device, String> {
    Held::Paired {
        material: paired.material.clone(),
        run: paired.run.clone(),
    }
    .keep(dir)?;
    *running.lock().unwrap() = Some(paired.participant);
    Ok(Device::Paired {
        days: participant::run_days(&paired.run),
        dataset: Some(paired.dataset),
    })
}

/// **The home page as things stand**, sent to the window.  Off the
/// drawing thread: asking the kernel where it is connected waits on it.
fn show_home(
    back: &EventLoopProxy<Said>,
    running: &Mutex<Option<Arc<Participant>>>,
    device: &Mutex<Option<Device>>,
) {
    let p = running.lock().unwrap().clone();
    let d = device.lock().unwrap().clone();
    if let (Some(p), Some(d)) = (p, d) {
        let agreed: Vec<(Vec<u8>, Vec<String>)> = p
            .agreed()
            .into_iter()
            .map(|a| (a.serving, a.addresses))
            .collect();
        let line = participant::connection(&p.attachments(), &agreed);
        let _ = back.send_event(Said::Show(participant::home_page(&keyhash(&p), &d, &line)));
    }
}

/// **Connect to a node already running**, as the phone's attach screen
/// does: the agreement kept by the kernel first, so the node is attached
/// again at every start, then the attach.  The tap that sent this is the
/// choosing of the node; nothing attaches to one unchosen.
fn serve_from(p: &Participant, node: Vec<u8>, address: String) -> Result<(), String> {
    p.agree_to(node.clone(), vec![address.clone()])
        .map_err(|e| format!("the node could not be remembered: {e}"))?;
    attach_to(p, node, vec![address])
}

/// Attach to `node`, sweeping the people this identity knows rather than
/// the infrastructure.
fn attach_to(p: &Participant, node: Vec<u8>, addresses: Vec<String>) -> Result<(), String> {
    let population = p
        .contacts()
        .into_iter()
        .filter(|c| c.how != rhtn_model::How::Infra)
        .map(|c| c.keyhash.to_vec())
        .collect();
    p.attach(node, addresses, population)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The nodes agreed to, in the kernel's order, until one answers.
fn reattach(p: &Participant) {
    for a in p.agreed() {
        if attach_to(p, a.serving, a.addresses).is_ok() {
            return;
        }
    }
}

/// How the command line is used, in this build.
#[cfg(not(feature = "fieldtest"))]
const USAGE: &str = "[--at ADDRESS] [DIRECTORY]";
#[cfg(feature = "fieldtest")]
const USAGE: &str = "[--at ADDRESS] [--phrase-from FILE [--phrase-for NAME]] [DIRECTORY]";

/// What the command line says.
#[derive(Default)]
struct Args {
    /// The local address a pairing offers.
    at: Option<std::net::IpAddr>,
    /// The directory this desktop keeps its state in.
    dir: Option<String>,
    /// **A phrase to restore from, read from a file** — a field-test
    /// build's alone, for the bench's test identities; a releasable build
    /// does not know the flag.
    phrase: Option<Zeroizing<String>>,
}

/// The command line: an address named with `--at`, a directory, and in a
/// field-test build a phrase file with `--phrase-from` and which of its
/// entries with `--phrase-for`.
fn arguments(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut out = Args::default();
    #[cfg(feature = "fieldtest")]
    let (mut from, mut entry): (Option<String>, Option<String>) = (None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--at" => {
                let v = args.next().ok_or("--at names an address")?;
                out.at = Some(v.parse().map_err(|_| format!("{v} is not an address"))?);
            }
            #[cfg(feature = "fieldtest")]
            "--phrase-from" => from = Some(args.next().ok_or("--phrase-from names a file")?),
            #[cfg(feature = "fieldtest")]
            "--phrase-for" => entry = Some(args.next().ok_or("--phrase-for names an entry")?),
            flag if flag.starts_with("--") => return Err(format!("{flag} is not a flag")),
            _ => {
                if out.dir.replace(a).is_some() {
                    return Err("one directory at most".into());
                }
            }
        }
    }
    #[cfg(feature = "fieldtest")]
    match (from, entry) {
        (Some(file), entry) => {
            let text =
                Zeroizing::new(std::fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?);
            out.phrase = Some(phrase::from_file(&text, entry.as_deref())?);
        }
        (None, Some(_)) => return Err("--phrase-for picks from a --phrase-from file".into()),
        (None, None) => {}
    }
    Ok(out)
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

/// **A web view in `window`.**  On Linux it goes into the GTK box tao
/// gives every window, as wry's own example does: built as a bare X11 child
/// it drew nothing on this bench (a Quadro under nouveau, WebKitGTK 2.52),
/// and the box serves Wayland as well.
fn web_view(builder: WebViewBuilder, window: &tao::window::Window) -> wry::Result<wry::WebView> {
    #[cfg(target_os = "linux")]
    {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().expect("tao gives every window a box");
        builder.build_gtk(vbox)
    }
    #[cfg(not(target_os = "linux"))]
    builder.build(window)
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
    let v = web_view(WebViewBuilder::new().with_url(url), &w)?;
    Ok((w, v))
}
