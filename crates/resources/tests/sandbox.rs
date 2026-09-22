//! What a hosted package may reach, and what it costs before it is stopped.

use rhtn_node::resources::{Backend, HOST_EXPORTS};
use rhtn_resources::{HOST_INSTANCE, Hosted, Limits, Refusal, Sandbox};
use rhtn_sim::packages::*;

fn refusal(r: &Result<Sandbox, Refusal>) -> String {
    match r {
        Ok(_) => "admitted".into(),
        Err(e) => e.to_string(),
    }
}

// acceptance: RSC-30
#[test]
fn a_package_reaching_past_the_two_bindings_is_not_admitted() {
    // the control: a package that asks for nothing runs, so the refusals
    // below are about what was asked for
    assert!(
        Sandbox::admit(&silent(), Limits::default()).is_ok(),
        "a package that imports nothing is admitted"
    );
    let ok = Sandbox::admit(&echo(), Limits::default()).expect("the two bindings are offered");
    assert_eq!(
        ok.reaches(),
        HOST_EXPORTS.to_vec(),
        "and it says which, in the manifest's own vocabulary"
    );
    assert!(
        Sandbox::admit(&silent(), Limits::default())
            .expect("admitted")
            .reaches()
            .is_empty(),
        "a package that reaches nothing says so"
    );

    // the datasets §9.2 names, each asked for the way a package would ask
    for elsewhere in [
        "rhtn:host/topology@1.0.0",
        "rhtn:host/liveness@1.0.0",
        "rhtn:host/queue@1.0.0",
        "rhtn:host/prekeys@1.0.0",
        "rhtn:host/roles@1.0.0",
        "wasi:sockets/network@0.2.0",
        "wasi:filesystem/types@0.2.0",
        "wasi:clocks/wall-clock@0.2.0",
        "wasi:random/random@0.2.0",
    ] {
        match Sandbox::admit(&reaching(elsewhere), Limits::default()) {
            Err(Refusal::UnknownImport(n)) => {
                assert_eq!(n, elsewhere, "the refusal names what was asked for")
            }
            other => panic!(
                "{elsewhere} is not a binding this host has: {}",
                refusal(&other)
            ),
        }
    }

    // naming the right instance is not the check
    for hook in ["topology", "queue", "peers"] {
        match Sandbox::admit(&overreaching(hook), Limits::default()) {
            Err(Refusal::UnknownImport(n)) => assert_eq!(
                n,
                format!("{HOST_INSTANCE}#{hook}"),
                "the hook is named, not just the instance"
            ),
            other => panic!(
                "a hook inside the host's own instance is still a hook that does not exist: {}",
                refusal(&other)
            ),
        }
    }

    assert!(
        matches!(
            Sandbox::admit(&mute(), Limits::default()),
            Err(Refusal::NoEntry)
        ),
        "a package the host cannot call is not a package"
    );
    assert!(
        matches!(
            Sandbox::admit(b"\0asm\x01\0\0\0", Limits::default()),
            Err(Refusal::NotAComponent(_))
        ),
        "a core module is not a component"
    );
}

// acceptance: RSC-31
#[test]
fn a_package_is_handed_its_request_and_gives_back_its_answer() {
    let s = Sandbox::admit(&echo(), Limits::default()).expect("admitted");
    let message = b"GET /thing HTTP/1.1\r\nhost: shop.example\r\nrhtn-principal: AAAA\r\nrhtn-roles: reader\r\nrhtn-audience: BBBB\r\nrhtn-session: CCCC\r\n\r\n";
    let back = s.serve(message).expect("served");
    assert_eq!(
        back, message,
        "the package sees the message the node built and nothing else"
    );

    // nothing carries between requests: each one gets its own store, and
    // the bindings offer no other door
    let second = s.serve(b"GET / HTTP/1.1\r\n\r\n").expect("served");
    assert_eq!(
        second, b"GET / HTTP/1.1\r\n\r\n",
        "and the next request is not the last one's"
    );

    // a package that answers nothing has answered nothing, which is not
    // an error the host invents a body for
    let quiet = Sandbox::admit(&silent(), Limits::default()).expect("admitted");
    assert_eq!(quiet.serve(message).expect("served"), Vec::<u8>::new());
}

// acceptance: RSC-32
#[test]
fn a_package_that_will_not_stop_is_stopped_and_costs_the_next_caller_nothing() {
    let limits = Limits {
        fuel: 1_000_000,
        ..Limits::default()
    };
    let s = Sandbox::admit(&spinning(), limits).expect("admitted");
    assert!(
        matches!(s.serve(b"GET / HTTP/1.1\r\n\r\n"), Err(Refusal::Exhausted)),
        "a loop is stopped rather than run"
    );
    assert!(
        matches!(s.serve(b"GET / HTTP/1.1\r\n\r\n"), Err(Refusal::Exhausted)),
        "and stopped again, with its own budget"
    );

    // a package that breaks is a different fact from one that spends
    let b = Sandbox::admit(&broken(), Limits::default()).expect("admitted");
    assert!(
        matches!(b.serve(b"GET / HTTP/1.1\r\n\r\n"), Err(Refusal::Trapped(_))),
        "a trap is a defect, not capacity"
    );

    // and neither costs a package sharing the host anything
    let good = Sandbox::admit(&echo(), Limits::default()).expect("admitted");
    assert_eq!(good.serve(b"ok").expect("served"), b"ok");
}

// acceptance: RSC-33
#[test]
fn a_package_that_grows_without_bound_is_bounded() {
    let limits = Limits {
        memory: 2 << 20,
        ..Limits::default()
    };
    let s = Sandbox::admit(&greedy(), limits).expect("admitted");
    let back = s
        .serve(b"GET / HTTP/1.1\r\n\r\n")
        .expect("it stops rather than trapping");
    let pages = u32::from_le_bytes(back[..4].try_into().unwrap()) as usize;
    assert!(
        pages * 65536 <= limits.memory,
        "it reached {pages} pages, past the {} byte ceiling",
        limits.memory
    );
    assert!(
        pages > 1,
        "and it was allowed to grow at all: {pages} pages"
    );

    // an answer larger than the host carries is refused as an answer, not
    // reported as a defect in the package
    let loud = Sandbox::admit(
        &shouting(4096),
        Limits {
            response: 1024,
            ..Limits::default()
        },
    )
    .expect("admitted");
    assert!(
        matches!(
            loud.serve(b"GET / HTTP/1.1\r\n\r\n"),
            Err(Refusal::Oversized(4096))
        ),
        "the host says how big it was"
    );
}

// acceptance: RSC-34
#[test]
fn a_stopped_package_is_unavailable_rather_than_broken() {
    let h = Hosted::new(Sandbox::admit(&echo(), Limits::default()).expect("admitted"));
    assert!(h.running());
    assert_eq!(
        h.handle(b"GET / HTTP/1.1\r\n\r\n").expect("served"),
        b"GET / HTTP/1.1\r\n\r\n"
    );
    h.stop();
    assert!(
        !h.running(),
        "the gateway asks before it hands anything over"
    );
    assert!(
        h.handle(b"GET / HTTP/1.1\r\n\r\n").is_err(),
        "and a request that reaches it anyway is refused"
    );
}

// acceptance: RSC-35
#[test]
fn what_reaches_a_package_through_the_gateway_is_the_credential_and_the_request() {
    use rhtn_archive::catalog::ResourceRequest;
    use rhtn_archive::topology::Table;
    use rhtn_node::resources::{Binding, Gateway, Row};
    use std::collections::BTreeSet;
    use std::sync::Arc;

    let me = [7u8; 32];
    let resource = [9u8; 32];
    let mut table = Table::with_me(me);
    table.mark_infra(me);

    let mut g = Gateway::default();
    let hosted = Arc::new(Hosted::new(
        Sandbox::admit(&echo(), Limits::default()).expect("admitted"),
    ));
    g.bind(
        resource,
        Binding {
            owner: me,
            authority: "shop.internal".into(),
            backend: Some(hosted),
            declared_roles: BTreeSet::from(["reader".to_string()]),
        },
    );
    g.set_row(
        resource,
        me,
        Row {
            roles: BTreeSet::from(["reader".to_string()]),
            connect: true,
        },
    )
    .expect("a row");

    let req = ResourceRequest {
        resource,
        message: b"GET /orders HTTP/1.1\r\nhost: ignored\r\naccept: application/json\r\n\r\n"
            .to_vec(),
    };
    let answer = g.serve(&me, &table, &me, &req.encode());
    let body = String::from_utf8(answer.body.expect("the package answered")).expect("text");

    // the package echoes what it was handed, so this is the whole of what
    // crossed: the four credential headers `resource-requirements.md` §3
    // lists, the authority the node addressed it by, and the request
    for header in [
        "rhtn-principal:",
        "rhtn-roles: reader",
        "rhtn-audience:",
        "rhtn-session:",
        "host: shop.internal",
    ] {
        assert!(
            body.contains(header),
            "the package is handed {header}, and {body:?} does not carry it"
        );
    }
    assert!(
        body.starts_with("GET /orders HTTP/1.1"),
        "with its own request underneath"
    );
    for absent in [
        "rhtn-topology",
        "rhtn-liveness",
        "rhtn-queue",
        "rhtn-patron",
        "rhtn-keyhash",
    ] {
        assert!(
            !body.contains(absent),
            "and nothing about the network: {absent}"
        );
    }
}
