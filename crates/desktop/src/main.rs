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
use rhtn_desktop::participant::{self, Device, Held, hex};
use rhtn_desktop::platform::{self, Desk};
use rhtn_desktop::screens::{
    self, Body, Crown, History, LineRow, Person, Place, ResourceRow, Snapshot, SubnetRow,
};
use rhtn_desktop::threads::{self, Delivery, Record, Sealer, Threads};
use rhtn_desktop::{Act, act, code, directory, frame, pairing, phrase, said, verify_and_keep};
use rhtn_ffi::client::{PageShape, Participant, ResourceAnswer, UtilityOutcome};
use rhtn_ffi::net::Event as Kernel;
use rhtn_ffi::types::kind_application;
use std::collections::HashMap;
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
    /// A place's rail and body redrawn where they stand, for the place
    /// they were drawn for: dropped where the window has gone elsewhere.
    Redraw(Place, String),
    /// The kernel's question for the person, and where the answer goes.
    Ask(String, Sender<bool>),
    /// A resource's first answer, to be framed in a window of its own.
    Frame(Framed),
    /// What a resource answered with that this desktop does not show, the
    /// person having said to keep it: a save dialog for it.
    Save(Offered),
}

/// **An answer offered for saving**: the name to suggest and the bytes.
struct Offered {
    suggested: String,
    body: Vec<u8>,
}

/// **What opening a resource came to**, for the window to frame.
struct Framed {
    node: Vec<u8>,
    resource: Vec<u8>,
    name: String,
    /// The answer to `GET /`, as the engine is to be given it.
    first: frame::Served,
    /// Whether the person allowed this resource's code.
    allowed: bool,
}

/// **One resource's window**: the window, its web view, and the web
/// context its storage lives in, held together since dropping any of them
/// is the window gone.
struct FrameWindow {
    resource: Vec<u8>,
    window: tao::window::Window,
    _view: wry::WebView,
    _context: wry::WebContext,
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

    // the kernel and what the screens are drawn from, shared with the
    // workers that ask it, since none of that happens on this thread
    let up = Arc::new(Up {
        running: Mutex::new(None),
        device: Mutex::new(None),
        place: Mutex::new(History::default()),
        threads: Mutex::new(None),
        closing: AtomicBool::new(false),
        dir: dir.clone(),
    });
    // where a phrase typed on the first start goes, while one is wanted
    let phrases: Arc<Mutex<Option<Sender<First>>>> = Arc::new(Mutex::new(None));
    {
        let (back, dir, up) = (event_loop.create_proxy(), dir.clone(), up.clone());
        let phrases = phrases.clone();
        std::thread::spawn(
            move || match start(&dir, at, given, &back, &up.running, &phrases) {
                Ok(d) => {
                    let Some(p) = up.running.lock().unwrap().clone() else {
                        return;
                    };
                    // **the one line that carries the wall clock**, so a
                    // field-test build's diagnostics are placed beside the
                    // phones'
                    tracing::info!(
                        target: "diag",
                        unix_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |d| d.as_millis() as u64),
                        pid = std::process::id(),
                        process = "rhtn-desktop",
                        "diag.anchor"
                    );
                    *up.device.lock().unwrap() = Some(d);
                    *up.threads.lock().unwrap() = Some(Arc::new(Threads::new(
                        &dir,
                        &hex(&p.me()),
                        Box::new(KernelSealer(p.clone())),
                    )));
                    draw(&up, &back, true);
                    {
                        let (up, back) = (up.clone(), back.clone());
                        std::thread::spawn(move || pump(&up, &back, &p));
                    }
                    // **the nodes agreed to are attached at every start**, as
                    // the phone's are: the first that answers
                    let p = up.running.lock().unwrap().clone();
                    if let Some(p) = p.filter(|p| !p.agreed().is_empty()) {
                        reattach(&p);
                        draw(&up, &back, false);
                        let _ = sweep(&up);
                        draw(&up, &back, false);
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
    // every resource's window, one per resource
    let mut frames: Vec<FrameWindow> = Vec::new();
    let worker = event_loop.create_proxy();
    let root = dir.clone();
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
                    up.closing.store(true, Ordering::Relaxed);
                    if let Some(p) = up.running.lock().unwrap().as_ref() {
                        let _ = p.detach();
                        // **written before it goes**: the shell's point
                        // for it, as the kernel's `save` asks
                        let _ = p.save();
                    }
                    *control_flow = ControlFlow::Exit;
                } else {
                    // a provider's console or a resource's window closed:
                    // let it go, keep ours
                    consoles.retain(|(w, _)| w.id() != window_id);
                    frames.retain(|f| f.window.id() != window_id);
                }
            }
            Event::UserEvent(Said::Show(page)) => {
                *current.lock().unwrap() = page;
                let _ = ours.load_url(INDEX);
            }
            Event::UserEvent(Said::Redraw(place, script)) => {
                if *up.place.lock().unwrap().here() == place {
                    let _ = ours.evaluate_script(&script);
                }
            }
            Event::UserEvent(Said::Frame(framed)) => {
                let Some(p) = up.running.lock().unwrap().clone() else {
                    return;
                };
                match open_frame(target, &root, framed, p, worker.clone()) {
                    Ok(f) => frames.push(f),
                    Err(e) => {
                        let _ = ours.evaluate_script(&said(false, &e.to_string()));
                    }
                }
            }
            Event::UserEvent(Said::Save(offered)) => {
                let line = match save(offered) {
                    Ok(Some(path)) => (true, format!("saved to {}", path.display())),
                    Ok(None) => (true, "not saved".to_string()),
                    Err(why) => (false, format!("not saved: {why}")),
                };
                let _ = ours.evaluate_script(&said(line.0, &line.1));
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
                Ok(Act::Go { place }) => {
                    up.place.lock().unwrap().go(place.clone());
                    let (back, up) = (worker.clone(), up.clone());
                    std::thread::spawn(move || {
                        draw(&up, &back, true);
                        if let Place::Subnet(id) = place {
                            opened(&up, &back, &id);
                        }
                    });
                }
                Ok(Act::Cost { patron }) => {
                    let (back, up) = (worker.clone(), up.clone());
                    std::thread::spawn(move || cost(&up, &back, patron));
                }
                // **leaving is the person's right, and signed**: a paired
                // desktop is never offered it, and the kernel refuses a
                // departure from a key it holds no seed for in any case
                Ok(Act::Leave { patron }) => {
                    let (back, up) = (worker.clone(), up.clone());
                    std::thread::spawn(move || {
                        let Some(p) = up.running.lock().unwrap().clone() else {
                            return;
                        };
                        let said = match p.depart(patron, None) {
                            Ok(_) => (true, "Left. The line is ended.".to_string()),
                            Err(why) => (false, format!("not left: {why}")),
                        };
                        let _ = back.send_event(Said::ToPage(said.0, said.1));
                        draw(&up, &back, true);
                    });
                }
                Ok(Act::Rename { subnet, name }) => {
                    let (back, up) = (worker.clone(), up.clone());
                    std::thread::spawn(move || {
                        let Some(p) = up.running.lock().unwrap().clone() else {
                            return;
                        };
                        let said = match p.publish_profile(subnet, 1, name.clone(), None, None) {
                            Ok(()) => (true, format!("Named {name}; published to your node.")),
                            Err(why) => (false, format!("not published: {why}")),
                        };
                        let _ = back.send_event(Said::ToPage(said.0, said.1));
                        draw(&up, &back, false);
                    });
                }
                Ok(Act::Colour { name }) => {
                    let (back, up) = (worker.clone(), up.clone());
                    std::thread::spawn(move || {
                        let me = up.running.lock().unwrap().as_ref().map(|p| p.me());
                        if let Some(me) = me
                            && let Err(why) = participant::keep_colour(&up.dir, &me, &name)
                        {
                            let _ =
                                back.send_event(Said::ToPage(false, format!("not kept: {why}")));
                        }
                        up.place.lock().unwrap().back();
                        draw(&up, &back, true);
                    });
                }
                Ok(Act::Back) => {
                    up.place.lock().unwrap().back();
                    let (back, up) = (worker.clone(), up.clone());
                    std::thread::spawn(move || draw(&up, &back, true));
                }
                Ok(Act::Backup { bytes }) => {
                    let (back, up) = (worker.clone(), up.clone());
                    std::thread::spawn(move || import(&up, &back, bytes));
                }
                Ok(Act::Open {
                    node,
                    resource,
                    name,
                }) => {
                    // **one window per resource**: one already open is
                    // brought forward rather than opened twice
                    if let Some(f) = frames.iter().find(|f| f.resource == resource) {
                        f.window.set_focus();
                        return;
                    }
                    let (back, up, root) = (worker.clone(), up.clone(), root.clone());
                    std::thread::spawn(move || open(&up, &back, &root, node, resource, name));
                }
                Ok(Act::Send { to, text }) => {
                    let (back, up) = (worker.clone(), up.clone());
                    std::thread::spawn(move || send(&up, &back, to, text));
                }
                Ok(Act::Attach { node, address }) => {
                    let p = up.running.lock().unwrap().clone();
                    let Some(p) = p else {
                        let _ =
                            ours.evaluate_script(&said(false, "this desktop is not set up yet"));
                        return;
                    };
                    // **off the drawing thread**: an attach dials and waits
                    let (back, up) = (worker.clone(), up.clone());
                    std::thread::spawn(move || match serve_from(&p, node, address) {
                        Ok(()) => {
                            draw(&up, &back, false);
                            if let Some(why) = sweep(&up) {
                                let _ = back.send_event(Said::ToPage(false, why));
                            }
                            draw(&up, &back, false);
                        }
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

/// **What the window's workers share**: the kernel once it is up, what
/// this desktop is to it, where the window is, the identity's threads,
/// and whether the window is closing.
struct Up {
    running: Mutex<Option<Arc<Participant>>>,
    device: Mutex<Option<Device>>,
    place: Mutex<History>,
    /// Where this desktop keeps what it keeps, its colours among it.
    dir: PathBuf,
    threads: Mutex<Option<Arc<Threads>>>,
    closing: AtomicBool,
}

/// The kernel's sealing service, which the threads are kept under.
struct KernelSealer(Arc<Participant>);

impl Sealer for KernelSealer {
    fn seal(&self, label: &str, bytes: &[u8]) -> Vec<u8> {
        self.0.seal(label.to_string(), bytes.to_vec())
    }
    fn unseal(&self, label: &str, bytes: &[u8]) -> Option<Vec<u8>> {
        self.0.unseal(label.to_string(), bytes.to_vec()).ok()
    }
}

/// **The place the window is at, drawn from the kernel as it stands**:
/// whole where it was just gone to, its rail and body alone otherwise.
/// Off the drawing thread, since every question here waits on the kernel.
fn draw(up: &Up, back: &EventLoopProxy<Said>, whole: bool) {
    let p = up.running.lock().unwrap().clone();
    let d = up.device.lock().unwrap().clone();
    let (Some(p), Some(d)) = (p, d) else {
        return;
    };
    let place = up.place.lock().unwrap().here().clone();
    let threads = up.threads.lock().unwrap().clone();
    let mut snap = snapshot(&p, d, threads.as_deref());
    snap.colour = participant::colour_of(&up.dir, &snap.me);
    let body = match &place {
        Place::Thread(peer) => {
            let page = threads
                .map(|t| t.page(peer, None, threads::PAGE, HashMap::new()))
                .unwrap_or_default();
            Body::Thread(page.messages, page.before.is_some())
        }
        // **what the subnet's node offers**, one launcher each: the
        // resources swept from the node serving this key there
        Place::Subnet(id) => {
            let node = snap
                .subnets
                .iter()
                .find(|s| &s.id == id)
                .map(|s| s.node.clone());
            Body::Launchers(
                p.resources()
                    .into_iter()
                    .filter(|r| node.as_deref() == Some(hex(&r.node).as_str()))
                    .map(|r| ResourceRow {
                        instance: r.instance,
                        node: hex(&r.node),
                        resource: hex(&r.resource),
                        unseen: r.waiting,
                        icon: r.icon_format.zip(r.icon),
                    })
                    .collect(),
            )
        }
        // **every line this key holds**, each patron by the alias held
        Place::Authority => {
            let names = aliases(&p);
            Body::Authority(
                p.lines()
                    .into_iter()
                    .map(|line| LineRow {
                        alias: names.get(&hex(&line.patron)).cloned(),
                        line,
                    })
                    .collect(),
            )
        }
        _ => Body::Plain,
    };
    let _ = back.send_event(if whole {
        Said::Show(screens::page(&snap, &place, &body))
    } else {
        let script = screens::refresh(&snap, &place, &body);
        Said::Redraw(place, script)
    });
}

/// **The names this key holds for the parties it knows**, by keyhash in
/// hex: what the authority screen names a patron by.
fn aliases(p: &Participant) -> HashMap<String, String> {
    p.contacts()
        .into_iter()
        .filter_map(|c| c.alias.map(|a| (hex(&c.keyhash), a)))
        .collect()
}

/// **What leaving `patron` costs, put on the authority screen** where it
/// still stands: worked out off the drawing thread, since it walks the
/// topology this key holds.
fn cost(up: &Up, back: &EventLoopProxy<Said>, patron: Vec<u8>) {
    let p = up.running.lock().unwrap().clone();
    let d = up.device.lock().unwrap().clone();
    let (Some(p), Some(d)) = (p, d) else {
        return;
    };
    let names = aliases(&p);
    let who = hex(&patron);
    let c = p.cost_of_leaving(patron).map_err(|e| e.to_string());
    let script = screens::cost_script(
        &d,
        &who,
        names.get(&who).map(String::as_str),
        c.as_ref().map_err(String::as_str),
        &names,
    );
    let _ = back.send_event(Said::Redraw(Place::Authority, script));
}

/// **The kernel's answers as the screens' rows**: where it is connected,
/// its subnets, the people met, the threads kept.
fn snapshot(p: &Participant, device: Device, threads: Option<&Threads>) -> Snapshot {
    let agreed: Vec<(Vec<u8>, Vec<String>)> = p
        .agreed()
        .into_iter()
        .map(|a| (a.serving, a.addresses))
        .collect();
    let subnets = p.subnets();
    let me = p.me();
    let resources = p.resources();
    // **a subnet's own name for this person, then the default**, as the
    // phone's foot picks it (`Gateway.ownAlias`)
    let name = subnets
        .iter()
        .find_map(|s| s.own_alias.clone().filter(|a| !a.trim().is_empty()))
        .or_else(|| p.default_alias().filter(|a| !a.trim().is_empty()));
    Snapshot {
        colour: String::new(),
        me: p.me(),
        name,
        device,
        connection: participant::connection(&p.attachments(), &agreed),
        subnets: subnets
            .into_iter()
            .map(|s| {
                // **the node serving this key there**: the patron's, which
                // speaks as the patron, else this key's own as the root
                let node = s.patron.map_or_else(|| me.clone(), |k| k.to_vec());
                // **the orb's count** [author, 2026-11-23]: a move of its
                // node to answer, as the phone marks one, and what its
                // resources ask, rolled up
                let moved = s.relocation.as_ref().is_some_and(|r| {
                    matches!(
                        r.standing,
                        rhtn_model::Standing::Proved | rhtn_model::Standing::Declined
                    )
                });
                let asked: u32 = resources
                    .iter()
                    .filter(|r| r.node[..] == node[..])
                    .map(|r| r.waiting)
                    .sum();
                SubnetRow {
                    id: hex(&s.id),
                    name: s.alias.or(s.own_alias),
                    rank: rank(&s.rank),
                    crown: Crown::of(s.rank.root, s.rank.operator, s.rank.subordinates),
                    node: hex(&node),
                    unseen: u32::from(moved) + asked,
                }
            })
            .collect(),
        // **the people are the ones met**, as the phone's list has them:
        // an introduction, a verifier and a node are held but not listed
        people: p
            .contacts()
            .into_iter()
            .filter(|c| c.how == rhtn_model::How::Counterparty)
            .map(|c| Person {
                key: hex(&c.keyhash),
                alias: c.alias,
                how: match c.count {
                    1 => "met once".to_string(),
                    n => format!("met {n} times"),
                },
            })
            .collect(),
        threads: threads.map(Threads::index).unwrap_or_default(),
    }
}

/// What this key is in a subnet, in the phone's words for its downline.
fn rank(r: &rhtn_model::Rank) -> String {
    let what = match (r.root, r.operator) {
        (true, true) => "Its root, and its operator.",
        (true, false) => "Its root.",
        (false, true) => "Its operator.",
        (false, false) => "A member.",
    };
    let lines = match r.subordinates {
        0 => "No line of yours is issued here.".to_string(),
        1 => "Your downline here: 1 direct line.".to_string(),
        n => format!("Your downline here: {n} direct lines."),
    };
    format!("{what} {lines}")
}

/// **The kernel's events, for as long as the window is open**: a message
/// kept in its thread as it arrives, and the place redrawn where what it
/// shows changed.  The phone's loop, less the meeting a desktop never
/// holds.
fn pump(up: &Up, back: &EventLoopProxy<Said>, p: &Participant) {
    while !up.closing.load(Ordering::Relaxed) {
        let Some(e) = p.next_event(2000) else {
            continue;
        };
        match e {
            Kernel::Payload { from, kind, bytes } if kind == kind_application() => {
                let peer = hex(&from);
                let who = p
                    .contacts()
                    .into_iter()
                    .find(|c| c.keyhash[..] == from[..])
                    .and_then(|c| c.alias)
                    .unwrap_or_else(|| peer.chars().take(16).collect());
                let text = String::from_utf8_lossy(&bytes).into_owned();
                let kept = up.threads.lock().unwrap().clone().map(|t| {
                    t.append(
                        &peer,
                        &Record::Said {
                            mine: false,
                            who: who.clone(),
                            text,
                            delivery: Delivery::Sent,
                            reason: None,
                        },
                    )
                    .and_then(|_| t.list(&peer, &who))
                });
                if let Some(Err(why)) = kept {
                    let _ = back.send_event(Said::ToPage(
                        false,
                        format!("a message from {who} was not kept: {why}"),
                    ));
                }
                draw(up, back, false);
            }
            Kernel::Payload { kind, bytes, .. } => {
                let _ = back.send_event(Said::ToPage(
                    true,
                    format!(
                        "a payload of kind {kind}, {} bytes, not one this screen reads",
                        bytes.len()
                    ),
                ));
            }
            Kernel::Connection(_) | Kernel::Changed { .. } => draw(up, back, false),
            Kernel::Peer { what, .. } | Kernel::Conversed { what, .. } => {
                let _ = back.send_event(Said::ToPage(true, what));
            }
            Kernel::Answered {
                refused: Some(why), ..
            } => {
                let _ = back.send_event(Said::ToPage(
                    false,
                    format!("a response was refused: {why}"),
                ));
            }
            _ => {}
        }
    }
}

/// **Open a backup and install its archive** (`light-client-requirements.md`
/// §2): the kernel opens it under this identity, scans it for what is past
/// its retention, and refuses one of another identity or a desktop that
/// holds an archive already.  What came back is said, the discard with it.
fn import(up: &Up, back: &EventLoopProxy<Said>, bytes: Vec<u8>) {
    let Some(p) = up.running.lock().unwrap().clone() else {
        let _ = back.send_event(Said::ToPage(false, "this desktop is not set up yet".into()));
        return;
    };
    match p.restore_backup(bytes) {
        Ok(r) => {
            draw(up, back, false);
            let _ = back.send_event(Said::ToPage(true, imported(r.records, r.discarded)));
        }
        Err(why) => {
            let _ = back.send_event(Said::ToPage(
                false,
                format!("the backup was not taken: {why}"),
            ));
        }
    }
}

/// **What an import came to**, said as the phone says it: how much came
/// back, and what the scan deleted on the way in, which is told rather
/// than swallowed (design §13.7.1).
fn imported(records: u64, discarded: u64) -> String {
    let took = format!(
        "{records} record{} came back.",
        if records == 1 { "" } else { "s" }
    );
    match discarded {
        0 => format!("{took} Nothing in it was past its retention."),
        1 => format!(
            "{took} 1 sealed capture was past the window its subject declared, and was \
             deleted on import rather than kept."
        ),
        n => format!(
            "{took} {n} sealed captures were past the window their subjects declared, and \
             were deleted on import rather than kept."
        ),
    }
}

/// **Say something to a party**, as the phone's conversation does: kept as
/// sending first, so it is on the screen at once, then handed to the
/// kernel, and the answer kept as a change to it.
fn send(up: &Up, back: &EventLoopProxy<Said>, to: Vec<u8>, text: String) {
    let p = up.running.lock().unwrap().clone();
    let t = up.threads.lock().unwrap().clone();
    let (Some(p), Some(t)) = (p, t) else {
        let _ = back.send_event(Said::ToPage(false, "this desktop is not set up yet".into()));
        return;
    };
    let peer = hex(&to);
    let mine = Record::Said {
        mine: true,
        who: "me".into(),
        text: text.clone(),
        delivery: Delivery::Sending,
        reason: None,
    };
    let id = match t.append(&peer, &mine) {
        Ok(id) => id,
        Err(why) => {
            let _ = back.send_event(Said::ToPage(false, format!("not kept, so not sent: {why}")));
            return;
        }
    };
    if !t.index().iter().any(|e| e.peer == peer) {
        let _ = t.list(&peer, &peer);
    }
    draw(up, back, false);
    let (delivery, reason) = match p.send(to, kind_application(), text.into_bytes()) {
        Ok(()) => (Delivery::Sent, None),
        Err(why) => (Delivery::Unsent, Some(why.to_string())),
    };
    let _ = t.append(
        &peer,
        &Record::Settled {
            id,
            delivery,
            reason,
        },
    );
    draw(up, back, false);
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

/// **What a connection is for, fetched once one is up**: the profiles
/// that name the people and subnets held — what each device fetches for
/// itself rather than being handed (design §23.3). What each node offers
/// the kernel sweeps at the attach itself, unasked [author, 2026-11-23].
/// The refusal, where there is one.
fn sweep(up: &Up) -> Option<String> {
    let p = up.running.lock().unwrap().clone()?;
    if p.attachments().is_empty() {
        return Some("no node is attached".to_string());
    }
    p.request_profiles()
        .err()
        .map(|e| format!("no profiles were asked for: {e}"))
}

/// **Opening a subnet sweeps the node serving it** [author, 2026-11-23:
/// *"Sweep when the user opens the subnet"*], and its page is drawn again
/// with what that found. What could not be reached goes to the
/// diagnostics and not to the person, whose attention is not drawn to the
/// node.
fn opened(up: &Up, back: &EventLoopProxy<Said>, id: &str) {
    let Some(p) = up.running.lock().unwrap().clone() else {
        return;
    };
    let me = p.me();
    let Some(node) = p
        .subnets()
        .into_iter()
        .find(|s| hex(&s.id) == id)
        .map(|s| s.patron.map_or_else(|| me.clone(), |k| k.to_vec()))
    else {
        return;
    };
    if let Err(why) = p.browse(node) {
        tracing::info!(target: "attach", subnet = &id[..8], why = %why, "subnet.unswept");
    }
    draw(up, back, false);
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

/// **Open a resource** (`light-client-requirements.md` §8): its `GET /`
/// carried first, off the drawing thread, so what is not delivered or not
/// a page is said in the main window and no window opens for it; then the
/// question about its code where its page declares code and the person has
/// not answered; then the window.
fn open(
    up: &Up,
    back: &EventLoopProxy<Said>,
    root: &Path,
    node: Vec<u8>,
    resource: Vec<u8>,
    name: String,
) {
    let Some(p) = up.running.lock().unwrap().clone() else {
        let _ = back.send_event(Said::ToPage(false, "this desktop is not set up yet".into()));
        return;
    };
    let first = frame::compose("GET", "/", None, b"");
    let answer = match carry(&p, &node, &resource, first, &name, back) {
        Ok(a) => a,
        Err(why) => {
            let _ = back.send_event(Said::ToPage(false, format!("{name}: {why}")));
            return;
        }
    };
    let shown = frame::served(&answer, false);
    let page = match &answer.page {
        Some(page) if answer.status == 0 && answer.unreadable.is_none() => page,
        _ => {
            let why = shown
                .headers
                .iter()
                .find(|(n, _)| n == frame::CARRIED)
                .map(|_| String::from_utf8_lossy(&shown.body).trim().to_string())
                .unwrap_or_default();
            let _ = back.send_event(Said::ToPage(false, format!("{name}: {why}")));
            return;
        }
    };
    if page.shape == PageShape::Other {
        offer(back, &name, page);
        return;
    }
    let key = hex(&resource);
    let kept = frame::script_allowed(root, &key);
    let allowed = match kept {
        Some(a) => a,
        None if page.runs_script => {
            let (tx, rx) = channel();
            let yes = back
                .send_event(Said::Ask(format!("{name}: {}", frame::SCRIPT_ASKS), tx))
                .is_ok()
                && rx.recv().unwrap_or(false);
            if let Err(why) = frame::keep_script(root, &key, yes) {
                let _ = back.send_event(Said::ToPage(false, format!("not remembered: {why}")));
            }
            yes
        }
        None => false,
    };
    let _ = back.send_event(Said::Frame(Framed {
        node,
        resource,
        name,
        first: frame::served(&answer, allowed),
        allowed,
    }));
}

/// **One request carried to a resource**, and every ask it answers with
/// answered (§8.1): this desktop offers none of the device utilities yet,
/// so each is declined with that as its reason — an answer the resource
/// sees, never a silence — and the person is told what was asked.
fn carry(
    p: &Participant,
    node: &[u8],
    resource: &[u8],
    message: Vec<u8>,
    name: &str,
    back: &EventLoopProxy<Said>,
) -> Result<ResourceAnswer, String> {
    let mut a = p
        .request_resource(node.to_vec(), resource.to_vec(), message)
        .map_err(|e| e.to_string())?;
    // **a bound on a resource asking in a loop**, and only that: each ask
    // here is answered before the next, so §8.1's eight unanswered is not
    // it. The test resource asks eleven in a row; this client's own
    // number, not the protocol's
    for _ in 0..ASKS_RUNNING {
        let Some(ask) = a.ask.take() else {
            return Ok(a);
        };
        let _ = back.send_event(Said::ToPage(
            false,
            format!(
                "{name} asked for `{}`, which this desktop has not got; it was told so",
                ask.utility
            ),
        ));
        a = p
            .answer_utility(
                ask.id,
                UtilityOutcome::Declined {
                    reason: frame::NO_UTILITY.into(),
                },
            )
            .map_err(|e| e.to_string())?;
    }
    Err(format!(
        "it asked {ASKS_RUNNING} times running, and nothing was shown"
    ))
}

/// **Offer what a resource answered with for saving** (the phone's "Save
/// to a file"): named in the main window with its declared type and size,
/// and kept only on the person's yes, through a save dialog. Asked off
/// the thread that carried the answer, which does not wait for it.
fn offer(back: &EventLoopProxy<Said>, name: &str, page: &rhtn_ffi::client::ResourcePage) {
    if page.body.is_empty() {
        let _ = back.send_event(Said::ToPage(
            true,
            format!("{name} answered with {}", frame::named(page)),
        ));
        return;
    }
    let back = back.clone();
    let question = format!(
        "{name} answered with {}. Save it to a file?",
        frame::named(page)
    );
    let offered = Offered {
        suggested: frame::suggested(name, page.media.as_deref()),
        body: page.body.clone(),
    };
    std::thread::spawn(move || {
        let (tx, rx) = channel();
        if back.send_event(Said::Ask(question, tx)).is_ok() && rx.recv().unwrap_or(false) {
            let _ = back.send_event(Said::Save(offered));
        }
    });
}

/// **The save dialog**, on the drawing thread where the toolkit lives:
/// the place is the person's to choose, a file already there is replaced
/// only on their word, and what came of it is said.
#[cfg(target_os = "linux")]
fn save(offered: Offered) -> Result<Option<PathBuf>, String> {
    use gtk::prelude::*;
    let dialog = gtk::FileChooserNative::new(
        Some("Save what the resource answered with"),
        None::<&gtk::Window>,
        gtk::FileChooserAction::Save,
        Some("Save"),
        Some("Cancel"),
    );
    dialog.set_current_name(&offered.suggested);
    dialog.set_do_overwrite_confirmation(true);
    let chosen = (dialog.run() == gtk::ResponseType::Accept)
        .then(|| dialog.filename())
        .flatten();
    dialog.destroy();
    let Some(path) = chosen else {
        return Ok(None);
    };
    std::fs::write(&path, &offered.body).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Some(path))
}

/// Elsewhere there is no dialog yet, and nothing is written.
#[cfg(not(target_os = "linux"))]
fn save(_: Offered) -> Result<Option<PathBuf>, String> {
    Err("this desktop has no save dialog on this platform yet".into())
}

/// How many asks in a row one request may meet before the desktop stops
/// answering them ([`carry`]).
const ASKS_RUNNING: usize = 32;

/// **A resource's window** (`frame`): a web context of its own under the
/// desktop's directory, so its storage is its own and kept; its origin's
/// scheme carried through the kernel; a navigation anywhere else not
/// followed and said; no window it could open, no download it could
/// start; and `rhtn.request` only where the person allowed its code.
fn open_frame(
    target: &tao::event_loop::EventLoopWindowTarget<Said>,
    root: &Path,
    framed: Framed,
    p: Arc<Participant>,
    back: EventLoopProxy<Said>,
) -> Result<FrameWindow, Box<dyn std::error::Error>> {
    let Framed {
        node,
        resource,
        name,
        first,
        allowed,
    } = framed;
    let key = hex(&resource);
    let held = resource.clone();
    let origin = frame::origin(&key)?;
    let storage = frame::dir(root, &key).join("web");
    platform::create_private(&storage)?;
    let mut context = wry::WebContext::new(Some(storage));
    let w = WindowBuilder::new()
        .with_title(format!("{name} — a resource, in a frame of its own"))
        .with_inner_size(LogicalSize::new(1000.0, 820.0))
        .build(target)?;

    let carry: rhtn_desktop::window::Carrier = {
        let back = Mutex::new(back.clone());
        Arc::new(move |method, path, media, body, document| {
            let message = frame::compose(method, path, media, body);
            let back = back.lock().unwrap().clone();
            match carry(&p, &node, &resource, message, &name, &back) {
                // **the page moving to something it cannot show** is
                // named there, and the bytes offered for keeping here,
                // as the phone's save does; a script's own request gets
                // the bytes, which are its to read
                Ok(a) => match &a.page {
                    Some(page)
                        if document
                            && a.status == 0
                            && a.unreadable.is_none()
                            && page.shape == PageShape::Other =>
                    {
                        offer(&back, &name, page);
                        frame::not_shown(page)
                    }
                    _ => frame::served(&a, allowed),
                },
                Err(why) => frame::not_carried(&why),
            }
        })
    };
    let builder = rhtn_desktop::window::builder(
        &mut context,
        &origin,
        Some(first),
        allowed,
        carry,
        move || {
            let _ = back.send_event(Said::ToPage(
                false,
                "a resource's page reaches its own resource and nothing else: its window did \
                 not follow that"
                    .into(),
            ));
        },
    );
    let view = web_view(builder, &w)?;
    Ok(FrameWindow {
        resource: held,
        window: w,
        _view: view,
        _context: context,
    })
}
