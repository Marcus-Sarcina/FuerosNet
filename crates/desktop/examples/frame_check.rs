//! **What the engine does with a resource's frame**, checked in the
//! engine itself (`rhtn_desktop::window`): the unit tests hold what this
//! client tells WebKitGTK; this holds what WebKitGTK then does with it,
//! which no unit test can.
//!
//! ```sh
//! cargo run -p rhtn-desktop --features gui --example frame_check
//! ```
//!
//! Two canned resources, each in a window and a web context of its own,
//! served by a carrier that answers from a table rather than the kernel:
//!
//!  - **one whose code is allowed**, and whose page tries everything a page
//!    might — a form's body through the scheme, `rhtn.request`, a fetch off
//!    its origin, a path under `/.rhtn/`, storage of its own, and a
//!    navigation away — and writes down what happened;
//!  - **one whose page declares code the person refused**, which must run
//!    none of it.
//!
//! The windows open on the display, read back what each page recorded,
//! and close; the verdict is printed and the exit status is it.

use rhtn_desktop::frame;
use rhtn_ffi::client::{PageShape, ResourceAnswer, ResourcePage};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;

const ALLOWED: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const REFUSED: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";

/// The page whose code runs: every attempt recorded on `window.seen`.
const TRYING: &str = r#"<h1>trying</h1>
<iframe src="/doc" hidden></iframe>
<script>
window.seen = {};
const note = (k, v) => { window.seen[k] = v; };
note('ran', true);
try { localStorage.setItem('k', 'kept'); note('storage', localStorage.getItem('k')); }
catch (e) { note('storage', 'threw ' + e); }
try { note('bridge', JSON.parse(rhtn.request('POST', '/echo', 'text/plain', 'hé'))); }
catch (e) { note('bridge', 'threw ' + e); }
try { note('bridge_rhtn', JSON.parse(rhtn.request('GET', '/.rhtn/x', null, null))); }
catch (e) { note('bridge_rhtn', 'threw ' + e); }
fetch('/echo', { method: 'POST', headers: { 'content-type': 'text/plain' }, body: 'a=1' })
  .then(r => r.text()).then(t => note('fetch_post', t), e => note('fetch_post', 'refused ' + e));
fetch('https://example.com/').then(r => note('fetch_away', 'reached ' + r.status),
  e => note('fetch_away', 'refused'));
const img = new Image();
img.onload = () => note('img_away', 'loaded');
img.onerror = () => note('img_away', 'refused');
img.src = 'https://example.com/favicon.ico';
setTimeout(() => { note('navigating', true); location.href = 'https://example.com/'; }, 1500);
</script>"#;

/// The page whose code the person refused: it must not run.
const REFUSING: &str = r#"<h1>refused</h1>
<script>window.seen = { ran: true };</script>
<p id="p" onclick="window.seen={ran:true}">no code here</p>"#;

fn page(body: &str, policy: &str) -> frame::Served {
    let a = ResourceAnswer {
        status: 0,
        page: Some(ResourcePage {
            http_status: 200,
            media: Some("text/html".into()),
            charset: Some("utf-8".into()),
            shape: PageShape::Page,
            policy: Some(policy.into()),
            runs_script: true,
            body: body.as_bytes().to_vec(),
        }),
        unreadable: None,
        ask: None,
    };
    frame::served(&a, true)
}

fn text(body: Vec<u8>) -> frame::Served {
    frame::Served {
        status: 200,
        headers: vec![("content-type".into(), "text/plain; charset=utf-8".into())],
        body,
    }
}

fn main() {
    let event_loop = EventLoopBuilder::<()>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let dir = std::env::temp_dir().join(format!("rhtn-frame-check-{}", std::process::id()));
    let refusals = Arc::new(AtomicUsize::new(0));
    let carried: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let mut held = Vec::new();
    for (resource, allowed, first) in [
        (
            ALLOWED,
            true,
            page(TRYING, "script-src 'self' 'unsafe-inline'"),
        ),
        (REFUSED, false, {
            let a = ResourceAnswer {
                status: 0,
                page: Some(ResourcePage {
                    http_status: 200,
                    media: Some("text/html".into()),
                    charset: None,
                    shape: PageShape::Page,
                    policy: Some("script-src 'self' 'unsafe-inline'".into()),
                    runs_script: true,
                    body: REFUSING.as_bytes().to_vec(),
                }),
                unreadable: None,
                ask: None,
            };
            frame::served(&a, false)
        }),
    ] {
        let origin = frame::origin(resource).unwrap();
        let mut context = wry::WebContext::new(Some(frame::dir(&dir, resource).join("web")));
        let w = WindowBuilder::new()
            .with_title(format!("frame check: {}", &resource[..4]))
            .build(&event_loop)
            .unwrap();
        let log = carried.clone();
        let carry: rhtn_desktop::window::Carrier =
            Arc::new(move |method, path, media, body, document| {
                log.lock().unwrap().push(format!(
                    "{method} {path} {media:?} {} doc={document}",
                    body.len()
                ));
                match path {
                    "/echo" => text(body.to_vec()),
                    _ => frame::nothing(),
                }
            });
        let r = refusals.clone();
        let b = rhtn_desktop::window::builder(
            &mut context,
            &origin,
            Some(first),
            allowed,
            carry,
            move || {
                r.fetch_add(1, Ordering::Relaxed);
            },
        );
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let v = b.build_gtk(w.default_vbox().unwrap()).unwrap();
        held.push((resource, w, v, context));
    }

    // **read back once the pages have had their time**: the navigation
    // away is at 1.5 s, so 4 s sees everything settle
    let later = proxy.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        let _ = later.send_event(());
    });
    let results: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let mut asked = false;
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        let Event::UserEvent(()) = event else {
            return;
        };
        if !asked {
            asked = true;
            for (resource, _, v, _) in &held {
                let (results, name, wake) = (results.clone(), resource.to_string(), proxy.clone());
                v.evaluate_script_with_callback(
                    "JSON.stringify(window.seen === undefined ? null : window.seen)",
                    move |out| {
                        results.lock().unwrap().push((name.clone(), out));
                        let _ = wake.send_event(());
                    },
                )
                .unwrap();
            }
            return;
        }
        let got = results.lock().unwrap().clone();
        if got.len() < held.len() {
            return;
        }
        let ok = verdict(
            &got,
            refusals.load(Ordering::Relaxed),
            &carried.lock().unwrap(),
        );
        let _ = std::fs::remove_dir_all(&dir);
        std::process::exit(if ok { 0 } else { 1 });
    });
}

fn verdict(got: &[(String, String)], refusals: usize, carried: &[String]) -> bool {
    let mut ok = true;
    let mut check = |what: &str, held: bool| {
        println!("{} {what}", if held { "ok  " } else { "FAIL" });
        ok &= held;
    };
    for (resource, out) in got {
        println!("{}: {out}", &resource[..4]);
        // the engine hands back the value as JSON, and the value is a
        // string of JSON
        let v = serde_json::from_str::<serde_json::Value>(out)
            .ok()
            .and_then(|v| match v {
                serde_json::Value::String(s) => serde_json::from_str(&s).ok(),
                other => Some(other),
            })
            .unwrap_or(serde_json::Value::Null);
        if resource == REFUSED {
            check("refused: its code did not run", v.is_null());
            continue;
        }
        check("allowed: its code ran", v["ran"] == true);
        check("allowed: storage of its own", v["storage"] == "kept");
        check(
            "allowed: rhtn.request carried a body and answered as the phone's",
            v["bridge"]["status"] == 200
                && v["bridge"]["media"] == "text/plain"
                && v["bridge"]["body"] == "aMOp",
        );
        check(
            "allowed: rhtn.request refuses /.rhtn/",
            v["bridge_rhtn"]["error"]
                .as_str()
                .is_some_and(|e| e.contains("/.rhtn/")),
        );
        check(
            "allowed: a fetch's body is carried by the scheme",
            v["fetch_post"] == "a=1",
        );
        check(
            "allowed: a fetch off its origin is refused",
            v["fetch_away"] == "refused",
        );
        check(
            "allowed: an image off its origin is refused",
            v["img_away"] == "refused",
        );
        check(
            "allowed: it tried to navigate away",
            v["navigating"] == true,
        );
    }
    check("the navigation away was not followed", refusals >= 1);
    check(
        "a frame's own navigation is told as one",
        carried
            .iter()
            .any(|c| c.starts_with("GET /doc ") && c.ends_with("doc=true")),
    );
    check(
        "a script's request is not",
        carried
            .iter()
            .any(|c| c.starts_with("POST /echo ") && c.ends_with("doc=false")),
    );
    println!("carried: {carried:?}");
    ok
}
