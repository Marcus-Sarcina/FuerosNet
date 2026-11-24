//! Write the participant window's pages to a directory, beside nothing:
//! the caller puts `tokens.css` and `fonts/` there to look at them in a
//! browser.  `cargo run -p rhtn-desktop --example render_pages -- <dir>`
//!
//! The screens past setup are drawn from made-up rows, the shapes the
//! kernel answers in, so each place can be looked at with no kernel up.
use rhtn_desktop::participant::Device;
use rhtn_desktop::screens::{self, Body, Crown, Person, Place, ResourceRow, Snapshot, SubnetRow};
use rhtn_desktop::threads::{Delivery, Entry, Message};
use rhtn_desktop::{code, pairing, participant};

fn main() {
    let dir = std::env::args().nth(1).expect("a directory");
    let offer = pairing::Offer::new(pairing::fresh_seed(), "127.0.0.1".parse().unwrap()).unwrap();
    let snap = Snapshot {
        me: vec![0x70; 32],
        name: Some("Marcus".into()),
        device: Device::Restored,
        colour: "Teal".into(),
        held: vec![screens::Holder {
            by: "70".repeat(32),
            name: Some("Marcus".into()),
            colour: "Teal".into(),
        }],
        connection: "Connected to 703a82c269ea77ad… at 157.245.85.18:7431".into(),
        subnets: vec![
            SubnetRow {
                id: "2e".repeat(32),
                name: Some("Mirror Manse".into()),
                rank: "Its root, and its operator. Your downline here: 2 direct lines.".into(),
                crown: Crown::Full,
                node: "70".repeat(32),
                unseen: 2,
                by: "70".repeat(32),
                colour: "Teal".into(),
            },
            SubnetRow {
                id: "9a".repeat(32),
                name: None,
                rank: "A member. No line of yours is issued here.".into(),
                crown: Crown::None,
                node: "75".repeat(32),
                unseen: 0,
                by: "70".repeat(32),
                colour: "Teal".into(),
            },
        ],
        people: vec![
            Person {
                key: "33".repeat(32),
                alias: Some("Carol".into()),
                how: "met 3 times".into(),
                by: "70".repeat(32),
                ..Default::default()
            },
            Person {
                key: "44".repeat(32),
                alias: None,
                how: "met once".into(),
                by: "70".repeat(32),
                ..Default::default()
            },
        ],
        threads: vec![Entry {
            peer: "33".repeat(32),
            name: "Carol".into(),
            direct: true,
            by: "70".repeat(32),
        }],
        chosen: Default::default(),
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
    // long enough to run off the window: the bar and the box stay put and
    // the conversation alone scrolls
    let long = Body::Thread(
        (0..40u64)
            .map(|i| {
                said(
                    i * 60,
                    i % 3 == 0,
                    &format!("Message {} of a long conversation.", i + 1),
                    Delivery::Sent,
                )
            })
            .collect(),
        true,
    );
    let board = Body::Board {
        title: Some("A long thread".into()),
        posts: (0..40u8)
            .map(|i| rhtn_model::BoardPost {
                hash: [i; 32],
                thread: [0; 32],
                writer: if i % 3 == 0 { [0x70; 32] } else { [0x33; 32] },
                alias: Some(if i % 3 == 0 { "Marcus" } else { "Ada" }.into()),
                mine: i % 3 == 0,
                text: format!("Post {} of a long thread.", i + 1),
                written_at: 1_000 + u64::from(i) * 600,
                up: 2,
                down: 2,
                read: true,
            })
            .collect(),
        now: 30_000,
    };
    let catalog = Body::Launchers(
        vec![
            ResourceRow {
                instance: "Shared drive".into(),
                node: "70".repeat(32),
                resource: "71".repeat(32),
                unseen: 0,
                icon: None,
            },
            ResourceRow {
                instance: "Notices".into(),
                node: "70".repeat(32),
                resource: "72".repeat(32),
                unseen: 2,
                icon: Some((1, include_bytes!("../../testapp/res/icon.png").to_vec())),
            },
        ],
        rhtn_desktop::screens::BoardList::default(),
    );
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
            "thread-long.html",
            screens::page(&snap, &Place::Thread("33".repeat(32)), &long),
        ),
        (
            "board.html",
            screens::page(
                &snap,
                &Place::Board("2e".repeat(32), "00".repeat(32)),
                &board,
            ),
        ),
        (
            "subnet.html",
            screens::page(&snap, &Place::Subnet("2e".repeat(32)), &catalog),
        ),
        (
            "options.html",
            screens::page(&snap, &Place::Options, &Body::Plain),
        ),
        (
            "node.html",
            screens::page(&snap, &Place::Node, &Body::Plain),
        ),
        (
            "colour.html",
            screens::page(&snap, &Place::Colour, &Body::Plain),
        ),
        (
            "authority.html",
            screens::page(
                &snap,
                &Place::Authority,
                &Body::Authority(vec![screens::LineRow {
                    line: rhtn_model::Line {
                        patron: [0x33; 32],
                        series: 0,
                        anchor: [0; 32],
                        nibbles: 2,
                    },
                    alias: Some("Carol".into()),
                    by: "70".repeat(32),
                }]),
            ),
        ),
        (
            "subnet-options.html",
            screens::page(&snap, &Place::SubnetOptions("2e".repeat(32)), &Body::Plain),
        ),
    ];
    for (name, html) in pages {
        std::fs::write(format!("{dir}/{name}"), html).unwrap();
    }
}
