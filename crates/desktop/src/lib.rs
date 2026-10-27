//! **The operator's desktop** (`light-client-requirements.md` §10): the
//! provider's own console in a window, the steps beside it, the token
//! pasted back, and what it is worth said at once.
//!
//! **Why a desktop client at all.** §8.3 has the provisioning pages ship
//! with the client, and an operator standing up their first node has no
//! node to serve them one: the pages must work before any node exists.
//! The phone does this already; a desktop is where an operator with a
//! keyboard would rather do it, and it is the only client that can put a
//! provider's sign-up next to the instructions for it.
//!
//! **Everything here is what the window is not.** The window and its web
//! view are the system's and are `main.rs`'s business, behind the `gui`
//! feature; this module is the page it shows, the acts that page can ask
//! for, and what each act does — all of it testable with no desktop
//! libraries present, which is what lets the gate check it.
//!
//! **No credential of the project's, and none scraped.** The provider's
//! console is a page of theirs in a window of ours: another origin, which
//! this client cannot read and does not try to (a token read out of
//! someone else's page would be a credential taken rather than given).
//! The operator makes a token there as the provider's own steps say, and
//! pastes it. What this client then does is try it once and keep it where
//! the command line's `--credential-file` expects it.

// Every public item carries its own documentation.
#![warn(missing_docs)]

use rhtn_launch::Transport;
use std::path::{Path, PathBuf};

/// What the page can ask the shell to do.  A closed set, because the page
/// is this crate's own and a message that is not one of these is a
/// message nobody wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Open this provider's console in a window of its own: the sign-up,
    /// the payment method, the two-factor prompt and the token page, at
    /// whatever pace they take.
    Console {
        /// The provider, as a credential is filed under.
        provider: String,
    },
    /// Try a pasted token at its provider and keep it where it works.
    Token {
        /// The provider.
        provider: String,
        /// What was pasted.
        token: String,
    },
}

/// Read one message from the page, or say why it is not one.
pub fn act(message: &str) -> Result<Act, String> {
    let v: serde_json::Value =
        serde_json::from_str(message).map_err(|e| format!("not a message: {e}"))?;
    let s = |k: &str| -> Result<String, String> {
        v.get(k)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(String::from)
            .ok_or_else(|| format!("`{k}` is missing"))
    };
    let provider = s("provider")?;
    if rhtn_launch::provider(&provider).is_none() {
        return Err(format!("{provider} is not a provider this build drives"));
    }
    match s("act")?.as_str() {
        "console" => Ok(Act::Console { provider }),
        "token" => Ok(Act::Token {
            provider,
            token: s("token")?,
        }),
        other => Err(format!("`{other}` is not an act")),
    }
}

/// Where a provider's token is kept: one file a provider, under the
/// directory [`directory`] names, and the same shape the command line
/// reads with `--credential-file`.
pub fn credential_file(dir: &Path, provider: &str) -> PathBuf {
    dir.join(format!("{}.token", provider.to_ascii_lowercase()))
}

/// **Where this client keeps what it is given**: the directory named on
/// the command line, else `$XDG_CONFIG_HOME/rhtn`, else `~/.config/rhtn`.
/// Nothing is written until an operator pastes something.
pub fn directory(named: Option<&str>) -> Result<PathBuf, String> {
    if let Some(d) = named {
        return Ok(PathBuf::from(d));
    }
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME").filter(|x| !x.is_empty()) {
        return Ok(PathBuf::from(x).join("rhtn"));
    }
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .ok_or("neither XDG_CONFIG_HOME nor HOME is set: name a directory on the command line")?;
    Ok(PathBuf::from(home).join(".config/rhtn"))
}

/// **Try a token at its provider, and keep it only if it answers.**
///
/// The one read-only call every adapter has is the regions, so that is
/// the try: a token that cannot list them cannot launch either, and the
/// operator hears the provider's own words now rather than at the launch.
/// What is kept is kept readable by its owner alone — a provider's token
/// is a key, and `keys mint` writes an identity the same way.
pub fn verify_and_keep(
    t: &dyn Transport,
    dir: &Path,
    provider: &str,
    token: &str,
) -> Result<String, String> {
    let p = rhtn_launch::provider(provider)
        .ok_or_else(|| format!("{provider} is not a provider this build drives"))?;
    let regions = p.regions(t, token)?;
    let sizes = p.sizes(t, token)?;
    let at = credential_file(dir, provider);
    if let Some(d) = at.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    write_private(&at, token)?;
    Ok(format!(
        "the token answers at {}: {} regions and {} sizes offered. Kept in {} — the launch reads \
         it from there, and `rhtn node launch --credential-file {}` does too.",
        p.name(),
        regions.len(),
        sizes.len(),
        at.display(),
        at.display()
    ))
}

/// Write a token readable by its owner alone.
fn write_private(at: &Path, text: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(at)
            .map_err(|e| format!("{}: {e}", at.display()))?;
        f.write_all(text.as_bytes())
            .map_err(|e| format!("{}: {e}", at.display()))?;
        std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("{}: {e}", at.display()))
    }
    #[cfg(not(unix))]
    {
        std::fs::write(at, text).map_err(|e| format!("{}: {e}", at.display()))
    }
}

/// The console a provider's steps start at.
pub fn console_of(provider: &str) -> Option<&'static str> {
    rhtn_launch::provider(provider).map(|p| p.token_page())
}

/// **The page this client serves itself**: every provider this build
/// drives, its steps in its own words, and one field for what they
/// produce.  Composed here rather than written as a file beside the
/// binary, so that a provider added to the launch crate appears here
/// without a second place to remember.
pub fn page() -> String {
    let mut cards = String::new();
    for name in rhtn_launch::provider_names() {
        let Some(p) = rhtn_launch::provider(name) else {
            continue;
        };
        let steps: String = p
            .token_walkthrough()
            .iter()
            .map(|s| format!("<li>{}</li>", escape(s)))
            .collect();
        cards.push_str(&format!(
            "<section data-provider=\"{name}\">\n\
             <h2>{} <span class=\"asn\">AS{}</span></h2>\n\
             <p class=\"sub\">The token is yours, made at {}&#39;s own console and pasted back. \
             Nothing of this project&#39;s is registered there, and this window cannot read \
             their page — you paste, it is tried once, and it is kept readable by you alone.</p>\n\
             <ol>{steps}</ol>\n\
             <button data-act=\"console\">Open {}&#39;s console</button>\n\
             <input type=\"password\" placeholder=\"paste the token\" autocomplete=\"off\" \
             spellcheck=\"false\">\n\
             <button data-act=\"token\">Try it and keep it</button>\n\
             </section>\n",
            escape(p.name()),
            p.asn(),
            escape(p.name()),
            escape(p.name()),
        ));
    }
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <title>Launch a node</title>\n<style>\n{STYLE}\n</style></head><body>\n\
         <h1>Launch a node at a hosting provider</h1>\n\
         <p class=\"sub\">One instance, at a provider you choose, with a credential of your own \
         and no broker between (<code>light-client-requirements.md</code> §10).</p>\n\
         {cards}<pre id=\"said\"></pre>\n<script>\n{SCRIPT}\n</script></body></html>\n"
    )
}

/// What the page says back, as the shell hands it: a line for the person,
/// and whether it went through.
pub fn said(ok: bool, text: &str) -> String {
    format!(
        "document.getElementById('said').textContent = {};\
         document.getElementById('said').className = {};",
        json_string(text),
        if ok { "'ok'" } else { "'bad'" }
    )
}

/// A string as JavaScript source reads it.
fn json_string(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = ":root { color-scheme: light dark }\n\
     body { font: 15px/1.55 system-ui, sans-serif; margin: 0 auto; padding: 1.5rem; max-width: 46rem }\n\
     h1 { font-size: 1.2rem; margin: 0 0 .25rem }\n\
     h2 { font-size: 1rem; margin: 0 0 .25rem }\n\
     .asn { font-weight: normal; opacity: .6; font-size: .85rem }\n\
     p.sub { margin: 0 0 1rem; opacity: .75 }\n\
     section { border: 1px solid rgba(127,127,127,.35); border-radius: 6px; padding: 1rem; margin: 1rem 0 }\n\
     ol { margin: 0 0 1rem; padding-left: 1.4rem }\n\
     li { margin: .3rem 0 }\n\
     input { width: 100%; box-sizing: border-box; padding: .5rem; margin: .5rem 0;\n\
       font: 13px ui-monospace, monospace }\n\
     button { padding: .45rem .8rem; margin-right: .4rem }\n\
     pre { padding: .6rem .8rem; background: rgba(127,127,127,.12); border-radius: 4px;\n\
       white-space: pre-wrap; font: 13px/1.5 ui-monospace, monospace; min-height: 1.5em }\n\
     pre.bad { background: rgba(200,80,80,.18) }";

const SCRIPT: &str = "for (const s of document.querySelectorAll('section')) {\n\
  const provider = s.dataset.provider;\n\
  const field = s.querySelector('input');\n\
  for (const b of s.querySelectorAll('button')) {\n\
    b.addEventListener('click', () => {\n\
      const act = b.dataset.act;\n\
      const message = { act, provider };\n\
      if (act === 'token') {\n\
        if (!field.value.trim()) { document.getElementById('said').textContent = 'paste the token first'; return }\n\
        message.token = field.value.trim();\n\
      }\n\
      document.getElementById('said').textContent = 'asking…';\n\
      window.ipc.postMessage(JSON.stringify(message));\n\
    });\n\
  }\n\
}";

#[cfg(test)]
mod tests {
    use super::*;

    /// A transport that answers from a script, as the launch crate's own
    /// tests use.
    struct Scripted(std::cell::RefCell<Vec<(u16, String)>>);

    impl Transport for Scripted {
        fn exchange(
            &self,
            _: &str,
            _: &str,
            _: &[(&str, &str)],
            _: Option<&str>,
        ) -> Result<(u16, String), String> {
            self.0
                .borrow_mut()
                .pop()
                .ok_or("no answer scripted".to_string())
        }
    }

    fn scripted(answers: Vec<(u16, &str)>) -> Scripted {
        Scripted(std::cell::RefCell::new(
            answers
                .into_iter()
                .map(|(s, b)| (s, b.to_string()))
                .rev()
                .collect(),
        ))
    }

    #[test]
    fn the_page_carries_every_provider_this_build_drives_and_its_own_steps() {
        let html = page();
        for name in rhtn_launch::provider_names() {
            let p = rhtn_launch::provider(name).unwrap();
            assert!(
                html.contains(&format!("data-provider=\"{name}\"")),
                "{name} is on the page"
            );
            assert!(html.contains(&format!("AS{}", p.asn())));
            // its steps, in its own words
            let first = p.token_walkthrough()[0];
            assert!(html.contains(&escape(first)), "{name}: {first}");
        }
        assert!(
            html.contains("type=\"password\""),
            "a token is not shown as it is typed"
        );
        assert!(
            html.contains("cannot read \n             their page") || html.contains("cannot read"),
            "the page says what it does not do"
        );
    }

    #[test]
    fn a_message_from_the_page_is_read_and_anything_else_refused() {
        assert_eq!(
            act(r#"{"act":"console","provider":"hetzner"}"#).unwrap(),
            Act::Console {
                provider: "hetzner".into()
            }
        );
        assert_eq!(
            act(r#"{"act":"token","provider":"digitalocean","token":" dop_v1_x "}"#).unwrap(),
            Act::Token {
                provider: "digitalocean".into(),
                token: "dop_v1_x".into()
            }
        );
        for (message, why) in [
            (
                r#"{"act":"token","provider":"aws","token":"x"}"#,
                "provider",
            ),
            (r#"{"act":"launch","provider":"hetzner"}"#, "act"),
            (r#"{"act":"token","provider":"hetzner"}"#, "token"),
            ("not json", "message"),
        ] {
            let e = act(message).unwrap_err();
            assert!(e.contains(why), "{message}: {e}");
        }
    }

    #[test]
    fn a_token_that_answers_is_kept_owner_only_and_one_that_does_not_is_not_kept() {
        let dir = std::env::temp_dir().join(format!("rhtn-desktop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let refusing = scripted(vec![(
            401,
            r#"{"error":{"code":"unauthorized","message":"the token you have provided is invalid"}}"#,
        )]);
        let e = verify_and_keep(&refusing, &dir, "hetzner", "nope").unwrap_err();
        assert_eq!(
            e,
            "Hetzner answered 401: the token you have provided is invalid"
        );
        assert!(
            !credential_file(&dir, "hetzner").exists(),
            "a token that does not answer is not kept"
        );

        let answering = scripted(vec![
            (
                200,
                r#"{"locations":[{"id":1,"name":"fsn1","description":"Falkenstein","country":"DE","city":"Falkenstein"}]}"#,
            ),
            (
                200,
                r#"{"server_types":[{"name":"cx22","cores":2,"memory":4.0,"architecture":"x86","prices":[{"location":"fsn1","price_monthly":{"net":"4.59"}}],"locations":[{"name":"fsn1","available":true}]}]}"#,
            ),
        ]);
        let said = verify_and_keep(&answering, &dir, "hetzner", "good-token").unwrap();
        assert!(said.contains("1 regions and 1 sizes"), "{said}");
        let at = credential_file(&dir, "hetzner");
        assert_eq!(std::fs::read_to_string(&at).unwrap(), "good-token");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&at).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "a provider's token is a key");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_directory_is_the_one_named_or_the_platforms_own() {
        assert_eq!(directory(Some("/tmp/x")).unwrap(), PathBuf::from("/tmp/x"));
        // the environment of the test process is not ours to set about,
        // so this asserts the shape rather than one answer
        let d = directory(None).unwrap();
        assert!(d.ends_with("rhtn"), "{}", d.display());
    }

    #[test]
    fn what_the_page_is_told_is_a_string_a_script_can_hold() {
        let s = said(false, "Hetzner answered 401: \"no\"\nand stopped");
        assert!(s.contains("'bad'"));
        assert!(s.contains("\\\"no\\\""), "a quote is escaped: {s}");
        assert!(s.contains("\\n"), "and a newline: {s}");
        assert!(!s.contains("\nand stopped"), "nothing breaks the script");
    }
}
