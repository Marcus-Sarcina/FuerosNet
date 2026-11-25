//! The streaming face (`resource-requirements.md` §3): a file in parts and
//! a clip in segments, each answer one bounded message, and the page's own
//! check held against the bytes served.

use rhtn_testapp::stream::*;

fn get(s: &mut Stream, path: &str) -> (String, Vec<u8>) {
    let out = s.handle(format!("GET {path} HTTP/1.1\r\nhost: x\r\n\r\n").as_bytes());
    let end = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    (
        String::from_utf8(out[..end].to_vec()).unwrap(),
        out[end + 4..].to_vec(),
    )
}

#[test]
fn every_part_is_one_bounded_answer_and_the_pages_check_is_the_check_of_what_is_served() {
    let mut s = Stream::new();
    let mut carried = 1u32;
    let mut first = Vec::new();
    for n in 0..PARTS {
        let (head, body) = get(&mut s, &format!("/part/{n}"));
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        assert!(head.contains("content-type: application/octet-stream"));
        assert_eq!(body.len(), PART_BYTES);
        assert!(
            body.len() < 8 << 20,
            "a part is under what a node takes from a package in one answer"
        );
        if n == 0 {
            first = body.clone();
        } else {
            assert_ne!(body, first, "no part repeats the first");
        }
        carried = check(carried, &body);
    }
    const {
        assert!(
            PARTS * PART_BYTES > 16 << 20,
            "the whole is larger than one message may be"
        );
    }
    let (_, page) = get(&mut s, "/");
    let page = String::from_utf8(page).unwrap();
    assert!(
        page.contains(&format!("WANT = {carried},")),
        "the page checks the parts against the figure the parts come to"
    );
    assert!(
        get(&mut s, &format!("/part/{PARTS}"))
            .0
            .starts_with("HTTP/1.1 404")
    );
    assert!(get(&mut s, "/part/x").0.starts_with("HTTP/1.1 404"));
}

#[test]
fn the_page_declares_its_script_and_the_clip_is_an_init_segment_and_ten_fragments() {
    let mut s = Stream::new();
    let (head, page) = get(&mut s, "/");
    assert!(head.contains("content-type: text/html"));
    assert!(
        head.contains("content-security-policy:") && head.contains("script-src"),
        "a page that runs says so in its own policy: {head}"
    );
    assert!(String::from_utf8(page).unwrap().contains("avc1.42C00D"));

    let (head, init) = get(&mut s, "/init.mp4");
    assert!(head.contains("content-type: video/mp4"));
    assert_eq!(&init[4..8], b"ftyp");
    assert!(init.windows(4).any(|w| w == b"moov"));
    let mut whole = init.len();
    for n in 0..SEGMENTS.len() {
        let (head, seg) = get(&mut s, &format!("/seg/{n}"));
        assert!(head.starts_with("HTTP/1.1 200"));
        assert_eq!(&seg[4..8], b"moof", "segment {n} opens with its fragment");
        assert!(
            seg.windows(4).any(|w| w == b"tfdt"),
            "and says when it plays"
        );
        whole += seg.len();
    }
    assert!(get(&mut s, "/seg/10").0.starts_with("HTTP/1.1 404"));
    assert_eq!(get(&mut s, "/clip.mp4").1.len(), whole);
}

#[test]
fn the_page_is_told_where_to_aim_a_probe_only_where_the_instance_was_and_only_an_address() {
    let (_, plain) = get(&mut Stream::new(), "/");
    assert!(
        String::from_utf8(plain)
            .unwrap()
            .contains("const PROBE = \"\";")
    );
    let (_, told) = get(&mut Stream::new().probing("192.0.2.7:7462"), "/");
    assert!(
        String::from_utf8(told)
            .unwrap()
            .contains("const PROBE = \"192.0.2.7:7462\";")
    );
    // what is written into a script is an address and nothing else
    let (_, odd) = get(&mut Stream::new().probing("x\";alert(1);//"), "/");
    assert!(
        String::from_utf8(odd)
            .unwrap()
            .contains("const PROBE = \"\";")
    );
}

#[test]
fn what_the_page_measured_is_kept_as_one_line_and_nothing_else_is_taken() {
    let mut s = Stream::new();
    let body = "{\"parts_check\":\"matches\",\"parts_bytes\":33554432}";
    let out = s.handle(
        format!(
            "POST /report HTTP/1.1\r\nhost: x\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
    assert!(out.starts_with(b"HTTP/1.1 200"));
    assert_eq!(
        s.said.last().unwrap(),
        "{\"what\":\"stream.report\",\"got\":{\"parts_check\":\"matches\",\"parts_bytes\":33554432}}"
    );
    let bad = "not json\nsecond line";
    s.handle(
        format!(
            "POST /report HTTP/1.1\r\nhost: x\r\ncontent-length: {}\r\n\r\n{bad}",
            bad.len()
        )
        .as_bytes(),
    );
    assert!(s.said.last().unwrap().contains("\"unreadable\""));
    assert!(
        s.handle(b"DELETE / HTTP/1.1\r\nhost: x\r\n\r\n")
            .starts_with(b"HTTP/1.1 405")
    );
    assert!(
        s.handle(b"PUT /.rhtn/relationships HTTP/1.1\r\nhost: x\r\n\r\n")
            .starts_with(b"HTTP/1.1 204")
    );
    // a face that asks for a live session asks once, before its page, and
    // the page is what answers the approval
    let mut l = rhtn_testapp::stream::Stream::new().living(40);
    let asked = l.handle(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n");
    let asked = String::from_utf8_lossy(&asked).to_string();
    assert!(
        asked.contains("content-type: application/vnd.rhtn.utility+json"),
        "{asked}"
    );
    assert!(asked.contains("\"utility\":\"live\""), "{asked}");
    assert!(asked.contains("\"return\":\"/live\""), "{asked}");
    assert!(asked.contains("\"seconds\":40"), "{asked}");
    let body = b"--b\r\ncontent-disposition: form-data; name=\"outcome\"\r\n\r\ngiven\r\n--b--\r\n";
    let mut post = format!(
        "POST /live HTTP/1.1\r\nhost: x\r\ncontent-type: multipart/form-data; boundary=b\r\ncontent-length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    post.extend_from_slice(body);
    let page = l.handle(&post);
    assert!(String::from_utf8_lossy(&page).contains("Content in parts"));
    // and loaded again, it is the page and not the question
    let again = l.handle(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n");
    assert!(String::from_utf8_lossy(&again).contains("Content in parts"));
    assert!(
        l.said
            .iter()
            .any(|s| s.contains("\"stream.live\",\"given\":true"))
    );

    // and where its host says its media port is reached is kept, and said
    // once each time it moves
    assert_eq!(s.media, None);
    for _ in 0..2 {
        assert!(
            s.handle(
                b"PUT /.rhtn/relationships HTTP/1.1\r\nhost: x\r\nrhtn-media: 203.0.113.7:7462\r\n\r\n"
            )
            .starts_with(b"HTTP/1.1 204")
        );
    }
    assert_eq!(s.media.as_deref(), Some("203.0.113.7:7462"));
    assert_eq!(
        s.said
            .iter()
            .filter(|l| l.contains("stream.media"))
            .collect::<Vec<_>>(),
        vec!["{\"what\":\"stream.media\",\"at\":\"203.0.113.7:7462\"}"]
    );
}

#[test]
fn the_pages_own_button_is_answered_with_a_live_ask_for_the_seconds_it_names() {
    let mut s = Stream::new();
    let (_, page) = get(&mut s, "/");
    let page = String::from_utf8(page).unwrap();
    assert!(page.contains("rhtn.request('POST', '/go-live'"));
    let post = |s: &mut Stream, body: &str| {
        let out = s.handle(
            format!(
                "POST /go-live HTTP/1.1\r\nhost: x\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        );
        String::from_utf8(out).unwrap()
    };
    let asked = post(&mut s, "45");
    assert!(asked.starts_with("HTTP/1.1 200"), "{asked}");
    assert!(asked.contains("content-type: application/vnd.rhtn.utility+json"));
    assert!(asked.contains("\"utility\":\"live\""));
    assert!(asked.contains("\"return\":\"/live\""));
    assert!(asked.contains("\"seconds\":45}"));
    // no number, or one outside 5 to 120: thirty
    for body in ["", "soon", "4", "121", "-1"] {
        assert!(post(&mut s, body).contains("\"seconds\":30}"), "{body}");
    }
    assert!(
        s.said
            .iter()
            .any(|l| l == "{\"what\":\"stream.asked\",\"seconds\":45,\"by\":\"page\"}")
    );
}
