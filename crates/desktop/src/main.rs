//! **The window** (`rhtn_desktop`): one of ours showing the page this
//! crate composes, and one of the provider's own on demand, each a web
//! view of the system's rather than a browser shipped with this binary.
//!
//! **What is here is only the window.** Every decision — what the page
//! says, what an act means, what a token is worth, where it is kept — is
//! the library's, tested with no desktop libraries present. This file
//! carries the event loop, and the one rule it keeps for itself: **the
//! network never runs on the thread that draws**, so a provider that
//! takes five seconds to answer does not freeze the window an operator is
//! reading.
//!
//! **The provider's console is their page in a window of ours**, another
//! origin which nothing here reads: the operator makes a token there and
//! pastes it into ours, which is the whole of the capture.

use rhtn_cli::launch::TlsTransport;
use rhtn_desktop::{Act, act, directory, page, said, verify_and_keep};
use std::sync::Arc;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

/// What reaches the event loop from somewhere else: the page's messages,
/// and the answers a worker thread brings back.
enum Said {
    /// One message from our own page, as it sent it.
    FromPage(String),
    /// A line for the person, and whether it went through.
    ToPage(bool, String),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // the directory tokens are kept in: named on the command line, else
    // the platform's own
    let named = std::env::args().nth(1);
    let dir = match directory(named.as_deref()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("rhtn-desktop: {e}");
            std::process::exit(2);
        }
    };

    let event_loop = EventLoopBuilder::<Said>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("Launch a node — RHTN")
        .with_inner_size(LogicalSize::new(820.0, 900.0))
        .build(&event_loop)?;

    let to_loop = event_loop.create_proxy();
    let ours = WebViewBuilder::new()
        .with_html(page())
        .with_ipc_handler(move |request| {
            let _ = to_loop.send_event(Said::FromPage(request.into_body()));
        })
        .build(&window)?;

    // the provider's own windows, held so that they stay open; a web view
    // dropped is a window gone
    let mut consoles: Vec<(tao::window::Window, wry::WebView)> = Vec::new();
    let worker = event_loop.create_proxy();
    let dir = Arc::new(dir);

    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
                ..
            } => {
                if window_id == window.id() {
                    *control_flow = ControlFlow::Exit;
                } else {
                    // a provider's console closed: let it go, keep ours
                    consoles.retain(|(w, _)| w.id() != window_id);
                }
            }
            Event::UserEvent(Said::FromPage(message)) => match act(&message) {
                Err(why) => {
                    let _ = ours.evaluate_script(&said(false, &why));
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
                    let (back, dir) = (worker.clone(), dir.clone());
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
