//! Write the participant window's pages to a directory, beside nothing:
//! the caller puts `tokens.css` and `fonts/` there to look at them in a
//! browser.  `cargo run -p rhtn-desktop --example render_pages -- <dir>`
//!
//! The screens past setup are drawn from made-up rows, the shapes the
//! kernel answers in, so each place can be looked at with no kernel up.
use rhtn_desktop::participant::Device;
use rhtn_desktop::screens::{self, Body, Person, Place, ResourceRow, Snapshot, SubnetRow};
use rhtn_desktop::threads::{Delivery, Entry, Message};
use rhtn_desktop::{code, pairing, participant};

fn main() {
    let dir = std::env::args().nth(1).expect("a directory");
    let offer = pairing::Offer::new(pairing::fresh_seed(), "127.0.0.1".parse().unwrap()).unwrap();
    let snap = Snapshot {
        me: vec![0x70; 32],
        name: Some("Marcus".into()),
        device: Device::Restored,
        connection: "Connected to 703a82c269ea77ad… at 157.245.85.18:7431".into(),
        subnets: vec![
            SubnetRow {
                id: "2e".repeat(32),
                name: Some("Mirror Manse".into()),
                rank: "Its root, and its operator. Your downline here: 2 direct lines.".into(),
                node: "70".repeat(32),
                unseen: 2,
            },
            SubnetRow {
                id: "9a".repeat(32),
                name: None,
                rank: "A member. No line of yours is issued here.".into(),
                node: "75".repeat(32),
                unseen: 0,
            },
        ],
        people: vec![
            Person {
                key: "33".repeat(32),
                alias: Some("Carol".into()),
                how: "met 3 times".into(),
            },
            Person {
                key: "44".repeat(32),
                alias: None,
                how: "met once".into(),
            },
        ],
        threads: vec![Entry {
            peer: "33".repeat(32),
            name: "Carol".into(),
            direct: true,
        }],
    };
    let said = |id, mine, text: &str, delivery| Message {
        id,
        mine,
        who: if mine { "me" } else { "Carol" }.into(),
        text: text.into(),
        delivery,
        reason: (delivery == Delivery::Unsent).then(|| "no session with the node".into()),
    };
    let thread = Body::Thread(
        vec![
            said(0, false, "Are you at the bench today?", Delivery::Sent),
            said(80, true, "Yes, from the desktop this time.", Delivery::Sent),
            said(
                170,
                true,
                "This one waits on the kernel.",
                Delivery::Sending,
            ),
            said(260, true, "And this one did not go.", Delivery::Unsent),
        ],
        true,
    );
    let catalog = Body::Launchers(vec![
        ResourceRow {
            instance: "Shared drive".into(),
            node: "70".repeat(32),
            resource: "71".repeat(32),
            unseen: 0,
        },
        ResourceRow {
            instance: "Notices".into(),
            node: "70".repeat(32),
            resource: "72".repeat(32),
            unseen: 2,
        },
    ]);
    let pages = [
        (
            "pair.html",
            participant::pair_page(&code::svg(&offer.code(), 320).unwrap()),
        ),
        ("launch.html", rhtn_desktop::page()),
        (
            "stopped.html",
            participant::stopped_page("no Secret Service on this session"),
        ),
        (
            "home.html",
            screens::page(&snap, &Place::Home, &Body::Plain),
        ),
        (
            "direct.html",
            screens::page(&snap, &Place::Direct, &Body::Plain),
        ),
        (
            "thread.html",
            screens::page(&snap, &Place::Thread("33".repeat(32)), &thread),
        ),
        (
            "subnet.html",
            screens::page(&snap, &Place::Subnet("2e".repeat(32)), &catalog),
        ),
        (
            "options.html",
            screens::page(&snap, &Place::Options, &Body::Plain),
        ),
    ];
    for (name, html) in pages {
        std::fs::write(format!("{dir}/{name}"), html).unwrap();
    }
}
