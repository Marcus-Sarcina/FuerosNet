//! **The authorisation flow that hands a client the provider's token**
//! (`light-client-requirements.md` §10): the client opens the provider's
//! own page, the person signs up or in and approves — at whatever pace,
//! through payment and a second factor and a confirmation mail — and the
//! provider sends the person back to a local callback with a code, which
//! the client exchanges for the token.  **The listener outlives the
//! attempt**: a fresh request can be issued as often as it takes, and the
//! redirect lands whichever window it comes from [author, 2026-10-25].
//!
//! The authorisation-code grant with PKCE (RFC 7636), as a public client:
//! no secret ships in a desktop or a phone, the proof that the party
//! exchanging the code is the party that asked is the verifier it kept.
//! A confidential client's secret is taken where the operator registered
//! one and chose to carry it.
//!
//! **A token that expires is held with what renews it.**  DigitalOcean's
//! access tokens last an hour for a public client and a month for a
//! confidential one, and a refresh token comes beside each; the
//! [`Credential`] carries both and the time the access ends, and
//! [`refresh`] renews it over the same transport.  Which providers offer
//! the flow at all: DigitalOcean and GCP; the others hand a token over
//! their console, pasted.

use crate::Transport;
use serde_json::Value;

/// A provider's OAuth2 endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// Where the person is sent to approve.
    pub authorize: String,
    /// Where a code or a refresh token is exchanged.
    pub token: String,
    /// The scopes a launch needs, as the provider spells them.
    pub scope: String,
}

/// DigitalOcean's endpoints.
pub fn digitalocean() -> Endpoints {
    Endpoints {
        authorize: "https://cloud.digitalocean.com/v1/oauth/authorize".into(),
        token: "https://cloud.digitalocean.com/v1/oauth/token".into(),
        scope: "read write".into(),
    }
}

/// **The application this client is, at DigitalOcean**: registered once by
/// the project under its own account (the registration is the project's,
/// not an operator's), and public — the id identifies the software and is
/// no secret, the proof at exchange is the PKCE verifier.  A confidential
/// client's secret, where an operator chooses to carry one, is given at
/// the command line and never kept here [author, 2026-10-25].
pub const DIGITALOCEAN_CLIENT_ID: &str =
    "c0dfa21d7030deb7857f1d931088c1a066e2e6af9cf5a1c8844c72c4ac40a979";

/// The client id this build carries for a provider, where the project
/// registered one.
pub fn client_id(provider: &str) -> Option<&'static str> {
    match provider.to_ascii_lowercase().as_str() {
        "digitalocean" => Some(DIGITALOCEAN_CLIENT_ID),
        _ => None,
    }
}

/// The endpoints of a provider that offers the flow, by name.
pub fn endpoints(provider: &str) -> Option<Endpoints> {
    match provider.to_ascii_lowercase().as_str() {
        "digitalocean" => Some(digitalocean()),
        _ => None,
    }
}

/// This client as the provider registered it: its id, its secret where
/// it is a confidential client, and the callback it named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// The client id the provider issued.
    pub client_id: String,
    /// The secret, for a confidential client; a public client has none
    /// and proves itself with PKCE.
    pub client_secret: Option<String>,
    /// The callback the provider sends the person back to.
    pub redirect_uri: String,
}

/// What the client keeps from an authorisation: the token, what renews
/// it, and when it ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    /// The bearer token a launch speaks with.
    pub access_token: String,
    /// What renews it, where the provider gave one.
    pub refresh_token: Option<String>,
    /// Seconds since the epoch at which `access_token` stops working, where
    /// the provider said; a pasted token has none.
    pub expires_at: Option<u64>,
    /// The scopes granted, as the provider spelled them.
    pub scope: Option<String>,
    /// Whose account, as the provider described it: a name or an email,
    /// for the operator to recognise.
    pub account: Option<String>,
    /// The client id the token was issued to, which a refresh needs
    /// again; none for a pasted token.
    pub client_id: Option<String>,
}

impl Credential {
    /// A pasted token: no expiry, nothing to renew it with.
    pub fn pasted(token: &str) -> Credential {
        Credential {
            access_token: token.trim().to_string(),
            refresh_token: None,
            expires_at: None,
            scope: None,
            account: None,
            client_id: None,
        }
    }

    /// Whether the access token should be renewed before use at `now`:
    /// within a minute of its end, or past it.
    pub fn stale_at(&self, now: u64) -> bool {
        matches!(self.expires_at, Some(end) if now + 60 >= end)
    }

    /// The document the kernel keeps for this provider.
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "access_token": self.access_token,
            "refresh_token": self.refresh_token,
            "expires_at": self.expires_at,
            "scope": self.scope,
            "account": self.account,
            "client_id": self.client_id,
        })
        .to_string()
    }

    /// Read back; a bare token, not a document, is a pasted one.
    pub fn from_json(text: &str) -> Credential {
        let t = text.trim();
        let Ok(v) = serde_json::from_str::<Value>(t) else {
            return Credential::pasted(t);
        };
        match v.get("access_token").and_then(Value::as_str) {
            Some(a) => Credential {
                access_token: a.to_string(),
                refresh_token: v
                    .get("refresh_token")
                    .and_then(Value::as_str)
                    .map(String::from),
                expires_at: v.get("expires_at").and_then(Value::as_u64),
                scope: v.get("scope").and_then(Value::as_str).map(String::from),
                account: v.get("account").and_then(Value::as_str).map(String::from),
                client_id: v.get("client_id").and_then(Value::as_str).map(String::from),
            },
            None => Credential::pasted(t),
        }
    }
}

/// A PKCE pair: the verifier the client keeps, the challenge it sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    /// 43 characters of base64url over 32 random bytes.
    pub verifier: String,
    /// `base64url(sha256(verifier))`, the S256 method.
    pub challenge: String,
}

impl Pkce {
    /// From 32 random bytes the caller drew.
    pub fn from_random(random: &[u8; 32]) -> Pkce {
        let verifier = base64url(random);
        let challenge = base64url(&rhtn_codec::cose::sha256(verifier.as_bytes()));
        Pkce {
            verifier,
            challenge,
        }
    }
}

/// The page to open: the provider's authorisation endpoint with this
/// client's id, the callback, the scopes, the state and the challenge.
pub fn authorize_url(e: &Endpoints, app: &App, state: &str, pkce: &Pkce) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        e.authorize,
        form(&app.client_id),
        form(&app.redirect_uri),
        form(&e.scope),
        form(state),
        form(&pkce.challenge)
    )
}

/// The code and the state out of the callback's request line, as the
/// listener reads it: `GET /callback?code=...&state=... HTTP/1.1`.
pub fn callback_params(request_line: &str) -> Option<(String, String)> {
    let path = request_line.split_whitespace().nth(1)?;
    let query = path.split_once('?')?.1;
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "code" => code = Some(unform(v)),
            "state" => state = Some(unform(v)),
            _ => {}
        }
    }
    Some((code?, state?))
}

/// Exchange the code for the token, proving with the verifier (and the
/// secret, where this client is a confidential one).
pub fn exchange(
    t: &dyn Transport,
    e: &Endpoints,
    app: &App,
    code: &str,
    pkce: &Pkce,
    now: u64,
) -> Result<Credential, String> {
    let mut body = format!(
        "grant_type=authorization_code&code={}&client_id={}&code_verifier={}&redirect_uri={}",
        form(code),
        form(&app.client_id),
        form(&pkce.verifier),
        form(&app.redirect_uri)
    );
    if let Some(s) = &app.client_secret {
        body.push_str(&format!("&client_secret={}", form(s)));
    }
    let mut c = token_request(t, e, &body, now)?;
    c.client_id = Some(app.client_id.clone());
    Ok(c)
}

/// Renew an access token with its refresh token.
pub fn refresh(
    t: &dyn Transport,
    e: &Endpoints,
    app: &App,
    refresh_token: &str,
    now: u64,
) -> Result<Credential, String> {
    let mut body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        form(refresh_token),
        form(&app.client_id)
    );
    if let Some(s) = &app.client_secret {
        body.push_str(&format!("&client_secret={}", form(s)));
    }
    let mut c = token_request(t, e, &body, now)?;
    c.client_id = Some(app.client_id.clone());
    Ok(c)
}

fn token_request(
    t: &dyn Transport,
    e: &Endpoints,
    body: &str,
    now: u64,
) -> Result<Credential, String> {
    let (status, text) = t.exchange(
        "POST",
        &e.token,
        &[
            ("Content-Type", "application/x-www-form-urlencoded"),
            ("Accept", "application/json"),
        ],
        Some(body),
    )?;
    let v: Value = serde_json::from_str(&text)
        .map_err(|_| format!("the token answer did not read ({status})"))?;
    if !(200..300).contains(&status) {
        let why = v
            .get("error_description")
            .or_else(|| v.get("error"))
            .and_then(Value::as_str)
            .unwrap_or("");
        return Err(format!("the provider answered {status}: {why}"));
    }
    let access = v
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or("the token answer carried no access token")?;
    let info = v.get("info");
    let account = info.and_then(|i| {
        i.get("email")
            .or_else(|| i.get("name"))
            .and_then(Value::as_str)
            .map(String::from)
    });
    Ok(Credential {
        access_token: access.to_string(),
        refresh_token: v
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(String::from),
        expires_at: v.get("expires_in").and_then(Value::as_u64).map(|s| now + s),
        scope: v.get("scope").and_then(Value::as_str).map(String::from),
        account,
        client_id: None,
    })
}

/// The one page the listener answers with, so the person's window says
/// what happened and can be closed.
pub fn callback_page(ok: bool) -> String {
    let body = if ok {
        "<h1>Authorised</h1><p>This window can be closed. The client has the token.</p>"
    } else {
        "<h1>Not authorised</h1><p>The state did not match the request this client made. Close this window and ask again from the client.</p>"
    };
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// RFC 4648 base64url, no padding.
pub fn base64url(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(A[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(A[n as usize & 63] as char);
        }
    }
    out
}

/// Percent-encoding for a form field or a query value.
fn form(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn unform(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct One(RefCell<Option<(u16, String)>>, RefCell<String>);
    impl Transport for One {
        fn exchange(
            &self,
            _: &str,
            _: &str,
            _: &[(&str, &str)],
            body: Option<&str>,
        ) -> Result<(u16, String), String> {
            *self.1.borrow_mut() = body.unwrap_or("").to_string();
            self.0.borrow_mut().take().ok_or("spent".into())
        }
    }

    // acceptance: PRT-08
    #[test]
    fn pkce_matches_the_rfc_example_and_the_url_carries_the_challenge() {
        // RFC 7636 appendix B: the verifier's bytes and the challenge
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = base64url(&rhtn_codec::cose::sha256(verifier.as_bytes()));
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        let p = Pkce {
            verifier: verifier.into(),
            challenge,
        };
        let app = App {
            client_id: "cid".into(),
            client_secret: None,
            redirect_uri: "http://127.0.0.1:7450/callback".into(),
        };
        let url = authorize_url(&digitalocean(), &app, "st4te", &p);
        assert!(url.starts_with("https://cloud.digitalocean.com/v1/oauth/authorize?client_id=cid&redirect_uri=http%3A%2F%2F127.0.0.1%3A7450%2Fcallback&response_type=code&scope=read%20write&state=st4te&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256"), "{url}");
        assert_eq!(
            callback_params("GET /callback?code=abc%2Fd&state=st4te HTTP/1.1"),
            Some(("abc/d".into(), "st4te".into()))
        );
        assert_eq!(callback_params("GET /favicon.ico HTTP/1.1"), None);
    }

    // acceptance: PRT-08
    #[test]
    fn the_exchange_posts_the_code_with_the_verifier_and_keeps_what_renews_the_token() {
        let t = One(
            RefCell::new(Some((
                200,
                r#"{"access_token":"doo_v1_x","token_type":"bearer","expires_in":3600,"refresh_token":"dor_v1_y","scope":"read write","info":{"name":"Dana","email":"dana@example.org"}}"#.into(),
            ))),
            RefCell::new(String::new()),
        );
        let p = Pkce::from_random(&[7u8; 32]);
        let app = App {
            client_id: "cid".into(),
            client_secret: None,
            redirect_uri: "http://127.0.0.1:7450/callback".into(),
        };
        let c = exchange(&t, &digitalocean(), &app, "the-code", &p, 1_000).unwrap();
        let sent = t.1.borrow().clone();
        assert!(sent.starts_with(
            "grant_type=authorization_code&code=the-code&client_id=cid&code_verifier="
        ));
        assert!(sent.contains(&format!("code_verifier={}", p.verifier)));
        assert!(!sent.contains("client_secret"));
        assert_eq!(c.access_token, "doo_v1_x");
        assert_eq!(c.refresh_token.as_deref(), Some("dor_v1_y"));
        assert_eq!(c.expires_at, Some(4_600));
        assert_eq!(c.account.as_deref(), Some("dana@example.org"));
        assert_eq!(c.client_id.as_deref(), Some("cid"));
        assert!(!c.stale_at(4_000));
        assert!(c.stale_at(4_560));
        let back = Credential::from_json(&c.to_json());
        assert_eq!(back, c);
        assert_eq!(
            Credential::from_json("  dop_v1_pasted \n").access_token,
            "dop_v1_pasted"
        );
    }

    // acceptance: PRT-08
    #[test]
    fn a_refusal_carries_the_providers_words() {
        let t = One(
            RefCell::new(Some((
                401,
                r#"{"error":"invalid_grant","error_description":"The code has expired"}"#.into(),
            ))),
            RefCell::new(String::new()),
        );
        let app = App {
            client_id: "cid".into(),
            client_secret: None,
            redirect_uri: "http://127.0.0.1:7450/callback".into(),
        };
        let e = refresh(&t, &digitalocean(), &app, "dor", 0).unwrap_err();
        assert!(
            e.contains("401") && e.contains("The code has expired"),
            "{e}"
        );
    }
}
