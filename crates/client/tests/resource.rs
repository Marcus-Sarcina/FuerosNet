//! **What a resource answered, read once** (`light-client-requirements.md`
//! §8; `wire-format.md` §11, §11.2).
//!
//! The parse is in the kernel because a second parser above the boundary
//! is exactly what §11.2 forbids below it: two parties disagreeing about
//! where one message ends and the next begins. So the cases that matter
//! here are the ambiguities — a bare newline, a folded header, two
//! lengths, a length beside an encoding, bytes after the body — each of
//! which is refused rather than normalised.

use rhtn_client::resource::{Refusal, Shape, answer};

fn msg(headers: &[&str], body: &str) -> Vec<u8> {
    let mut out = String::from("HTTP/1.1 200 OK\r\n");
    for h in headers {
        out.push_str(h);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    out.push_str(body);
    out.into_bytes()
}

// acceptance: RSC-56
#[test]
fn a_page_is_the_declared_type_and_never_the_bodys_shape() {
    let a = answer(&msg(
        &[
            "content-type: text/html; charset=UTF-8",
            "content-length: 20",
        ],
        "<p>the standpipe</p>",
    ));
    let a = a.expect("a plain answer");
    assert_eq!(a.http_status, 200);
    assert_eq!(
        a.media.as_deref(),
        Some("text/html"),
        "lower-cased, parameters off"
    );
    assert_eq!(a.charset.as_deref(), Some("utf-8"));
    assert_eq!(a.shape, Shape::Page);

    // **nothing is sniffed.** The same bytes under no declaration are not
    // a page: a body inspected to find out what it is arrives as
    // something the person never agreed to look at.
    let b = answer(&msg(&["content-length: 20"], "<p>the standpipe</p>")).expect("still parses");
    assert_eq!(b.media, None);
    assert_eq!(b.shape, Shape::Other, "undeclared is not html");

    // and a type this client does not render is named, not guessed at
    let c = answer(&msg(
        &["content-type: application/pdf", "content-length: 4"],
        "%PDF",
    ))
    .expect("parses");
    assert_eq!(c.media.as_deref(), Some("application/pdf"));
    assert_eq!(c.shape, Shape::Other);

    // text is not a page, and wants no frame
    let d = answer(&msg(
        &["content-type: text/plain", "content-length: 3"],
        "hey",
    ))
    .expect("parses");
    assert_eq!(d.shape, Shape::Text);
}

// acceptance: RSC-57
#[test]
fn every_way_two_parsers_could_disagree_about_the_body_is_refused() {
    // two lengths
    assert_eq!(
        answer(&msg(&["content-length: 3", "content-length: 4"], "abc")),
        Err(Refusal::Ambiguous("content-length twice")),
    );
    // a length beside an encoding, which is request smuggling's own shape
    assert_eq!(
        answer(&msg(
            &["content-length: 3", "transfer-encoding: chunked"],
            "abc"
        )),
        Err(Refusal::Ambiguous(
            "a content-length beside a transfer-encoding"
        )),
    );
    // an encoding this client does not implement
    assert_eq!(
        answer(&msg(&["transfer-encoding: gzip"], "")),
        Err(Refusal::Ambiguous("a transfer-encoding other than chunked")),
    );
    // two types, so which one decides what may be shown is undecided
    assert_eq!(
        answer(&msg(
            &["content-type: text/html", "content-type: text/plain"],
            ""
        )),
        Err(Refusal::Ambiguous("content-type twice")),
    );
    // bytes after the declared body: the second message
    assert_eq!(
        answer(&msg(&["content-length: 3"], "abcHTTP/1.1 200 OK\r\n\r\n")),
        Err(Refusal::Body("bytes after the declared body")),
    );
    // and fewer than declared, which is a body nobody finished sending
    assert_eq!(
        answer(&msg(&["content-length: 9"], "abc")),
        Err(Refusal::Body("fewer bytes than declared")),
    );
}

// acceptance: RSC-57
#[test]
fn a_folded_header_and_a_bare_newline_are_refused_rather_than_normalised() {
    let folded = b"HTTP/1.1 200 OK\r\ncontent-type: text/html\r\n  ;charset=utf-8\r\n\r\n".to_vec();
    match answer(&folded) {
        Err(Refusal::Header(_)) => {}
        other => panic!("a folded header is not read: {other:?}"),
    }
    let bare = b"HTTP/1.1 200 OK\ncontent-length: 0\n\n".to_vec();
    assert!(
        answer(&bare).is_err(),
        "a message delimited by bare newlines is not this grammar"
    );
    // a header with no colon at all
    match answer(&msg(&["content-type text/html"], "")) {
        Err(Refusal::Header(h)) => assert!(h.contains("content-type"), "{h}"),
        other => panic!("{other:?}"),
    }
    // and a name with space before its colon, the other old disagreement
    match answer(&msg(&["content-length : 0"], "")) {
        Err(Refusal::Header(_)) => {}
        other => panic!("{other:?}"),
    }
}

// acceptance: RSC-56
#[test]
fn only_an_http_eleven_status_line_is_read_and_a_missing_one_is_said_so() {
    assert_eq!(
        answer(b"not an http message at all"),
        Err(Refusal::NoHeaderEnd)
    );
    assert_eq!(answer(b"HTTP/1.0 200 OK\r\n\r\n"), Err(Refusal::StatusLine));
    assert_eq!(answer(b"HTTP/1.1 2x0 OK\r\n\r\n"), Err(Refusal::StatusLine));
    assert_eq!(answer(b"HTTP/1.1 200OK\r\n\r\n"), Err(Refusal::StatusLine));
    // a reason phrase is ignored, and its absence is fine
    assert_eq!(
        answer(b"HTTP/1.1 404\r\n\r\n").expect("parses").http_status,
        404
    );
    // **the application's status, not the node's.** §11's six statuses say
    // whether anything was delivered; this is what the resource said.
    let a = answer(&msg(&["content-type: text/html", "content-length: 0"], "")).expect("parses");
    assert_eq!(a.http_status, 200);
    assert!(a.body.is_empty());
}

// acceptance: RSC-56
#[test]
fn a_chunked_body_is_joined_and_a_malformed_one_is_not() {
    let m = b"HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ntransfer-encoding: chunked\r\n\r\n\
               4\r\nthe \r\n5\r\nrota\n\r\n0\r\n\r\n"
        .to_vec();
    let a = answer(&m).expect("a chunked page");
    assert_eq!(String::from_utf8_lossy(&a.body), "the rota\n");
    assert_eq!(a.shape, Shape::Page);

    // a chunk shorter than it said
    let short =
        b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n9\r\nabc\r\n0\r\n\r\n".to_vec();
    assert_eq!(
        answer(&short),
        Err(Refusal::Body("a chunk shorter than its size"))
    );
    // a size with an extension on it, which is a place parsers have differed
    let ext =
        b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n3;x=1\r\nabc\r\n0\r\n\r\n".to_vec();
    assert_eq!(
        answer(&ext),
        Err(Refusal::Body("a chunk size that is not plain hex"))
    );
    // a trailer section, likewise
    let trailer =
        b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\nx: 1\r\n\r\n"
            .to_vec();
    assert_eq!(
        answer(&trailer),
        Err(Refusal::Body("something after the last chunk"))
    );
    // and nothing after the terminal chunk
    let after = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n0\r\n\r\nmore".to_vec();
    assert_eq!(
        answer(&after),
        Err(Refusal::Body("something after the last chunk"))
    );
}

#[test]
fn with_neither_length_nor_encoding_the_rest_is_the_body() {
    // §11 has the stream carry one answer and close, so there is nothing
    // after it to mistake for a second message
    let a = answer(&msg(&["content-type: text/plain"], "the yard tap")).expect("parses");
    assert_eq!(String::from_utf8_lossy(&a.body), "the yard tap");
    assert_eq!(a.shape, Shape::Text);
}

#[test]
fn every_refusal_says_what_it_was_in_a_sentence() {
    for r in [
        Refusal::NoHeaderEnd,
        Refusal::StatusLine,
        Refusal::Header("x".into()),
        Refusal::Ambiguous("content-length twice"),
        Refusal::Body("fewer bytes than declared"),
    ] {
        let said = r.say();
        assert!(
            said.len() > 12 && said.chars().next().unwrap().is_lowercase(),
            "{said}"
        );
    }
}
