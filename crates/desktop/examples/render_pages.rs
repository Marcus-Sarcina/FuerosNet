//! Write the participant window's pages to a directory, beside nothing:
//! the caller puts `tokens.css` and `fonts/` there to look at them in a
//! browser.  `cargo run -p rhtn-desktop --example render_pages -- <dir>`
use rhtn_desktop::{code, pairing, participant};

fn main() {
    let dir = std::env::args().nth(1).expect("a directory");
    let offer = pairing::Offer::new(pairing::fresh_seed(), "127.0.0.1".parse().unwrap()).unwrap();
    let pages = [
        (
            "pair.html",
            participant::pair_page(&code::svg(&offer.code(), 320).unwrap()),
        ),
        (
            "home.html",
            participant::home_page(
                &[0x5a; 32],
                &participant::Device::Restored,
                "Not connected to any node.",
            ),
        ),
        ("launch.html", rhtn_desktop::page()),
        (
            "stopped.html",
            participant::stopped_page("no Secret Service on this session"),
        ),
    ];
    for (name, html) in pages {
        std::fs::write(format!("{dir}/{name}"), html).unwrap();
    }
}
