//! **A resource's web view, as the engine is told to build it** (`frame`):
//! the one definition the window and the engine check
//! (`examples/frame_check.rs`) both build, so the check cannot pass while
//! the window drifts — the phone's `Frames.resource` argument.
//!
//! What is here is the engine's side alone. What a request carries, and
//! where to, is the [`Carrier`]'s: the kernel's in the window, a canned
//! resource in the check.

use crate::frame;
use std::sync::{Arc, Mutex};
use wry::WebViewBuilder;
use wry::http::{Response, StatusCode};

/// **What carries one request to the resource**: its method, its rooted
/// path, its body's declared type and the body, to the answer the engine
/// is given. Called off the drawing thread, and may wait.
pub type Carrier = Arc<dyn Fn(&str, &str, Option<&str>, &[u8]) -> frame::Served + Send + Sync>;

/// **A resource's web view**, in `context`: its origin's scheme answered
/// by `carry` and nothing else, `first` served to the first `GET /` where
/// it is in hand, a navigation anywhere else not followed and `refused`
/// called, no window it could open and no download it could start, and
/// `rhtn.request` published only where `allowed`.
pub fn builder<'a>(
    context: &'a mut wry::WebContext,
    origin: &str,
    first: Option<frame::Served>,
    allowed: bool,
    carry: Carrier,
    refused: impl Fn() + 'static,
) -> WebViewBuilder<'a> {
    let first = Arc::new(Mutex::new(first));
    let (serving, inside) = (origin.to_string(), origin.to_string());
    let b = WebViewBuilder::new_with_web_context(context)
        .with_asynchronous_custom_protocol(frame::SCHEME.into(), move |_, request, responder| {
            if !frame::inside(&serving, &request.uri().to_string()) {
                return responder.respond(response(frame::nothing()));
            }
            let path = request
                .uri()
                .path_and_query()
                .map_or("/".to_string(), |p| p.as_str().to_string());
            let method = request.method().as_str().to_string();
            if let Err(why) = frame::carried(&method, &path) {
                return responder.respond(response(frame::not_carried(&why)));
            }
            if method == "GET"
                && path == "/"
                && let Some(s) = first.lock().unwrap().take()
            {
                return responder.respond(response(s));
            }
            let media = request
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(String::from);
            let body = request.body().clone();
            let carry = carry.clone();
            // **off the drawing thread**: the kernel and the network are
            // never on it, and the engine waits for its answer meanwhile
            std::thread::spawn(move || {
                responder.respond(response(carry(&method, &path, media.as_deref(), &body)));
            });
        })
        .with_navigation_handler(move |url| {
            let going = frame::inside(&inside, &url);
            if !going {
                refused();
            }
            going
        })
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        // **the inspector, for whoever is writing the page**, in a
        // field-test build alone
        .with_devtools(cfg!(feature = "fieldtest"))
        .with_url(format!("{origin}/"));
    if allowed {
        b.with_initialization_script(frame::bridge(origin))
    } else {
        b
    }
}

/// The engine's response for one framed answer.
fn response(s: frame::Served) -> Response<Vec<u8>> {
    let mut r = Response::builder().status(s.status);
    for (name, value) in &s.headers {
        r = r.header(name.as_str(), value.as_str());
    }
    r.body(s.body).unwrap_or_else(|_| {
        Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Vec::new())
            .unwrap()
    })
}
