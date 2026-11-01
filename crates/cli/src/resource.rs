//! **Ask a resource for something, as a client of its host**
//! (`wire-format.md` §11, §11.1).
//!
//! **Why this is not in [`crate::probe`]**: that command sends the three
//! read-only request types and says so — *a resource request has an
//! application effect, whatever the HTTP method inside; the omission is
//! the feature*. This is the command that has the effect, named for it,
//! so neither is mistaken for the other.
//!
//! What it is for: the operator's own check that what they installed on
//! their node answers, before any phone is involved — and the one way to
//! read a resource's answer without a light client's screen. **It is a
//! client of the host and nothing more**: it attaches as an identity the
//! caller holds the key for, and the node evaluates that identity's reach
//! exactly as it evaluates a phone's (§11's membership gate, the subtree
//! acknowledgement, the role row). A stranger gets `refused` here as a
//! stranger gets it anywhere.
//!
//! **It reads the one HTTP message the answer carries with the client's
//! own reader** (`rhtn_client::resource`), which is the same parse a
//! light client does below its own boundary — not a second one written
//! here (§11.2).

use rhtn_archive::Keyhash;
use rhtn_archive::catalog::{REQUEST_RESOURCE, ResourceRequest, ResourceResponse};
use rhtn_transport::session::Session;

/// What §11.1's six statuses say, in words.
pub fn delivery(status: u64) -> &'static str {
    match status {
        0 => "delivered",
        1 => "refused: not a member of the owner's horizon, or no such resource here",
        2 => "the resource is unavailable: nothing is running behind it",
        3 => "the request was malformed",
        4 => "no subtree acknowledgement covers this identity",
        5 => "no role of this identity's matches",
        _ => "a status this version does not name",
    }
}

/// One request to `resource` through the node this `session` is with: a
/// `GET` of `path`, which is the simplest application request there is.
///
/// The answer is described rather than returned raw: the status in words,
/// then what the message carried — its own status, its declared type, its
/// size, and the body where it is text or a typed ask, which is what the
/// person running this came to read.
pub async fn ask(
    session: &Session,
    resource: Keyhash,
    path: &str,
    body: Option<(&str, Vec<u8>)>,
) -> Result<String, String> {
    let message = match &body {
        None => format!("GET {path} HTTP/1.1\r\nhost: resource\r\n\r\n").into_bytes(),
        Some((media, bytes)) => {
            let mut out = format!(
                "POST {path} HTTP/1.1\r\nhost: resource\r\ncontent-type: {media}\r\ncontent-length: {}\r\n\r\n",
                bytes.len()
            )
            .into_bytes();
            out.extend_from_slice(bytes);
            out
        }
    };
    let request = ResourceRequest { resource, message };
    let bytes = session.request(REQUEST_RESOURCE, &request.encode()).await?;
    let reply =
        ResourceResponse::decode(&bytes).map_err(|e| format!("the reply does not decode: {e}"))?;
    let mut out = format!("status    {} ({})\n", reply.status, delivery(reply.status));
    let Some(carried) = reply.body else {
        return Ok(out);
    };
    out.push_str(&format!("message   {} bytes\n", carried.len()));
    match rhtn_client::resource::answer(&carried) {
        Err(why) => {
            out.push_str(&format!("unreadable {}\n", why.say()));
            Ok(out)
        }
        Ok(a) => {
            out.push_str(&format!("http      {}\n", a.http_status));
            out.push_str(&format!(
                "type      {}\n",
                a.media.clone().unwrap_or_else(|| "none declared".into())
            ));
            out.push_str(&format!("body      {} bytes\n", a.body.len()));
            // **what is printed is what the type says it is**: text and a
            // typed ask are text, and anything else is named rather than
            // spilled into a terminal that would read it as commands
            let text = matches!(
                a.shape,
                rhtn_client::resource::Shape::Text
                    | rhtn_client::resource::Shape::Ask
                    | rhtn_client::resource::Shape::Page
            );
            if text {
                let shown = String::from_utf8_lossy(&a.body);
                out.push('\n');
                out.push_str(&shown[..shown.len().min(8192)]);
                if !shown.ends_with('\n') {
                    out.push('\n');
                }
            }
            Ok(out)
        }
    }
}
