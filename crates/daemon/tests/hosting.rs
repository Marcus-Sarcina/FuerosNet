//! What an operator's hosting file may say, and what it may not.

use rhtn_daemon::hosting::apply;
use rhtn_node::resources::Gateway;
use rhtn_resources::Limits;
use std::path::{Path, PathBuf};

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Dir {
        let d = std::env::temp_dir().join(format!("rhtn-hosting-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("a directory");
        Dir(d)
    }
    fn put(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).expect("written");
        p
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const RES: &str = "0909090909090909090909090909090909090909090909090909090909090909";
const WHO: &str = "0101010101010101010101010101010101010101010101010101010101010101";

/// One `[[host]]` entry naming `manifest`, which most of these start from.
fn host(manifest: &Path) -> String {
    format!(
        "[[host]]\nresource = \"{RES}\"\nowner = \"{WHO}\"\nauthority = \"shop.internal\"\nmanifest = \"{}\"\n",
        manifest.display()
    )
}

fn run(d: &Dir, text: &str) -> Result<usize, String> {
    let f = d.put("hosting", text);
    let mut g = Gateway::default();
    apply(
        &mut g,
        Path::new(&f),
        Limits::default(),
        Some(&d.0.join("grants")),
        Some(&d.0.join("state")),
    )
    .map(|a| a.bound)
    .map_err(|e| e.to_string())
}

/// Write a resource's grants where the node keeps them, which is what an
/// operator's surface rewrites and what `apply` reads.
fn grants_for(d: &Dir, resource: &str, text: &str) {
    let dir = d.0.join("grants");
    std::fs::create_dir_all(&dir).expect("a directory");
    std::fs::write(dir.join(format!("{resource}.toml")), text).expect("written");
}

// acceptance: DMN-20
#[test]
fn a_manifest_that_does_not_describe_its_component_is_refused() {
    let d = Dir::new("manifest");
    d.put("echo.wasm", rhtn_sim::packages::echo());
    let full = d.put(
        "full.manifest",
        "roles = [\"reader\"]\nimports = [\"rhtn/1:request\", \"rhtn/1:response\"]\ncomponent = \"echo.wasm\"\n",
    );
    let quiet = d.put(
        "quiet.manifest",
        "roles = [\"reader\"]\ncomponent = \"echo.wasm\"\n",
    );
    let extra = d.put("extra.manifest", "roles = [\"reader\"]\nimports = [\"rhtn/1:request\", \"rhtn/1:response\", \"rhtn/1:topology\"]\ncomponent = \"echo.wasm\"\n");

    assert_eq!(
        run(&d, &host(&full)),
        Ok(1),
        "a manifest that says what its component does"
    );

    // under-declaring is the case that matters: a manifest an operator
    // reads and believes reaches nothing, over a component that reaches
    let under = run(&d, &host(&quiet)).expect_err("refused");
    assert!(
        under.contains("declares []"),
        "the operator is shown both sides: {under}"
    );
    assert!(
        under.contains("rhtn/1:request"),
        "including what the component actually reaches: {under}"
    );

    // and over-declaring is refused by the host having no such binding,
    // before the component is even read
    let over = run(&d, &host(&extra)).expect_err("refused");
    assert!(
        over.contains("rhtn/1:topology"),
        "the binding it asked for is named: {over}"
    );

    // **a manifest with no component is not a mistake**: it is a resource
    // this node does not run, which is either reached over its own port
    // or brokered entirely (§10.6), and both are covered below.
    for (name, text, wrong) in [
        (
            "twice.manifest",
            "roles = [\"reader\"]\nroles = [\"writer\"]\ncomponent = \"echo.wasm\"\n",
            "duplicate key",
        ),
        (
            "unknown.manifest",
            "storage = \"1G\"\ncomponent = \"echo.wasm\"\n",
            "unknown field",
        ),
        (
            "reserved.manifest",
            "roles = [\"connect\"]\ncomponent = \"echo.wasm\"\n",
            "reserved",
        ),
    ] {
        let m = d.put(name, text);
        let e = run(&d, &host(&m)).expect_err("refused");
        assert!(e.contains(wrong), "{name}: expected {wrong:?}, got {e}");
    }
}

// acceptance: DMN-21
#[test]
fn a_grant_is_checked_against_the_package_before_anything_is_bound() {
    let d = Dir::new("grants");
    d.put("echo.wasm", rhtn_sim::packages::echo());
    let m = d.put(
        "echo.manifest",
        "roles = [\"reader\", \"writer\"]\nimports = [\"rhtn/1:request\", \"rhtn/1:response\"]\ncomponent = \"echo.wasm\"\n",
    );
    let one = host(&m);
    // **a grant to one named party is a predicate like any other**
    // (`infra-client-requirements.md` §10.3: named individuals, inside the
    // membership gate), so there is one shape in the file rather than one
    // for everyone and another for somebody
    let granting = |roles: &str| {
        grants_for(
            &d,
            RES,
            &format!(
                "[[grant]]\nroles = [{roles}]\n[[grant.where]]\nof = \"named\"\nwho = \"{WHO}\"\n"
            ),
        );
        one.clone()
    };

    assert_eq!(
        run(&d, &granting("\"reader\", \"writer\"")),
        Ok(1),
        "roles the package declared"
    );
    assert_eq!(
        run(&d, &granting("")),
        Ok(1),
        "and a grant of no role is one an operator may write: reaching the resource is the grant"
    );

    let e = run(&d, &granting("\"admin\"")).expect_err("refused");
    assert!(
        e.contains("`admin` is not a role this package declared"),
        "{e}"
    );
    for reserved in ["\"connect\"", "\"discover\""] {
        let e = run(&d, &granting(reserved)).expect_err("refused");
        assert!(
            e.contains("reserved for the node"),
            "a grant may not write the node's own role: {e}"
        );
    }

    // **the hosting file is not where grants live**, and one that still
    // carries them says where they went rather than ignoring them
    let e = run(
        &d,
        &format!("{one}\n[[host.grant]]\nroles = [\"reader\"]\n"),
    )
    .expect_err("refused");
    assert!(e.contains("the node's own table"), "{e}");

    // nothing is bound out of a file that is refused anywhere in it
    let f = d.put("hosting", granting("\"admin\""));
    let mut g = Gateway::default();
    assert!(
        apply(
            &mut g,
            Path::new(&f),
            Limits::default(),
            Some(&d.0.join("grants")),
            Some(&d.0.join("state")),
        )
        .is_err()
    );
    assert!(
        g.binding(&[9u8; 32]).is_none(),
        "a file refused at its last line binds nothing from its first"
    );

    // a sound grant again, since the file above left a refused one behind
    // and these cases are about the hosting file rather than the table
    let _ = granting("\"reader\"");

    // **a grant naming a resource nothing hosts is not expressible.**  It
    // was an error the line format could write down; nesting the grant
    // inside the package it grants on removes the case rather than
    // catching it.
    for (text, wrong) in [
        ("[[serve]]\nresource = \"x\"\n", "unknown field `serve`"),
        (
            &format!("[[host]]\nresource = \"{RES}\"\nowner = \"{WHO}\"\nauthority = \"a\"\n"),
            "missing field `manifest`",
        ),
        (
            &format!(
                "[[host]]\nresource = \"XX\"\nowner = \"{WHO}\"\nauthority = \"a\"\nmanifest = \"m\"\n"
            ),
            "64 lower-case hex digits",
        ),
        (&format!("{one}{one}"), "is hosted twice"),
    ] {
        let e = run(&d, text).expect_err("refused");
        assert!(e.contains(wrong), "expected {wrong:?}, got {e}");
    }
}

/// **A package's declared administrative operations** (`resource-
/// requirements.md` §8, §7.3): read from the manifest, bounded at
/// admission, and refused where the node could not draw them.
#[test]
fn a_manifest_declares_administrative_operations_and_their_bounds() {
    let d = Dir::new("admin-ops");
    d.put("echo.wasm", rhtn_sim::packages::echo());
    let good = d.put(
        "shop.manifest",
        "roles = [\"reader\"]\n\
         imports = [\"rhtn/1:request\", \"rhtn/1:response\"]\n\
         component = \"echo.wasm\"\n\
         \n\
         [[admin]]\n\
         name = \"set-greeting\"\n\
         label = \"The greeting on the front page\"\n\
         help = \"What a visitor reads before signing in.\"\n\
         \n\
         [[admin.parameter]]\n\
         name = \"text\"\n\
         label = \"Greeting\"\n\
         type = \"text\"\n\
         max = 120\n\
         \n\
         [[admin.parameter]]\n\
         name = \"shown\"\n\
         label = \"Show it\"\n\
         type = \"flag\"\n",
    );
    // the gateway itself, since what is asserted is what reached the
    // binding rather than how many bound
    let f = d.put("hosting", host(&good));
    let mut g = Gateway::default();
    apply(
        &mut g,
        Path::new(&f),
        Limits::default(),
        Some(&d.0.join("grants")),
        Some(&d.0.join("state")),
    )
    .expect("a manifest with operations binds");
    let bound = g.bound();
    let b = g.binding(&bound[0]).expect("the binding");
    assert_eq!(
        1,
        b.declared.admin.len(),
        "the operation is carried to the binding"
    );
    assert_eq!("set-greeting", b.declared.admin[0].name);
    assert_eq!(2, b.declared.admin[0].parameters.len());
    assert_eq!(
        rhtn_node::resources::Kind::Text { max: 120 },
        b.declared.admin[0].parameters[0].kind,
        "a text field carries the length its package declared"
    );

    // **every bound is checked at admission**, because the node draws all
    // of this and a field it cannot draw is one an operator cannot answer
    for (name, body, wrong) in [
        (
            "nolabel.manifest",
            "component = \"echo.wasm\"\n[[admin]]\nname = \"x\"\nlabel = \"\"\n",
            "a label is 1 to",
        ),
        (
            "longtext.manifest",
            "component = \"echo.wasm\"\n[[admin]]\nname = \"x\"\nlabel = \"X\"\n\
             [[admin.parameter]]\nname = \"t\"\nlabel = \"T\"\ntype = \"text\"\nmax = 99999\n",
            "a text field takes 1 to",
        ),
        (
            "unbounded.manifest",
            "component = \"echo.wasm\"\n[[admin]]\nname = \"x\"\nlabel = \"X\"\n\
             [[admin.parameter]]\nname = \"t\"\nlabel = \"T\"\ntype = \"text\"\n",
            "names `max`",
        ),
        (
            "notatype.manifest",
            "component = \"echo.wasm\"\n[[admin]]\nname = \"x\"\nlabel = \"X\"\n\
             [[admin.parameter]]\nname = \"t\"\nlabel = \"T\"\ntype = \"blob\"\n",
            "not a parameter type",
        ),
        (
            "backwards.manifest",
            "component = \"echo.wasm\"\n[[admin]]\nname = \"x\"\nlabel = \"X\"\n\
             [[admin.parameter]]\nname = \"n\"\nlabel = \"N\"\ntype = \"number\"\nlow = 9\nhigh = 1\n",
            "is not below",
        ),
        (
            "twice.manifest",
            "component = \"echo.wasm\"\n[[admin]]\nname = \"x\"\nlabel = \"X\"\n\
             [[admin]]\nname = \"x\"\nlabel = \"Y\"\n",
            "declared twice",
        ),
    ] {
        let m = d.put(name, body);
        let e = run(&d, &host(&m)).expect_err("refused");
        assert!(e.contains(wrong), "{name}: expected {wrong:?}, got {e}");
    }
}

/// **A package ships its grants ready-made** (`infra-client-requirements.md`
/// §10.4), written in the same clause vocabulary an operator's own grant
/// is, so that what a one-click choice grants is legible before the click.
/// Read, bounded, and carried to the binding — and never applied.
#[test]
fn a_manifest_ships_templates_in_the_operators_own_vocabulary() {
    let d = Dir::new("templates");
    d.put("echo.wasm", rhtn_sim::packages::echo());
    let head = "roles = [\"reader\", \"writer\"]\n\
                imports = [\"rhtn/1:request\", \"rhtn/1:response\"]\n\
                component = \"echo.wasm\"\n";
    let good = d.put(
        "shop.manifest",
        format!(
            "{head}\n\
             [[template]]\n\
             name = \"org-read\"\n\
             label = \"Everyone in my org may read\"\n\
             roles = [\"reader\"]\n\
             \n\
             [[template.where]]\n\
             of = \"grandclients\"\n\
             \n\
             [[template]]\n\
             name = \"trusted-write\"\n\
             label = \"My ten most trusted may write\"\n\
             roles = [\"reader\", \"writer\"]\n\
             \n\
             [[template.where]]\n\
             of = \"most-trusted\"\n\
             n = 10\n"
        ),
    );
    let f = d.put("hosting", host(&good));
    let mut g = Gateway::default();
    apply(
        &mut g,
        Path::new(&f),
        Limits::default(),
        Some(&d.0.join("grants")),
        Some(&d.0.join("state")),
    )
    .expect("a manifest with templates binds");
    let bound = g.bound();
    let b = g.binding(&bound[0]).expect("the binding");
    assert_eq!(2, b.declared.templates.len(), "both reached the binding");
    assert_eq!("org-read", b.declared.templates[0].name);
    assert_eq!(
        "reader to my clients and grand-clients",
        b.declared.templates[0].grant.to_string(),
        "§10.4: legible in the vocabulary the operator uses elsewhere"
    );
    assert_eq!(
        "reader, writer to my 10 most trusted",
        b.declared.templates[1].grant.to_string()
    );

    // **offered, not applied**: §10.4 has shrink-wrapping mean trusting
    // the author's judgment about access, which is the operator's to give
    assert!(
        g.grants_for(&bound[0]).is_empty() && g.rows().is_empty(),
        "installing a package grants nobody anything"
    );
    assert_eq!(
        vec![("reader".to_string(), false), ("writer".to_string(), false)],
        g.role_bindings(&bound[0]),
        "§7: both roles declared, neither bound to anyone yet"
    );

    // every bound and every clause is checked at admission
    for (name, body, wrong) in [
        (
            "nolabel.manifest",
            "[[template]]\nname = \"t\"\nroles = [\"reader\"]\n",
            "`label` is not set",
        ),
        (
            "nothing.manifest",
            "[[template]]\nname = \"t\"\nlabel = \"T\"\nroles = []\n",
            "grants nothing",
        ),
        (
            "undeclared.manifest",
            "[[template]]\nname = \"t\"\nlabel = \"T\"\nroles = [\"admin\"]\n",
            "not a role this package declared",
        ),
        (
            "badname.manifest",
            "[[template]]\nname = \"Org Read\"\nlabel = \"T\"\nroles = [\"reader\"]\n",
            "[a-z0-9_-]",
        ),
        (
            "unknown-clause.manifest",
            "[[template]]\nname = \"t\"\nlabel = \"T\"\nroles = [\"reader\"]\n\
             [[template.where]]\nof = \"everyone\"\n",
            "is not a clause",
        ),
        (
            "wrong-key.manifest",
            "[[template]]\nname = \"t\"\nlabel = \"T\"\nroles = [\"reader\"]\n\
             [[template.where]]\nof = \"clients\"\nn = 3\n",
            "takes no other key",
        ),
        (
            "missing-key.manifest",
            "[[template]]\nname = \"t\"\nlabel = \"T\"\nroles = [\"reader\"]\n\
             [[template.where]]\nof = \"distance\"\n",
            "names `edges`",
        ),
        (
            "twice.manifest",
            "[[template]]\nname = \"t\"\nlabel = \"T\"\nroles = [\"reader\"]\n\
             [[template]]\nname = \"t\"\nlabel = \"T\"\nroles = [\"writer\"]\n",
            "declared twice",
        ),
    ] {
        let m = d.put(name, format!("{head}{body}"));
        let e = run(&d, &host(&m)).expect_err("refused");
        assert!(e.contains(wrong), "{name}: expected {wrong:?}, got {e}");
    }
}

/// The same vocabulary from the operator's side, including the one clause
/// that carries a party: a keyhash the file got wrong is refused with the
/// rest of the file (`infra-client-requirements.md` §10.3).
#[test]
fn an_operators_grant_is_written_in_clauses_and_refused_as_a_whole() {
    let d = Dir::new("op-clauses");
    d.put("echo.wasm", rhtn_sim::packages::echo());
    let m = d.put(
        "echo.manifest",
        "roles = [\"reader\"]\n\
         imports = [\"rhtn/1:request\", \"rhtn/1:response\"]\n\
         component = \"echo.wasm\"\n",
    );
    let one = host(&m);
    let with = |clause: &str| {
        grants_for(
            &d,
            RES,
            &format!("[[grant]]\nroles = [\"reader\"]\n[[grant.where]]\n{clause}"),
        );
        one.clone()
    };
    for clause in [
        "of = \"clients\"\n",
        "of = \"grandclients\"\n",
        "of = \"distance\"\nedges = 2\n",
        "of = \"most-trusted\"\nn = 10\n",
        "of = \"top-fraction\"\npercent = 20\n",
        "of = \"joined-before\"\nwhen = 1690000000\n",
        &format!("of = \"named\"\nwho = \"{WHO}\"\n"),
    ] {
        assert_eq!(
            run(&d, &with(clause)),
            Ok(1),
            "§10.3's vocabulary, as written: {clause}"
        );
    }
    for (clause, wrong) in [
        (
            "of = \"named\"\nwho = \"beef\"\n",
            "64 lower-case hex digits",
        ),
        ("of = \"named\"\n", "64 lower-case hex digits"),
        ("of = \"top-fraction\"\npercent = 0\n", "1 to 100"),
        ("of = \"distance\"\nedges = 9\n", "further than two edges"),
        ("of = \"clients\"\nwho = \"x\"\n", "takes no other key"),
    ] {
        let e = run(&d, &with(clause)).expect_err("refused");
        assert!(e.contains(wrong), "expected {wrong:?}, got {e}");
    }
}

/// **A resource that holds its own port**, reached over
/// `resource-requirements.md` §3's second leg: the node authenticates, the
/// credential of §2 travels as request headers, and what comes back is the
/// resource's own answer relayed unread (§3: "the node relays them; it
/// does not interpret them").
#[test]
fn a_resource_that_holds_its_own_port_is_reached_over_a_socket() {
    use rhtn_archive::topology::Table;
    use rhtn_node::grant::Standing;
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    // the resource: an ordinary HTTP server, which knows nothing of this
    // network beyond the headers it is handed
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("a port of its own");
    let addr = l.local_addr().expect("its address");
    let got = Arc::new(Mutex::new(String::new()));
    let keep = got.clone();
    let serving = std::thread::spawn(move || {
        let (mut c, _) = l.accept().expect("the node connects");
        let mut buf = [0u8; 8192];
        let n = c.read(&mut buf).expect("the request arrives");
        *keep.lock().unwrap() = String::from_utf8_lossy(&buf[..n]).to_string();
        // chunked, so the relay's re-framing is exercised too
        c.write_all(
            b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ntransfer-encoding: chunked\r\n\r\n\
              5\r\nhello\r\n0\r\n\r\n",
        )
        .expect("answered");
    });

    let d = Dir::new("relay");
    let m = d.put("front.manifest", "roles = [\"reader\"]\n");
    let f = d.put(
        "hosting",
        format!(
            "[[host]]\nresource = \"{RES}\"\nowner = \"{WHO}\"\nauthority = \"shop.internal\"\n\
             manifest = \"{}\"\naddress = \"{addr}\"\n",
            m.display()
        ),
    );
    grants_for(&d, RES, "[[grant]]\nroles = [\"reader\"]\n");
    let mut g = Gateway::default();
    apply(
        &mut g,
        Path::new(&f),
        Limits::default(),
        Some(&d.0.join("grants")),
        Some(&d.0.join("state")),
    )
    .expect("an address binds");
    let resource = g.bound()[0];
    assert!(
        g.binding(&resource).expect("bound").backend.is_some(),
        "§10.6: the node carries the traffic, so it is hosted rather than brokered"
    );

    // the owner is in its own horizon, so one party is enough to serve
    let owner = [1u8; 32];
    let table = Table::with_me(owner);
    g.refresh(&table, &Standing::unknown());
    let request = rhtn_archive::catalog::ResourceRequest {
        resource,
        message: b"GET /things HTTP/1.1\r\nhost: anything\r\nrhtn-roles: admin\r\n\r\n".to_vec(),
    }
    .encode();
    let reply = g.serve(&owner, &table, &owner, &request);
    serving.join().expect("the resource answered");

    assert_eq!(0, reply.status, "the request reached the resource");
    let body = String::from_utf8_lossy(&reply.body.expect("a response")).to_string();
    assert!(body.starts_with("HTTP/1.1 200 OK"), "{body}");
    assert!(
        body.contains("content-length: 5") && body.ends_with("hello"),
        "a chunked answer is re-framed with the count it has: {body}"
    );
    assert!(
        !body.to_ascii_lowercase().contains("transfer-encoding"),
        "and the framing it no longer has is gone: {body}"
    );

    // what the resource was handed: §2's credential, and §3.1's hygiene
    let seen = got.lock().unwrap().clone();
    assert!(seen.starts_with("GET /things HTTP/1.1\r\n"), "{seen}");
    for header in [
        "rhtn-principal:",
        "rhtn-roles: reader",
        "rhtn-audience:",
        "rhtn-session:",
    ] {
        assert!(seen.contains(header), "expected {header:?} in {seen}");
    }
    assert!(
        !seen.contains("rhtn-roles: admin"),
        "§3.1: the caller's own `rhtn-*` header is removed before the node inserts its own: {seen}"
    );
    assert!(
        seen.contains("host: shop.internal"),
        "the authority the operator bound, not the one the caller wrote: {seen}"
    );
}

/// An address and a component are not a choice to be made quietly: §10.6
/// has the hosting model follow from where the resource runs, and both at
/// once describes two places.
#[test]
fn an_address_and_a_component_at_once_is_refused() {
    let d = Dir::new("relay-both");
    d.put("echo.wasm", rhtn_sim::packages::echo());
    let m = d.put(
        "both.manifest",
        "roles = [\"reader\"]\nimports = [\"rhtn/1:request\", \"rhtn/1:response\"]\ncomponent = \"echo.wasm\"\n",
    );
    let e = run(&d, &format!("{}address = \"127.0.0.1:9\"\n", host(&m))).expect_err("refused");
    assert!(e.contains("is not one it runs"), "{e}");

    // and an address that is not one is refused for being that, with the
    // component out of the way
    let bare = d.put("bare.manifest", "roles = [\"reader\"]\n");
    let e = run(
        &d,
        &format!("{}address = \"not-an-address\"\n", host(&bare)),
    )
    .expect_err("refused");
    assert!(e.contains("is not an address"), "{e}");
}

/// A manifest with no component at all and no address: the node
/// authenticates and the traffic goes elsewhere (design §11.7, §10.6).
#[test]
fn a_resource_with_neither_component_nor_address_is_brokered() {
    let d = Dir::new("brokered");
    let m = d.put("away.manifest", "roles = [\"reader\"]\n");
    let f = d.put("hosting", host(&m));
    let mut g = Gateway::default();
    apply(
        &mut g,
        Path::new(&f),
        Limits::default(),
        Some(&d.0.join("grants")),
        Some(&d.0.join("state")),
    )
    .expect("a brokered resource binds");
    let b = g.binding(&g.bound()[0]).expect("bound");
    assert!(
        b.backend.is_none(),
        "§10.6: nothing of the caller's traffic passes through this node"
    );
}

/// **Plain HTTP only on a local socket** (`resource-requirements.md` §3).
/// The node already read the request, so what the far leg protects is
/// everyone else; until that leg can be given TLS, an address across a
/// network is refused rather than carried.
#[test]
fn an_address_across_a_network_is_refused_while_the_leg_is_plain() {
    let d = Dir::new("relay-remote");
    let m = d.put("front.manifest", "roles = [\"reader\"]\n");
    for away in ["10.0.0.4:8080", "[2001:db8::1]:8080", "0.0.0.0:8080"] {
        let e = run(&d, &format!("{}address = \"{away}\"\n", host(&m))).expect_err("refused");
        assert!(e.contains("requires HTTPS"), "{away}: {e}");
    }
    for home in ["127.0.0.1:8080", "[::1]:8080"] {
        assert_eq!(
            run(&d, &format!("{}address = \"{home}\"\n", host(&m))),
            Ok(1),
            "a local socket is the one place §3 permits plain HTTP: {home}"
        );
    }
}

/// **Two resources at one address is a collision.** Nothing partitions
/// ports across package authors, so where the operator names the address
/// the file is the only place the clash can be seen — and a node that
/// dialled the same socket for two resources would have one of them
/// answering for both.
#[test]
fn two_resources_at_one_address_are_refused() {
    let d = Dir::new("relay-clash");
    let m = d.put("front.manifest", "roles = [\"reader\"]\n");
    let other = "0808080808080808080808080808080808080808080808080808080808080808";
    let entry = |res: &str, port: u16| {
        format!(
            "[[host]]\nresource = \"{res}\"\nowner = \"{WHO}\"\nauthority = \"a{port}.internal\"\n\
             manifest = \"{}\"\naddress = \"127.0.0.1:{port}\"\n",
            m.display()
        )
    };
    assert_eq!(
        run(&d, &format!("{}{}", entry(RES, 8081), entry(other, 8082))),
        Ok(2),
        "two resources, two addresses"
    );
    let e = run(&d, &format!("{}{}", entry(RES, 8081), entry(other, 8081))).expect_err("refused");
    assert!(e.contains("already listens"), "{e}");
}

/// **A grant written out and read back is the grant it was**, for every
/// clause of the vocabulary.
///
/// This is what keeps an act honest: the table in force is what the
/// operator made, and the file is what a restart will read, so a clause
/// that did not survive the round trip would be a grant that quietly
/// changed meaning when the node restarted.
#[test]
fn every_clause_survives_being_written_and_read_back() {
    use rhtn_node::grant::{Clause, Grant};
    let d = Dir::new("round-trip");
    let dir = d.0.join("grants");
    let resource = [9u8; 32];
    let roles = |r: &[&str]| r.iter().map(|x| x.to_string()).collect();

    let grants = vec![
        Grant::standing(roles(&["reader"])),
        Grant {
            roles: roles(&["reader", "writer"]),
            clauses: vec![Clause::Clients],
        },
        Grant {
            roles: roles(&["writer"]),
            clauses: vec![Clause::Grandclients],
        },
        Grant {
            roles: roles(&["reader"]),
            clauses: vec![Clause::AtDistance { edges: 2 }],
        },
        Grant {
            roles: roles(&["reader"]),
            clauses: vec![Clause::MostTrusted { n: 10 }],
        },
        Grant {
            roles: roles(&["reader"]),
            clauses: vec![Clause::TopFraction { percent: 20 }],
        },
        Grant {
            roles: roles(&["reader"]),
            clauses: vec![Clause::JoinedBefore {
                when: 1_690_000_000,
            }],
        },
        Grant {
            roles: roles(&["reader"]),
            clauses: vec![Clause::Named { who: [7u8; 32] }],
        },
        // and a conjunction, which is §7.1's one compound affordance
        Grant {
            roles: roles(&["writer"]),
            clauses: vec![
                Clause::AtDistance { edges: 1 },
                Clause::MostTrusted { n: 3 },
            ],
        },
    ];
    rhtn_daemon::grants::write_for(&dir, &resource, &grants).expect("written");
    assert_eq!(
        grants,
        rhtn_daemon::grants::read_for(&dir, &resource).expect("read back"),
        "every clause, and the conjunction, came back as it went out"
    );

    // an absent file is a resource nobody has been granted anything on,
    // which `resource-requirements.md` §7 makes the ordinary state of one
    // just hosted rather than a fault
    assert!(
        rhtn_daemon::grants::read_for(&dir, &[1u8; 32])
            .expect("no file is no grants")
            .is_empty()
    );

    // and the file says whose it is, because an operator will find it
    let text = std::fs::read_to_string(dir.join(format!("{}.toml", "09".repeat(32))))
        .expect("named by its resource");
    assert!(text.contains("Written by the node"), "{text}");
}

/// **The directive plane across two instances one node hosts**
/// (`rhtn_node::federation`; `resource-design/directive-surface.md`): the
/// hosting file names the relationships, each instance is an ordinary
/// HTTP origin on a port of its own, and one pass carries the patron's
/// directive into the subordinate's inbox and the subordinate's receipt
/// back. A member's request naming a node path never reaches either.
#[test]
fn a_directive_crosses_between_two_hosted_instances_and_a_member_cannot_speak_on_the_nodes_paths() {
    use rhtn_archive::topology::Table;
    use rhtn_node::federation::{Addresses, Envelope, SCHEMA_DIRECTIVE, SCHEMA_RECEIPT};
    use rhtn_node::grant::Standing;
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    const PATRON: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
    const SUB: &str = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";
    let patron_k: [u8; 32] = [0x0a; 32];
    let sub_k: [u8; 32] = [0x0b; 32];

    /// An origin server that answers every request from a script and
    /// keeps what it was handed.
    fn origin(
        answers: Vec<(&'static str, String)>,
    ) -> (
        std::net::SocketAddr,
        Arc<Mutex<Vec<String>>>,
        std::thread::JoinHandle<()>,
    ) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
        let addr = l.local_addr().unwrap();
        let got = Arc::new(Mutex::new(Vec::new()));
        let keep = got.clone();
        let h = std::thread::spawn(move || {
            for _ in 0..answers.len() {
                let (mut c, _) = l.accept().expect("the node connects");
                let mut buf = vec![0u8; 65536];
                let n = c.read(&mut buf).expect("a request");
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = text.lines().next().unwrap_or("").to_string();
                keep.lock().unwrap().push(text);
                let body = answers
                    .iter()
                    .find(|(starts, _)| line.starts_with(starts))
                    .map(|(_, b)| b.clone())
                    .unwrap_or_default();
                let _ = c.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            }
        });
        (addr, got, h)
    }

    let directive = Envelope::live(
        patron_k,
        "policy-1",
        SCHEMA_DIRECTIVE,
        1,
        b"{\"retention_floor_days\":90}",
    );
    let receipt = Envelope::live(
        sub_k,
        "receipt-policy-1",
        SCHEMA_RECEIPT,
        1,
        b"{\"applied\":true}",
    );
    // the patron: told its relationships, then asked for its outbox
    let (pa, pgot, pj) = origin(vec![
        ("PUT /.rhtn/relationships", String::new()),
        (
            "GET /.rhtn/outbox",
            serde_json::json!({"emit": [{"to": "subordinates", "envelope": directive.to_json()}]})
                .to_string(),
        ),
    ]);
    // the subordinate: told its relationships, polled, handed the
    // directive (and answers with the receipt), polled again
    let (sa, sgot, sj) = origin(vec![
        ("PUT /.rhtn/relationships", String::new()),
        ("GET /.rhtn/outbox", "{\"emit\":[]}".into()),
        (
            "PUT /.rhtn/envelope",
            serde_json::json!({"emit": [{"to": "patron", "envelope": receipt.to_json()}]})
                .to_string(),
        ),
        ("GET /.rhtn/outbox", "{\"emit\":[]}".into()),
    ]);

    let d = Dir::new("federation");
    let m = d.put("front.manifest", "roles = [\"reader\"]\n");
    let f = d.put(
        "hosting",
        format!(
            "[[host]]\nresource = \"{PATRON}\"\nowner = \"{WHO}\"\nauthority = \"hall.internal\"\n\
             manifest = \"{m}\"\naddress = \"{pa}\"\nsubordinates = [\"{SUB}\"]\n\n\
             [[host]]\nresource = \"{SUB}\"\nowner = \"{WHO}\"\nauthority = \"club.internal\"\n\
             manifest = \"{m}\"\naddress = \"{sa}\"\npatron = \"{PATRON}\"\n[host.kin]\nguild = [\"{PATRON}\"]\n",
            m = m.display()
        ),
    );
    grants_for(&d, SUB, "[[grant]]\nroles = [\"reader\"]\n");
    let mut g = Gateway::default();
    apply(
        &mut g,
        Path::new(&f),
        Limits::default(),
        Some(&d.0.join("grants")),
        Some(&d.0.join("state")),
    )
    .expect("binds");
    let rs = rhtn_daemon::hosting::relationships(Path::new(&f)).expect("relationships read");
    assert_eq!(rs.len(), 2);
    let mut fed = rhtn_node::federation::Federation::default();
    for (res, r) in rs {
        fed.configure(res, r).expect("configured");
    }
    assert_eq!(
        fed.plane(&sub_k)
            .unwrap()
            .relationships
            .relation_of(&patron_k),
        Some(rhtn_node::federation::Relation::Patron),
        "a patron is answered before the kin group that also names it"
    );

    // **the rounds, as the daemon runs them**
    // (`rhtn_daemon::federation::rounds`): plan against the gateway,
    // perform the exchanges, commit. They come back to be performed
    // because nothing may be handed to a backend while the node's own
    // state is locked.
    let round = |fed: &mut rhtn_node::federation::Federation| {
        let answers: Vec<rhtn_node::federation::Answer> = fed
            .work(&g)
            .into_iter()
            .map(|e| {
                let answer = e
                    .backend
                    .handle(&e.message)
                    .map(|bytes| rhtn_node::federation::response_of(&bytes));
                rhtn_node::federation::Answer {
                    exchange: e,
                    answer,
                }
            })
            .collect();
        fed.done(answers)
    };
    // the first round tells each instance its relationships, which it
    // takes before anything else reaches it
    let one = round(&mut fed);
    assert_eq!((one.delivered, one.carried), (0, 0), "{one:?}");
    // then the patron's outbox emits the directive, which crosses into
    // the subordinate's plane
    let two = round(&mut fed);
    assert_eq!((two.emitted, two.carried), (1, 1), "{two:?}");
    assert!(two.refused.is_empty(), "{two:?}");
    // and the subordinate is handed it, answering with its receipt
    let three = round(&mut fed);
    assert_eq!(
        (three.delivered, three.emitted, three.carried),
        (1, 1, 1),
        "{three:?}"
    );
    assert_eq!(
        fed.plane(&patron_k).unwrap().waiting(),
        1,
        "the receipt waits for the patron"
    );

    // the gateway is what said where to reach it
    assert_eq!(
        g.reachable(&sub_k).map(|(a, _)| a).as_deref(),
        Some("club.internal")
    );
    sj.join().unwrap();
    let handed = sgot.lock().unwrap().clone();
    let delivery = handed
        .iter()
        .find(|t| t.starts_with("PUT /.rhtn/envelope"))
        .expect("the subordinate was handed the directive");
    assert!(delivery.contains("host: club.internal"), "{delivery}");
    assert!(delivery.contains("rhtn-relation: patron"), "{delivery}");
    assert!(
        delivery.contains(&format!("rhtn-origin: {PATRON}")),
        "{delivery}"
    );
    assert!(delivery.contains("rhtn-audience:"), "{delivery}");
    let body = delivery.split("\r\n\r\n").nth(1).unwrap();
    let got = Envelope::from_json(&serde_json::from_str(body).unwrap()).unwrap();
    assert_eq!(got.record, "policy-1");
    assert_eq!(
        got.path,
        vec![patron_k],
        "the path names the instance it crossed from"
    );
    let told = handed
        .iter()
        .find(|t| t.starts_with("PUT /.rhtn/relationships"))
        .unwrap();
    assert!(
        told.contains(PATRON) && told.contains("\"guild\""),
        "{told}"
    );

    // a member of the subordinate's owner asking the node's own path is
    // refused before anything is connected: the server has had its four
    // exchanges and is gone, and a fifth connect would fail loudly, so
    // the refusal here is the gateway's own
    let owner = [1u8; 32];
    let table = Table::with_me(owner);
    g.refresh(&table, &Standing::unknown());
    let request = rhtn_archive::catalog::ResourceRequest {
        resource: sub_k,
        message: b"GET /.rhtn/outbox HTTP/1.1\r\nhost: x\r\n\r\n".to_vec(),
    }
    .encode();
    let reply = g.serve(&owner, &table, &owner, &request);
    assert_eq!(
        reply.status,
        rhtn_archive::catalog::STATUS_REFUSED,
        "a member's request on a node path is refused"
    );
    let plain = rhtn_archive::catalog::ResourceRequest {
        resource: sub_k,
        message: b"GET /.rhtnot HTTP/1.1\r\nhost: x\r\n\r\n".to_vec(),
    }
    .encode();
    // the prefix is `/.rhtn/`, so a path that merely begins with the
    // letters is an ordinary one — this one reaches a server that is gone,
    // which is the unavailable code, not the refusal
    let reply = g.serve(&owner, &table, &owner, &plain);
    assert_ne!(reply.status, rhtn_archive::catalog::STATUS_REFUSED);
    pj.join().unwrap();
    let _ = pgot;
}

// acceptance: RSC-59
/// **A real package whose answer is not a response reaches nobody**
/// [found 2026-10-28].
///
/// This is the case that found the gap: `echo` hands back exactly the
/// message it was given, a conforming fixture for the sandbox's own ABI
/// test and not a response at all — and until the gateway read what a
/// package answered with, those bytes went to the asking client entire.
/// A request arriving where a response belongs is the mildest version of
/// `wire-format.md` §11.2's *more than one message*; the point is that
/// the node now looks.
#[test]
fn a_package_answering_with_something_that_is_not_a_response_is_unavailable() {
    use rhtn_archive::topology::Table;
    use rhtn_node::grant::Standing;

    let d = Dir::new("notaresponse");
    d.put("echo.wasm", rhtn_sim::packages::echo());
    let m = d.put(
        "echo.manifest",
        "roles = [\"reader\"]\nimports = [\"rhtn/1:request\", \"rhtn/1:response\"]\ncomponent = \"echo.wasm\"\n",
    );
    let f = d.put("hosting", host(&m));
    grants_for(&d, RES, "[[grant]]\nroles = [\"reader\"]\n");
    let mut g = Gateway::default();
    apply(
        &mut g,
        Path::new(&f),
        Limits::default(),
        Some(&d.0.join("grants")),
        Some(&d.0.join("state")),
    )
    .expect("the package binds");
    let resource = g.bound()[0];
    let owner = [1u8; 32];
    let table = Table::with_me(owner);
    g.refresh(&table, &Standing::unknown());
    let request = rhtn_archive::catalog::ResourceRequest {
        resource,
        message: b"GET /things HTTP/1.1\r\nhost: anything\r\n\r\n".to_vec(),
    }
    .encode();

    let reply = g.serve(&owner, &table, &owner, &request);
    assert_eq!(
        reply.status, 2,
        "the package ran and answered; what it answered with is not a response, \
         so the asker is told the resource is unavailable"
    );
    assert!(reply.body.is_none(), "and none of it travels");

    // the same package wrapped in a response is delivered, so this is the
    // answer being read and not the package being rejected
    let d2 = Dir::new("isaresponse");
    d2.put("echo.wasm", rhtn_sim::packages::replies());
    let m2 = d2.put(
        "echo.manifest",
        "roles = [\"reader\"]\nimports = [\"rhtn/1:request\", \"rhtn/1:response\"]\ncomponent = \"echo.wasm\"\n",
    );
    let f2 = d2.put("hosting", host(&m2));
    grants_for(&d2, RES, "[[grant]]\nroles = [\"reader\"]\n");
    let mut g2 = Gateway::default();
    apply(
        &mut g2,
        Path::new(&f2),
        Limits::default(),
        Some(&d2.0.join("grants")),
        Some(&d2.0.join("state")),
    )
    .expect("the package binds");
    g2.refresh(&table, &Standing::unknown());
    let ok = g2.serve(&owner, &table, &owner, &request);
    assert_eq!(
        ok.status, 0,
        "a package that answers with a response is delivered"
    );
    let body = ok.body.expect("a body");
    assert!(String::from_utf8_lossy(&body).starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(
        String::from_utf8_lossy(&body).contains("rhtn-principal:"),
        "and the credentialed request it was handed is still readable in the body",
    );
}

/// **The manifest names the host surface it tolerates, what it asks to be
/// bounded at, and where it may fetch from** (`resource-requirements.md`
/// §8, `infra-client-requirements.md` §9.3): a package tolerating none of
/// this host's versions is refused at installation with both lists named;
/// a block is provisioned under the node's state directory at the budget
/// the package declared, and the instance is to be told where; a fetch
/// destination with a path, a wildcard or the node's own machine in it is
/// refused; and an instance key that is not 32 bytes of hex is refused.
// acceptance: DMN-35
#[test]
fn a_manifest_declares_its_host_version_its_budget_and_its_destinations() {
    let d = Dir::new("surface");
    d.put("echo.wasm", rhtn_sim::packages::echo());
    let future = d.put(
        "future.manifest",
        "roles = [\"reader\"]\nimports = [\"rhtn/1:request\", \"rhtn/1:response\"]\ncomponent = \"echo.wasm\"\nhost = [\"rhtn/host@2\", \"rhtn/host@3\"]\n",
    );
    let why = run(&d, &host(&future)).expect_err("refused");
    assert!(
        why.contains("tolerates") && why.contains("rhtn/host@1"),
        "{why}"
    );
    let tolerant = d.put(
        "tolerant.manifest",
        "roles = [\"reader\"]\nimports = [\"rhtn/1:request\", \"rhtn/1:response\"]\ncomponent = \"echo.wasm\"\nhost = [\"rhtn/host@1\", \"rhtn/host@2\"]\n",
    );
    assert_eq!(
        run(&d, &host(&tolerant)),
        Ok(1),
        "one version in common is enough"
    );

    // a block at the declared budget, provisioned where the node keeps state
    d.put("noting.wasm", rhtn_sim::packages::noting());
    let noting = d.put(
        "noting.manifest",
        "roles = [\"reader\"]\nimports = [\"rhtn/1:request\", \"rhtn/1:response\", \"rhtn/1:block\", \"rhtn/1:log\", \"rhtn/1:clock\", \"rhtn/1:fetch\"]\ncomponent = \"noting.wasm\"\nfetch = [\"https://feeds.example\"]\n[budget]\nblock_bytes = 4096\nlog_bytes = 512\n",
    );
    let f = d.put("hosting", host(&noting));
    let mut g = Gateway::default();
    let applied = apply(
        &mut g,
        Path::new(&f),
        Limits::default(),
        Some(&d.0.join("grants")),
        Some(&d.0.join("state")),
    )
    .expect("bound");
    assert_eq!(applied.bound, 1);
    assert!(
        d.0.join("state").join("blocks").join(RES).is_dir(),
        "the block exists"
    );
    let (_, facts) = &applied.facts[0];
    let get = |n: &str| facts.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    assert_eq!(get("rhtn-host").as_deref(), Some("rhtn/host@1"));
    assert!(get("rhtn-block").unwrap().ends_with(RES));
    assert_eq!(get("rhtn-block-bytes").as_deref(), Some("4096"));
    assert!(get("rhtn-log").unwrap().ends_with(&format!("{RES}.log")));
    assert_eq!(get("rhtn-log-bytes").as_deref(), Some("512"));
    // and with no state directory, a node cannot host it, and says so
    let mut g2 = Gateway::default();
    let why = apply(&mut g2, Path::new(&f), Limits::default(), None, None)
        .err()
        .map(|e| e.to_string())
        .expect("refused");
    assert!(why.contains("state directory"), "{why}");

    // destinations: exact hosts only
    for (dest, word) in [
        ("http://feeds.example", "https://host"),
        ("https://feeds.example/rss", "no path"),
        ("https://*.example", "no path"),
        ("https://localhost:8080", "own machine"),
        ("https://127.0.0.1", "own machine"),
        ("https://10.0.0.5", "own machine"),
        ("https://feeds.example:notaport", "not a port"),
    ] {
        let m = d.put(
            "fetch.manifest",
            format!(
                "roles = [\"reader\"]\nimports = [\"rhtn/1:request\", \"rhtn/1:response\"]\ncomponent = \"echo.wasm\"\nfetch = [\"{dest}\"]\n"
            ),
        );
        let why = run(&d, &host(&m)).expect_err(dest);
        assert!(why.contains(word), "{dest}: {why}");
    }
    let ok = d.put(
        "fetch-ok.manifest",
        "roles = [\"reader\"]\nimports = [\"rhtn/1:request\", \"rhtn/1:response\"]\ncomponent = \"echo.wasm\"\nfetch = [\"https://feeds.example\", \"https://api.example:8443\"]\n",
    );
    assert_eq!(run(&d, &host(&ok)), Ok(1));

    // an instance key is 32 bytes of hex, or nothing
    let keyed = format!(
        "[[host]]\nresource = \"{RES}\"\nowner = \"{WHO}\"\nauthority = \"shop.internal\"\nmanifest = \"{}\"\nkey = \"deadbeef\"\n",
        tolerant.display()
    );
    let why = run(&d, &keyed).expect_err("refused");
    assert!(why.contains("instance key"), "{why}");
}
