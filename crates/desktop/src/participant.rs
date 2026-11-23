//! **The participant client's window**: the pages it shows, what it serves
//! them from, and what it keeps between runs beyond the kernel's own
//! state.  Everything decided here is testable with no desktop libraries
//! present; `main.rs` only carries it to the window.
//!
//! **The pages are served from the window's own scheme**, `rhtn://`, so
//! the shared look (`ui/tokens.css`, written from `crates/brand`) and the
//! Geist faces load as files beside the page rather than being pasted
//! into it, and nothing the window shows is fetched from anywhere else.

use std::path::Path;

/// The file beside the kernel's state that says how this desktop was
/// brought up and, where paired, which identity it is a device of and the
/// run it presents: all public, so a file the owner alone reads is
/// enough.  The seeds are the keyring's (`custody.rs`).
pub const HELD: &str = "device.json";

/// **What this desktop keeps to start again**, by the way it was brought
/// up (design §23.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// Paired from the phone: the identity's `KeyMaterial`
    /// (`wire-format.md` §2.2) and the run of delegations, in window order
    /// (§8.2), beside the transport seed the keyring holds.
    Paired {
        /// The identity's `KeyMaterial`.
        material: Vec<u8>,
        /// The run.
        run: Vec<Vec<u8>>,
    },
    /// Restored from the phrase: the identity's seeds are the keyring's,
    /// and nothing else is needed to start.
    Restored,
}

impl Held {
    /// Write it under `dir`, whole or not at all.
    pub fn keep(&self, dir: &Path) -> Result<(), String> {
        let v = match self {
            Held::Paired { material, run } => serde_json::json!({
                "paired": {
                    "material": hex(material),
                    "run": run.iter().map(|d| hex(d)).collect::<Vec<_>>(),
                }
            }),
            Held::Restored => serde_json::json!({ "restored": true }),
        };
        crate::platform::write_whole(&dir.join(HELD), v.to_string().as_bytes())
            .map_err(|e| format!("{HELD}: {e}"))
    }

    /// What is kept under `dir`: nothing before the first start, an error
    /// where something is kept and does not read.
    pub fn load(dir: &Path) -> Result<Option<Held>, String> {
        let b = match std::fs::read(dir.join(HELD)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{HELD}: {e}")),
        };
        let v: serde_json::Value =
            serde_json::from_slice(&b).map_err(|e| format!("{HELD}: {e}"))?;
        if v.get("restored").and_then(serde_json::Value::as_bool) == Some(true) {
            return Ok(Some(Held::Restored));
        }
        let p = v
            .get("paired")
            .ok_or_else(|| format!("{HELD}: neither paired nor restored"))?;
        let bytes = |s: Option<&serde_json::Value>| {
            s.and_then(serde_json::Value::as_str)
                .and_then(unhex)
                .ok_or_else(|| format!("{HELD}: a field is not hex"))
        };
        let run = p
            .get("run")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("{HELD}: no run"))?
            .iter()
            .map(|d| bytes(Some(d)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Held::Paired {
            material: bytes(p.get("material"))?,
            run,
        }))
    }
}

/// **How many days a run covers**: each window is 48 hours and the run
/// is contiguous (`wire-format.md` §8.2).
pub fn run_days(run: &[Vec<u8>]) -> u64 {
    run.len() as u64 * 2
}

/// Bytes in lower-case hex.
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Lower-case or upper-case hex back as bytes, or none where it is not hex.
pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// **What the window's scheme serves**: the page's own files, and nothing
/// a path could name outside them.  `None` for anything else.
pub fn asset(path: &str) -> Option<(&'static str, &'static [u8])> {
    Some(match path.trim_start_matches('/') {
        "tokens.css" => ("text/css", include_bytes!("../ui/tokens.css")),
        "fonts/geist.ttf" => ("font/ttf", include_bytes!("../../brand/res/font/geist.ttf")),
        "fonts/geist_italic.ttf" => (
            "font/ttf",
            include_bytes!("../../brand/res/font/geist_italic.ttf"),
        ),
        "fonts/geist_mono.ttf" => (
            "font/ttf",
            include_bytes!("../../brand/res/font/geist_mono.ttf"),
        ),
        "fonts/geist_mono_italic.ttf" => (
            "font/ttf",
            include_bytes!("../../brand/res/font/geist_mono_italic.ttf"),
        ),
        _ => return None,
    })
}

/// **The page a desktop starts on the first time**: the code to pair it
/// from the phone, and the phrase to restore it from where the phone
/// never shares a network with it (design §23.3).
pub fn pair_page(code_svg: &str) -> String {
    shell(
        "Set up this desktop",
        &format!(
            "<h1>Pair this desktop with your phone</h1>\n\
             <p class=\"sub\">On the phone, open the identity's settings and choose \
             <b>A desktop of this identity</b>, then point its camera at this code. \
             The desktop becomes a device of that identity: it can read everything \
             and sign nothing.</p>\n\
             <figure class=\"code\">{code_svg}</figure>\n\
             <p class=\"sub\">Both devices must be on the same local network. The code is \
             good until this window closes.</p>\n\
             <h2>Or restore it from your phrase</h2>\n\
             <p class=\"sub\">Where the phone and this desktop never share a network. The \
             desktop then holds the identity as the phone does and can sign as it; it knows \
             only the nodes you introduce it to, and receives what they serve from now on.</p>\n\
             <textarea id=\"phrase\" rows=\"4\" autocomplete=\"off\" autocapitalize=\"off\" \
             spellcheck=\"false\" placeholder=\"the twenty-four words\"></textarea>\n\
             <p><button id=\"restore\">Restore</button></p>"
        ),
    )
}

/// **What this desktop is to its identity**, as its home page says it
/// (design §23.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Device {
    /// Paired from the phone, presenting a run of `days`.
    Paired {
        /// How long the run lasts.
        days: u64,
        /// Whether the dataset came across, said once on the start that
        /// paired it and not again.
        dataset: Option<bool>,
    },
    /// Restored from the phrase: a signing device, as the phone is.
    Restored,
}

/// **Where this desktop is connected, in one line**: the node it holds a
/// session with first, else the nodes it agreed to and is not reaching,
/// else none.  `attached` is the kernel's sessions, serving node first;
/// `agreed` its agreements with their addresses.
pub fn connection(attached: &[Vec<u8>], agreed: &[(Vec<u8>, Vec<String>)]) -> String {
    let short = |k: &[u8]| -> String { hex(k).chars().take(16).collect() };
    let at = |k: &[u8]| -> String {
        agreed
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, a)| format!(" at {}", a.join(" ")))
            .unwrap_or_default()
    };
    if let Some(node) = attached.first() {
        return format!("Connected to {}…{}", short(node), at(node));
    }
    match agreed.first() {
        Some((node, _)) => format!(
            "Not connected. {}…{} is tried again at every start.",
            short(node),
            at(node)
        ),
        None => "Not connected to any node.".to_string(),
    }
}

/// **The page that says why the window cannot go on**: no keyring, a
/// state that will not open.  Nothing is offered that would fall back to
/// keeping a secret in a file.
pub fn stopped_page(why: &str) -> String {
    shell(
        "FuerosNet",
        &format!(
            "<h1>This desktop cannot start</h1>\n<pre class=\"bad\">{}</pre>",
            escape(why)
        ),
    )
}

/// A page in the shared look, with the line the shell writes into and the
/// question it asks.
pub(crate) fn shell(title: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <title>{}</title>\n<link rel=\"stylesheet\" href=\"tokens.css\">\n\
         <style>\n{STYLE}\n</style></head><body>\n{body}\n\
         <div id=\"ask\" hidden><p></p><button data-yes=\"1\">Yes</button>\
         <button data-yes=\"0\">No</button></div>\n\
         <pre id=\"said\"></pre>\n<script>\n{SCRIPT}\n</script></body></html>\n",
        escape(title)
    )
}

/// **The kernel's question, put to the person**: the script that shows it
/// with a yes and a no, whose answer comes back as an `answer` act.
pub fn ask_script(question: &str) -> String {
    format!(
        "(() => {{ const a = document.getElementById('ask'); \
         a.querySelector('p').textContent = {}; a.hidden = false; }})();",
        serde_json::Value::String(question.to_string())
    )
}

pub(crate) fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = "body { background: var(--ground); color: var(--ink);\n\
       font: 15px/1.55 var(--sans); margin: 0 auto; padding: 1.5rem; max-width: 46rem }\n\
     h1 { font-size: 1.25rem; font-weight: 600; margin: 0 0 .5rem }\n\
     p.sub { color: var(--sub); margin: 0 0 1rem }\n\
     .mono, pre { font-family: var(--mono) }\n\
     .mono { overflow-wrap: anywhere }\n\
     figure.code { margin: 1.5rem 0; display: flex; justify-content: center }\n\
     figure.code svg { max-width: 100%; height: auto }\n\
     h2 { font-size: 1rem; font-weight: 600; margin: 2rem 0 .5rem }\n\
     label { display: block; color: var(--sub); font-size: 13px; margin: .5rem 0 }\n\
     input { display: block; width: 100%; box-sizing: border-box; margin-top: .25rem;\n\
       background: var(--sunken); color: var(--ink); border: 1px solid var(--rule);\n\
       border-radius: 4px; padding: .5rem .6rem; font: 14px/1.5 var(--mono) }\n\
     textarea { width: 100%; box-sizing: border-box; background: var(--sunken); color: var(--ink);\n\
       border: 1px solid var(--rule); border-radius: 4px; padding: .6rem; font: 14px/1.5 var(--mono) }\n\
     #ask { background: var(--chrome); border: 1px solid var(--rule); border-radius: 6px;\n\
       padding: 1rem; margin: 1rem 0 }\n\
     button { background: var(--accent); color: var(--on-accent); border: 0;\n\
       border-radius: 4px; padding: .45rem .9rem; margin-right: .5rem; font: inherit }\n\
     button[data-yes=\"0\"] { background: var(--control); color: var(--ink) }\n\
     pre { background: var(--sunken); border-radius: 4px; padding: .6rem .8rem;\n\
       white-space: pre-wrap; font-size: 13px; min-height: 1.5em }\n\
     pre.bad, .bad { color: var(--warn) }\n\
     body:has(#rail) { max-width: none; margin: 0; padding: 1.5rem 1.5rem 1.5rem 16rem }\n\
     body:has(#rail) > :not(#rail) { max-width: 46rem }\n\
     #rail { position: fixed; top: 0; bottom: 0; left: 0; width: 14rem; box-sizing: border-box;\n\
       overflow-y: auto; padding: 1rem .75rem; background: var(--chrome);\n\
       border-right: 1px solid var(--rule); display: flex; flex-direction: column }\n\
     #rail h3 { font-size: 12px; font-weight: 600; color: var(--sub); margin: 1rem .5rem .25rem;\n\
       text-transform: uppercase; letter-spacing: .04em }\n\
     #rail small { color: var(--sub); padding: 0 .5rem }\n\
     #rail .foot { margin-top: auto; padding-top: 1rem }\n\
     a[data-go] { display: block; padding: .35rem .5rem; border-radius: 4px; color: var(--ink);\n\
       cursor: pointer; text-decoration: none; overflow: hidden; text-overflow: ellipsis }\n\
     a[data-go]:hover { background: var(--control) }\n\
     a[data-go].here { background: var(--sunken) }\n\
     ul.rows { list-style: none; padding: 0; margin: 0 }\n\
     .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));\n\
       gap: .75rem; margin: 1.5rem 0 }\n\
     button.card { position: relative; background: var(--sunken); color: var(--ink);\n\
       border: 1px solid var(--rule); border-radius: 8px; padding: 1.25rem .9rem;\n\
       margin: 0; text-align: left; font-weight: 600; cursor: pointer;\n\
       overflow-wrap: anywhere }\n\
     button.card:hover { background: var(--control) }\n\
     sup { display: inline-block; min-width: 1.1em; margin-left: .35em; padding: 0 .3em;\n\
       border-radius: .6em; background: var(--accent); color: var(--on-accent);\n\
       font-size: 11px; line-height: 1.5; text-align: center; vertical-align: super }\n\
     button.card sup { position: absolute; top: -.5em; right: -.5em; margin: 0 }\n\
     ul.rows small { color: var(--sub) }\n\
     .said { margin: .5rem 0; padding: .5rem .75rem; border-radius: 6px; background: var(--sunken);\n\
       max-width: 80%; width: fit-content }\n\
     .said p { margin: 0; white-space: pre-wrap }\n\
     .said.mine { margin-left: auto; background: var(--control) }\n\
     .said small { color: var(--sub) }\n\
     #compose { position: sticky; bottom: 0; background: var(--ground); padding-top: .5rem }";

const SCRIPT: &str = r#"const post = m => window.ipc.postMessage(JSON.stringify(m));
const say = t => { document.getElementById('said').textContent = t; };
// one listener for the page, since a place's rail and body are redrawn
// under it and anything bound to them would go with them
document.addEventListener('click', e => {
  const b = e.target.closest('button, a[data-go]');
  if (!b) return;
  if (b.dataset.go) return post({ act: 'go', place: b.dataset.go });
  if (b.dataset.yes) {
    document.getElementById('ask').hidden = true;
    return post({ act: 'answer', yes: b.dataset.yes === '1' });
  }
  if (b.dataset.act) return post({ act: b.dataset.act });
  if (b.dataset.open) {
    post({ act: 'open', resource: b.dataset.open, node: b.dataset.node, name: b.dataset.name });
    return say('opening ' + b.dataset.name + '…');
  }
  if (b.id === 'restore') {
    const phrase = document.getElementById('phrase');
    if (!phrase.value.trim()) return say('type the phrase first');
    post({ act: 'restore', phrase: phrase.value });
    phrase.value = '';
    return say('restoring…');
  }
  if (b.id === 'connect') {
    const node = document.getElementById('node').value.trim();
    const address = document.getElementById('address').value.trim();
    if (!node || !address) return say('the key and the address, both');
    post({ act: 'attach', node, address });
    return say('connecting to ' + address + '…');
  }
  if (b.id === 'send') send();
  if (b.id === 'import') {
    const f = document.getElementById('backup').files[0];
    if (!f) return say('choose the backup file first');
    say('opening ' + f.name + '…');
    f.arrayBuffer().then(buf => {
      const hex = Array.from(new Uint8Array(buf), x => x.toString(16).padStart(2, '0')).join('');
      post({ act: 'backup', data: hex });
    }, err => say('the file did not read: ' + err));
  }
});
function send() {
  const t = document.getElementById('text');
  if (!t.value.trim()) return;
  post({ act: 'send', to: t.dataset.peer, text: t.value });
  t.value = '';
}
// Enter sends, as a chat's box does; Shift and Enter is a new line
document.addEventListener('keydown', e => {
  if (e.target.id === 'text' && e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(); }
});
if (document.getElementById('messages')) window.scrollTo(0, document.body.scrollHeight);
if (document.getElementById('text')) document.getElementById('text').focus();"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_a_desktop_keeps_reads_back_and_nothing_kept_is_none() {
        let d = std::env::temp_dir().join(format!("rhtn-held-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        crate::platform::create_private(&d).unwrap();
        assert_eq!(Held::load(&d).unwrap(), None);
        let h = Held::Paired {
            material: vec![1, 2, 0xff],
            run: vec![vec![9; 4], vec![0; 3]],
        };
        h.keep(&d).unwrap();
        assert_eq!(Held::load(&d).unwrap(), Some(h));
        Held::Restored.keep(&d).unwrap();
        assert_eq!(Held::load(&d).unwrap(), Some(Held::Restored));
        std::fs::write(
            d.join(HELD),
            b"{\"paired\":{\"material\":\"zz\",\"run\":[]}}",
        )
        .unwrap();
        assert!(Held::load(&d).is_err(), "a kept file that does not read");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_scheme_serves_the_pages_files_and_nothing_else() {
        assert_eq!(asset("/tokens.css").unwrap().0, "text/css");
        for f in [
            "fonts/geist.ttf",
            "fonts/geist_italic.ttf",
            "fonts/geist_mono.ttf",
            "fonts/geist_mono_italic.ttf",
        ] {
            assert!(asset(f).unwrap().1.len() > 1000, "{f}");
        }
        for f in ["../Cargo.toml", "/etc/passwd", "fonts/../tokens.css", ""] {
            assert!(asset(f).is_none(), "{f}");
        }
        // every face the stylesheet names is one the scheme serves
        let css = std::str::from_utf8(asset("tokens.css").unwrap().1).unwrap();
        for url in css.split("url(\"").skip(1) {
            let path = url.split('"').next().unwrap();
            assert!(asset(path).is_some(), "{path}");
        }
    }

    #[test]
    fn the_pages_wear_the_shared_look_and_escape_what_they_are_given() {
        let p = pair_page("<svg/>");
        assert!(p.contains("href=\"tokens.css\"") && p.contains("<svg/>"));
        assert!(stopped_page("<b>no keyring</b>").contains("&lt;b&gt;no keyring"));
        assert!(p.contains("id=\"phrase\"") && p.contains("autocomplete=\"off\""));
        assert!(ask_script("Take \"it\"?").contains("\"Take \\\"it\\\"?\""));
    }

    #[test]
    fn the_connection_names_the_session_first_then_the_agreement() {
        let (a, b) = (vec![0x11; 32], vec![0x22; 32]);
        let agreed = vec![(a.clone(), vec!["203.0.113.7:7431".to_string()])];
        assert_eq!(
            connection(std::slice::from_ref(&a), &agreed),
            "Connected to 1111111111111111… at 203.0.113.7:7431"
        );
        assert_eq!(connection(&[b], &agreed), "Connected to 2222222222222222…");
        assert!(connection(&[], &agreed).starts_with("Not connected. 1111"));
        assert_eq!(connection(&[], &[]), "Not connected to any node.");
    }
}
