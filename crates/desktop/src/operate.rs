//! **The instances this identity runs, administered from the desktop**
//! (`infra-client-requirements.md` §8.3, `light-client-requirements.md`
//! §10) [author, 2026-11-26]: a restored desktop alone, which holds the
//! seed the machine's keys derive from; a paired one leaves it to the
//! phone.
//!
//! **The way in is the machine's own loopback**, as `bench-package.py`
//! reaches it: an ssh tunnel opened with the host control key this
//! identity derives, to the surface on the instance's `127.0.0.1`, where
//! an act needs no act token because arriving there is the authorisation.
//! So the desktop holds no token of the phone's and needs none.
//!
//! **What may be asked is the instance's declaration** (`rhtn_daemon::acts`),
//! drawn here as the phone's `Acts.kt` draws it: the same reading, the
//! same checks said before the round trip, the same body. **What is shown**
//! is the instance's own page, framed with script refused, its controls
//! bouncing to the act they name with their values as hints a person
//! confirms. Nothing in a declaration or a page is trusted beyond its
//! shape: the node is a party a seized machine could speak for.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;
use zeroize::Zeroizing;

/// The vocabulary this client reads (`rhtn_daemon::acts::SURFACE`).
pub const SURFACE: &str = "rhtn/administration@1";

/// **The scheme the node's page bounces an act through**
/// (`rhtn_daemon::operator::ACT_SCHEME`).
pub const ACT_SCHEME: &str = "rhtn-act";

/// Where a launched instance's surface listens on its own machine.
pub const SURFACE_PORT: u16 = 7449;

/// The scheme the instance's own page is framed under here.
pub const PAGE_SCHEME: &str = "rhtnn";

/// That frame's one origin.
pub const PAGE_ORIGIN: &str = "rhtnn://node";

/// What a parameter holds: the closed set, as the declaration names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// On or off.
    Flag,
    /// A whole number between two bounds.
    Number {
        /// The least.
        low: i64,
        /// The most.
        high: i64,
    },
    /// Text of at most `max` characters.
    Text {
        /// The most.
        max: usize,
    },
    /// One of a list.
    Choice {
        /// What it may be.
        of: Vec<String>,
    },
    /// Any of a list.
    Several {
        /// What may be chosen.
        of: Vec<String>,
    },
    /// 64 lower-case hex digits.
    Keyhash,
    /// **A kind this client does not draw**, named so it can say so.
    Unknown(String),
}

/// One parameter of an act.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Its name, which is the field the instance reads.
    pub name: String,
    /// What the person reads.
    pub label: String,
    /// What it holds.
    pub kind: Kind,
}

/// What the client brings that neither the person nor the page can.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Brings {
    /// Nothing.
    Nothing,
    /// The restricted ssh key for the machine, as `key`.
    HostKey,
    /// That key, and this client's runner signed by the operator's key.
    SignedRunner,
    /// Something this client cannot bring, which is a refusal.
    Unknown,
}

/// One act, as declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act {
    /// Its name.
    pub name: String,
    /// Its label.
    pub label: String,
    /// Its help line.
    pub help: String,
    /// What it brings.
    pub brings: Brings,
    /// Its parameters, in order.
    pub parameters: Vec<Parameter>,
}

/// One hosted resource's acts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hosted {
    /// The resource, in hex.
    pub resource: String,
    /// The authority it answers under.
    pub authority: String,
    /// Whether this instance installed it itself.
    pub installed: bool,
    /// Its acts.
    pub acts: Vec<Act>,
}

/// A whole declaration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Declared {
    /// The vocabulary it is written in.
    pub surface: String,
    /// The acts about the instance.
    pub node: Vec<Act>,
    /// Each resource's.
    pub resources: Vec<Hosted>,
}

impl Declared {
    /// Whether this client reads the vocabulary it is written in.
    pub fn understood(&self) -> bool {
        self.surface == SURFACE
    }

    /// One act by name, and the resource it is about where it is one's:
    /// a resource's act is looked for under that resource.
    pub fn act(&self, name: &str, resource: Option<&str>) -> Option<&Act> {
        match resource {
            Some(r) => self
                .resources
                .iter()
                .find(|h| h.resource == r)
                .and_then(|h| h.acts.iter().find(|a| a.name == name)),
            None => self.node.iter().find(|a| a.name == name).or_else(|| {
                self.resources
                    .iter()
                    .find_map(|h| h.acts.iter().find(|a| a.name == name))
            }),
        }
    }
}

/// **Read a declaration** out of what `GET /node/acts` answered.
pub fn read(text: &str) -> Result<Declared, String> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("not a declaration: {e}"))?;
    let s = |v: &serde_json::Value, k: &str| {
        v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
    };
    let list = |v: &serde_json::Value, k: &str| {
        v.get(k)
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default()
    };
    let act = |a: &serde_json::Value| Act {
        name: s(a, "name"),
        label: s(a, "label"),
        help: s(a, "help"),
        brings: match s(a, "brings").as_str() {
            "nothing" => Brings::Nothing,
            "host-key" => Brings::HostKey,
            "signed-runner" => Brings::SignedRunner,
            _ => Brings::Unknown,
        },
        parameters: list(a, "parameters")
            .iter()
            .map(|p| {
                let of = || {
                    list(p, "of")
                        .iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                };
                let n = |k: &str| p.get(k).and_then(serde_json::Value::as_i64).unwrap_or(0);
                Parameter {
                    name: s(p, "name"),
                    label: s(p, "label"),
                    kind: match s(p, "kind").as_str() {
                        "flag" => Kind::Flag,
                        "number" => Kind::Number {
                            low: n("low"),
                            high: n("high"),
                        },
                        "text" => Kind::Text {
                            max: usize::try_from(n("max")).unwrap_or(0),
                        },
                        "choice" => Kind::Choice { of: of() },
                        "several" => Kind::Several { of: of() },
                        "keyhash" => Kind::Keyhash,
                        other => Kind::Unknown(other.to_string()),
                    },
                }
            })
            .collect(),
    };
    Ok(Declared {
        surface: s(&v, "surface"),
        node: list(&v, "node").iter().map(act).collect(),
        resources: list(&v, "resources")
            .iter()
            .map(|r| Hosted {
                resource: s(r, "resource"),
                authority: s(r, "authority"),
                installed: r
                    .get("installed")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                acts: list(r, "acts").iter().map(act).collect(),
            })
            .collect(),
    })
}

/// **Whether this client can draw and send an act**, and why not: a field
/// whose meaning it does not know is not one to fill, and a credential it
/// cannot bring is not one to leave out.
pub fn undrawable(a: &Act) -> Option<String> {
    if a.brings == Brings::Unknown {
        return Some("it needs something this desktop does not know how to bring".into());
    }
    a.parameters.iter().find_map(|p| match &p.kind {
        Kind::Unknown(k) => Some(format!(
            "`{}` is a `{k}`, which this desktop does not draw",
            p.name
        )),
        _ => None,
    })
}

/// **Whether an answer is one the instance will take**, said before the
/// round trip, the bounds being the declaration's.
pub fn refused(p: &Parameter, given: &str) -> Option<String> {
    match &p.kind {
        Kind::Flag => (given != "on" && given != "off").then(|| "it is on or off".into()),
        Kind::Number { low, high } => match given.parse::<i64>() {
            Err(_) => Some("it is a number".into()),
            Ok(n) if n < *low || n > *high => Some(format!("it is between {low} and {high}")),
            Ok(_) => None,
        },
        Kind::Text { max } => {
            (given.chars().count() > *max).then(|| format!("at most {max} characters"))
        }
        Kind::Choice { of } => {
            (!of.iter().any(|o| o == given)).then(|| format!("it is one of {}", of.join(", ")))
        }
        Kind::Several { of } => given
            .split(',')
            .filter(|x| !x.is_empty())
            .find(|x| !of.iter().any(|o| o == x))
            .map(|x| format!("`{x}` is not one of {}", of.join(", "))),
        Kind::Keyhash => (given.len() != 64
            || !given
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then(|| "it is 64 lower-case hex digits".into()),
        Kind::Unknown(k) => Some(format!("this desktop does not draw a `{k}`")),
    }
}

/// **The body of an invocation**: the declared fields in declared order
/// and nothing else, an empty one left out, a `several` one field per
/// value — `Acts.body` on the phone.
pub fn body(a: &Act, answers: &[(String, String)]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for p in &a.parameters {
        let Some((_, given)) = answers.iter().find(|(k, _)| *k == p.name) else {
            continue;
        };
        if given.is_empty() {
            continue;
        }
        if let Some(why) = refused(p, given) {
            return Err(format!("`{}`: {why}", p.name));
        }
        match p.kind {
            Kind::Several { .. } => {
                for one in given.split(',').filter(|x| !x.is_empty()) {
                    out.push((p.name.clone(), one.to_string()));
                }
            }
            _ => out.push((p.name.clone(), given.clone())),
        }
    }
    Ok(out)
}

/// **The act a `rhtn-act:` link names, and its query**: a name the shape
/// a declared one has, so a page cannot put a path or a scheme there.
pub fn bounced(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix(ACT_SCHEME)?.strip_prefix(':')?;
    let (name, query) = rest.split_once('?').unwrap_or((rest, ""));
    let namelike = name
        .bytes()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_');
    (!name.is_empty() && name.len() <= 32 && namelike)
        .then(|| (name.to_string(), query.to_string()))
}

/// **What a bounce suggested**, as fields of the act: the names it
/// declares and no other, so a page cannot smuggle a field past the form.
/// Every value is a hint the person confirms.
pub fn hints(a: &Act, query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .filter_map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            let (k, v) = (unescape(k), unescape(v));
            a.parameters.iter().any(|d| d.name == k).then_some((k, v))
        })
        .collect()
}

/// Percent-decoding, and `+` for a space; a `%` not followed by two hex
/// digits is itself.
pub fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                match (
                    b.get(i + 1).copied().and_then(hex),
                    b.get(i + 2).copied().and_then(hex),
                ) {
                    (Some(h), Some(l)) => {
                        out.push((h * 16 + l) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `application/x-www-form-urlencoded`, as the surface reads it.
pub fn form(fields: &[(String, String)]) -> Zeroizing<Vec<u8>> {
    let escape = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    Zeroizing::new(
        fields
            .iter()
            .map(|(k, v)| format!("{k}={}", escape(v)))
            .collect::<Vec<_>>()
            .join("&")
            .into_bytes(),
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// One act drawn as a form, its fields by kind.
fn act_form(a: &Act, resource: Option<&str>) -> String {
    let mut out = format!(
        "<form class=\"act\" data-act=\"{}\"{}>\n<h3>{}</h3>\n<p class=\"sub\">{}</p>\n",
        escape_html(&a.name),
        resource.map_or(String::new(), |r| format!(
            " data-resource=\"{}\"",
            escape_html(r)
        )),
        escape_html(&a.label),
        escape_html(&a.help),
    );
    if let Some(why) = undrawable(a) {
        out.push_str(&format!(
            "<p class=\"bad\">Not drawn here: {}.</p>\n</form>\n",
            escape_html(&why)
        ));
        return out;
    }
    for p in &a.parameters {
        let name = escape_html(&p.name);
        let label = escape_html(&p.label);
        // the resource a resource's act is about is the one shown
        let given = match (p.name.as_str(), resource) {
            ("resource", Some(r)) => escape_html(r),
            _ => String::new(),
        };
        let field = match &p.kind {
            Kind::Flag => format!(
                "<label class=\"flag\"><input type=\"checkbox\" name=\"{name}\"> {label}</label>"
            ),
            Kind::Number { low, high } => format!(
                "<label>{label}<input type=\"number\" name=\"{name}\" min=\"{low}\" max=\"{high}\" value=\"{given}\"></label>"
            ),
            Kind::Text { max } if *max > 256 => format!(
                "<label>{label}<textarea name=\"{name}\" maxlength=\"{max}\" rows=\"6\" spellcheck=\"false\"></textarea></label>"
            ),
            Kind::Text { max } => format!(
                "<label>{label}<input name=\"{name}\" maxlength=\"{max}\" value=\"{given}\" autocomplete=\"off\" spellcheck=\"false\"></label>"
            ),
            Kind::Choice { of } => {
                let options: String = of
                    .iter()
                    .map(|o| format!("<option>{}</option>", escape_html(o)))
                    .collect();
                format!("<label>{label}<select name=\"{name}\">{options}</select></label>")
            }
            Kind::Several { of } => {
                let boxes: String = of
                    .iter()
                    .map(|o| {
                        format!(
                            "<label class=\"flag\"><input type=\"checkbox\" name=\"{name}\" value=\"{0}\"> {0}</label>",
                            escape_html(o)
                        )
                    })
                    .collect();
                format!("<fieldset><legend>{label}</legend>{boxes}</fieldset>")
            }
            Kind::Keyhash => format!(
                "<label>{label}<input name=\"{name}\" maxlength=\"64\" value=\"{given}\" autocomplete=\"off\" spellcheck=\"false\" class=\"hex\"></label>"
            ),
            Kind::Unknown(_) => String::new(),
        };
        out.push_str(&field);
        out.push('\n');
    }
    let brings = match a.brings {
        Brings::HostKey => " It brings this desktop's key for the machine, derived for this act.",
        Brings::SignedRunner => " It brings that key and this desktop's runner, signed as you.",
        _ => "",
    };
    out.push_str(&format!(
        "<p><button type=\"submit\">{}</button><span class=\"sub\">{brings}</span></p>\n<pre class=\"said\"></pre>\n</form>\n",
        escape_html(&a.label)
    ));
    out
}

/// **The operator window**: every act the instance declares, the node's
/// own first and then each resource's under its authority, an offering
/// to install from, and the door to the instance's own page.
pub fn page(label: &str, d: &Declared) -> String {
    let mut body = format!(
        "<h1>{}</h1>\n<p class=\"sub\">What this instance declares it can be asked, drawn by this \
         desktop and sent over its own machine's loopback. <button id=\"own\" type=\"button\">Its \
         own page</button></p>\n",
        escape_html(label)
    );
    if !d.understood() {
        body.push_str(&format!(
            "<p class=\"bad\">It declares its acts in `{}`, and this desktop reads `{SURFACE}`: a \
             newer desktop will draw them.</p>\n",
            escape_html(&d.surface)
        ));
    } else {
        body.push_str(
            "<section><h2>Install from an offering</h2>\n<p class=\"sub\">A link a package's \
             distributor published. It fills the install below, with what the package declares \
             shown, and installs nothing until you send it.</p>\n\
             <textarea id=\"offering\" rows=\"3\" spellcheck=\"false\" \
             placeholder=\"fueros://install?v=1&amp;…\"></textarea>\n\
             <p><button id=\"offered\" type=\"button\">Fill the install</button></p>\n\
             <pre class=\"said\" id=\"offering-said\"></pre></section>\n",
        );
        body.push_str("<section><h2>This instance</h2>\n");
        for a in &d.node {
            body.push_str(&act_form(a, None));
        }
        body.push_str("</section>\n");
        for h in &d.resources {
            body.push_str(&format!(
                "<section><h2>{}</h2>\n<p class=\"sub hex\">{}{}</p>\n",
                escape_html(&h.authority),
                escape_html(&h.resource),
                if h.installed {
                    ""
                } else {
                    " — written into the hosting file by hand"
                }
            ));
            for a in &h.acts {
                body.push_str(&act_form(a, Some(&h.resource)));
            }
            body.push_str("</section>\n");
        }
    }
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n<title>{}</title>\n\
         <style>\n{STYLE}\n</style></head><body>\n{body}<script>\n{SCRIPT}\n</script></body></html>\n",
        escape_html(label)
    )
}

/// What the window says back to one form, or to the offering's.
pub fn said_script(act: &str, resource: Option<&str>, ok: bool, text: &str) -> String {
    let j = |s: &str| serde_json::Value::String(s.to_string()).to_string();
    format!(
        "said({}, {}, {}, {});",
        j(act),
        resource.map_or("null".into(), j),
        ok,
        j(text)
    )
}

/// **A form filled with hints**, shown and scrolled to, sent by nobody.
pub fn fill_script(act: &str, resource: Option<&str>, values: &[(String, String)]) -> String {
    let j = |s: &str| serde_json::Value::String(s.to_string()).to_string();
    let pairs: Vec<String> = values
        .iter()
        .map(|(k, v)| format!("[{}, {}]", j(k), j(v)))
        .collect();
    format!(
        "fill({}, {}, [{}]);",
        j(act),
        resource.map_or("null".into(), j),
        pairs.join(", ")
    )
}

const STYLE: &str = ":root { color-scheme: light dark }\n\
     body { font: 15px/1.55 system-ui, sans-serif; margin: 0 auto; padding: 1.5rem; max-width: 52rem }\n\
     h1 { font-size: 1.25rem; margin: 0 0 .25rem } h2 { font-size: 1.05rem; margin: 0 0 .25rem }\n\
     h3 { font-size: .95rem; margin: .5rem 0 .1rem }\n\
     .sub { opacity: .75; margin: 0 0 .5rem } .hex { font: 12px ui-monospace, monospace }\n\
     section { border: 1px solid rgba(127,127,127,.35); border-radius: 6px; padding: 1rem; margin: 1rem 0 }\n\
     form.act { border-top: 1px solid rgba(127,127,127,.2); padding-top: .5rem; margin-top: .5rem }\n\
     form.act.lit { outline: 2px solid rgba(80,140,220,.6); outline-offset: 4px }\n\
     label { display: block; margin: .3rem 0 } label.flag { display: inline-block; margin-right: 1rem }\n\
     input:not([type=checkbox]), select, textarea { display: block; width: 100%; box-sizing: border-box;\n\
       padding: .4rem; margin: .2rem 0; font: 13px ui-monospace, monospace }\n\
     button { padding: .4rem .8rem; margin-right: .4rem }\n\
     pre.said { white-space: pre-wrap; font: 13px/1.5 ui-monospace, monospace; margin: .3rem 0 }\n\
     pre.said:empty { display: none } pre.said.ok { background: rgba(80,160,80,.15) }\n\
     pre.said.bad, p.bad { background: rgba(200,80,80,.18); padding: .3rem .5rem }";

const SCRIPT: &str = r#"const post = m => window.ipc.postMessage(JSON.stringify(m));
const formOf = (act, resource) => [...document.querySelectorAll('form.act')].find(f =>
  f.dataset.act === act && (resource === null ? !f.dataset.resource : f.dataset.resource === resource));
function said(act, resource, ok, text) {
  const out = act === 'offering' ? document.getElementById('offering-said')
    : (formOf(act, resource) || document.body).querySelector('pre.said');
  if (!out) return;
  out.textContent = text;
  out.className = 'said ' + (ok ? 'ok' : 'bad');
  const f = formOf(act, resource);
  if (f) f.querySelector('button[type=submit]').disabled = false;
}
function fill(act, resource, values) {
  const f = formOf(act, resource) || formOf(act, null);
  if (!f) return;
  for (const [k, v] of values) {
    const all = f.querySelectorAll('[name="' + CSS.escape(k) + '"]');
    for (const el of all) {
      if (el.type === 'checkbox') el.checked = el.value === 'on' ? v === 'on' : v.split(',').includes(el.value);
      else el.value = v;
    }
  }
  document.querySelectorAll('form.lit').forEach(x => x.classList.remove('lit'));
  f.classList.add('lit');
  f.scrollIntoView({ block: 'center' });
}
document.addEventListener('submit', e => {
  e.preventDefault();
  const f = e.target;
  const fields = [];
  const several = {};
  for (const el of f.querySelectorAll('[name]')) {
    if (el.type === 'checkbox' && el.value !== 'on') {
      if (el.checked) (several[el.name] = several[el.name] || []).push(el.value);
      else several[el.name] = several[el.name] || [];
    } else if (el.type === 'checkbox') fields.push([el.name, el.checked ? 'on' : 'off']);
    else fields.push([el.name, el.value]);
  }
  for (const [k, v] of Object.entries(several)) fields.push([k, v.join(',')]);
  f.querySelector('button[type=submit]').disabled = true;
  const out = f.querySelector('pre.said');
  out.textContent = 'asking…'; out.className = 'said';
  post({ act: f.dataset.act, resource: f.dataset.resource || null, fields });
});
document.getElementById('own').addEventListener('click', () => post({ act: 'own-page' }));
const offered = document.getElementById('offered');
if (offered) offered.addEventListener('click', () => {
  const text = document.getElementById('offering').value.trim();
  if (!text) return said('offering', null, false, 'paste the offering first');
  post({ act: 'offering', text });
});"#;

/// **What the operator window sent**: an act with its fields, the
/// offering pasted, or the door to the instance's own page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// Send this act.
    Act {
        /// Its name.
        name: String,
        /// The resource it is about, where it is a resource's.
        resource: Option<String>,
        /// The fields, as typed.
        fields: Vec<(String, String)>,
    },
    /// Fill the install from this offering.
    Offering(String),
    /// Open the instance's own page.
    OwnPage,
}

/// Read one message the operator window sent.
pub fn asked(message: &str) -> Result<Asked, String> {
    let v: serde_json::Value =
        serde_json::from_str(message).map_err(|e| format!("not a message: {e}"))?;
    let act = v
        .get("act")
        .and_then(|x| x.as_str())
        .ok_or("`act` is missing")?;
    match act {
        "own-page" => Ok(Asked::OwnPage),
        "offering" => Ok(Asked::Offering(
            v.get("text")
                .and_then(|x| x.as_str())
                .ok_or("`text` is missing")?
                .to_string(),
        )),
        name => {
            let fields = v
                .get("fields")
                .and_then(|x| x.as_array())
                .ok_or("`fields` is missing")?
                .iter()
                .filter_map(|p| {
                    let p = p.as_array()?;
                    Some((
                        p.first()?.as_str()?.to_string(),
                        p.get(1)?.as_str()?.to_string(),
                    ))
                })
                .collect();
            Ok(Asked::Act {
                name: name.to_string(),
                resource: v.get("resource").and_then(|x| x.as_str()).map(String::from),
                fields,
            })
        }
    }
}

/// **The machine a node of this identity's answers on**: the first
/// address among its points, its port dropped, since the tunnel goes to
/// the machine's ssh and the surface is on its loopback.
pub fn machine(addresses: &[String]) -> Option<String> {
    addresses.iter().find_map(|a| {
        let s: SocketAddr = a.parse().ok()?;
        Some(s.ip().to_string())
    })
}

/// **The tunnel to one instance's surface**: an ssh `-L` from a free
/// port of this machine to the instance's own loopback, opened with the
/// host control key and closed when this is dropped. The key is written
/// for ssh to read in a directory of this desktop's alone and removed as
/// soon as the session is up.
pub struct Tunnel {
    child: std::process::Child,
    /// The port of this machine the surface is reached at.
    pub port: u16,
}

impl Tunnel {
    /// Open it to `machine`, with `key` the private half `ssh -i` reads,
    /// in a directory under `private`.
    pub fn open(machine: &str, key: &Zeroizing<String>, private: &Path) -> Result<Tunnel, String> {
        crate::platform::create_private(private)?;
        let at: PathBuf = private.join(format!("control-{}", std::process::id()));
        {
            let mut o = std::fs::OpenOptions::new();
            o.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                o.mode(0o600);
            }
            let mut f = o.open(&at).map_err(|e| format!("{}: {e}", at.display()))?;
            f.write_all(key.as_bytes()).map_err(|e| e.to_string())?;
        }
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map_err(|e| e.to_string())?
            .port();
        let spawned = std::process::Command::new("ssh")
            .args([
                "-i",
                &at.display().to_string(),
                "-o",
                "IdentitiesOnly=yes",
                "-o",
                "ConnectTimeout=15",
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-o",
                "BatchMode=yes",
                "-N",
                "-L",
                &format!("{port}:127.0.0.1:{SURFACE_PORT}"),
                &format!("root@{machine}"),
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_file(&at);
                return Err(format!("ssh did not start: {e}"));
            }
        };
        for _ in 0..80 {
            if let Ok(Some(status)) = child.try_wait() {
                let _ = std::fs::remove_file(&at);
                let mut why = String::new();
                if let Some(mut e) = child.stderr.take() {
                    let _ = e.read_to_string(&mut why);
                }
                return Err(format!(
                    "the tunnel to {machine} closed ({status}): {}",
                    why.trim()
                ));
            }
            if TcpStream::connect_timeout(
                &SocketAddr::from(([127, 0, 0, 1], port)),
                Duration::from_millis(250),
            )
            .is_ok()
            {
                let _ = std::fs::remove_file(&at);
                return Ok(Tunnel { child, port });
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(&at);
        Err(format!("the tunnel to {machine} did not open"))
    }

    /// **One exchange with the surface**: the status and the body.
    pub fn exchange(
        &self,
        method: &str,
        path: &str,
        media: Option<&str>,
        body: &[u8],
        budget: Duration,
    ) -> Result<(u16, Vec<u8>), String> {
        exchange(self.port, method, path, media, body, budget)
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        // **by its own process**, never by a pattern
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One HTTP/1.1 exchange with the surface reached at `port`.
pub fn exchange(
    port: u16,
    method: &str,
    path: &str,
    media: Option<&str>,
    body: &[u8],
    budget: Duration,
) -> Result<(u16, Vec<u8>), String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut s =
        TcpStream::connect_timeout(&addr, Duration::from_secs(10)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(budget)).ok();
    s.set_write_timeout(Some(Duration::from_secs(15))).ok();
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nhost: 127.0.0.1:{SURFACE_PORT}\r\ncontent-length: {}\r\nconnection: close\r\n",
        body.len()
    );
    if let Some(m) = media {
        head.push_str(&format!("content-type: {m}\r\n"));
    }
    head.push_str("\r\n");
    s.write_all(head.as_bytes())
        .and_then(|()| s.write_all(body))
        .and_then(|()| s.flush())
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    s.read_to_end(&mut out).map_err(|e| e.to_string())?;
    let split = out
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("the surface's answer has no head")?;
    let status = std::str::from_utf8(&out[..split])
        .ok()
        .and_then(|h| h.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .ok_or("the surface's answer has no status")?;
    Ok((status, out[split + 4..].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECLARED: &str = r#"{"surface":"rhtn/administration@1",
      "node":[{"name":"install","label":"Install a package","help":"h","about":"node","brings":"host-key",
        "parameters":[{"name":"resource","label":"r","kind":"keyhash"},
                      {"name":"port","label":"p","kind":"number","low":1,"high":65535},
                      {"name":"sealed","label":"s","kind":"flag"}]},
              {"name":"odd","label":"Odd","help":"","about":"node","brings":"nothing",
        "parameters":[{"name":"x","label":"x","kind":"colour"}]}],
      "resources":[{"resource":"ab","authority":"shop.internal","installed":true,
        "acts":[{"name":"grant","label":"Grant a role","help":"","about":"resource","brings":"nothing",
          "parameters":[{"name":"resource","label":"r","kind":"keyhash"},
                        {"name":"roles","label":"roles","kind":"several","of":["reader","writer"]},
                        {"name":"of","label":"who","kind":"choice","of":["everyone","named"]}]}]}]}"#;

    #[test]
    fn a_declaration_is_read_and_what_this_desktop_cannot_draw_is_said() {
        let d = read(DECLARED).unwrap();
        assert!(d.understood());
        assert_eq!(d.node.len(), 2);
        assert_eq!(d.act("install", None).unwrap().brings, Brings::HostKey);
        assert!(d.act("grant", Some("ab")).is_some());
        assert!(
            d.act("grant", Some("cd")).is_none(),
            "a resource's act is that resource's"
        );
        assert_eq!(undrawable(d.act("install", None).unwrap()), None);
        assert!(
            undrawable(d.act("odd", None).unwrap())
                .unwrap()
                .contains("colour")
        );
        let p = page("GXV's node", &d);
        assert!(p.contains("data-act=\"install\"") && p.contains("data-resource=\"ab\""));
        assert!(p.contains("Not drawn here"));
        assert!(
            !page(
                "x",
                &Declared {
                    surface: "rhtn/administration@9".into(),
                    ..d
                }
            )
            .contains("data-act=")
        );
    }

    #[test]
    fn a_body_is_the_declared_fields_checked_and_a_set_is_one_field_a_value() {
        let d = read(DECLARED).unwrap();
        let grant = d.act("grant", Some("ab")).unwrap();
        let f = |v: &[(&str, &str)]| {
            v.iter()
                .map(|(k, x)| (k.to_string(), x.to_string()))
                .collect::<Vec<_>>()
        };
        let kh = "ab".repeat(32);
        assert_eq!(
            body(
                grant,
                &f(&[
                    ("resource", &kh),
                    ("roles", "reader,writer"),
                    ("of", "everyone"),
                    ("stray", "x")
                ])
            )
            .unwrap(),
            f(&[
                ("resource", &kh),
                ("roles", "reader"),
                ("roles", "writer"),
                ("of", "everyone")
            ])
        );
        assert!(
            body(grant, &f(&[("roles", "admin")]))
                .unwrap_err()
                .contains("admin")
        );
        assert!(body(grant, &f(&[("of", "someone")])).is_err());
        let install = d.act("install", None).unwrap();
        assert!(
            body(install, &f(&[("port", "70000")]))
                .unwrap_err()
                .contains("between 1 and 65535")
        );
        assert!(body(install, &f(&[("resource", "AB")])).is_err());
        assert_eq!(
            body(install, &f(&[("sealed", "off"), ("port", "")])).unwrap(),
            f(&[("sealed", "off")])
        );
    }

    #[test]
    fn a_bounce_names_a_declared_act_and_carries_only_its_fields() {
        let d = read(DECLARED).unwrap();
        assert_eq!(
            bounced("rhtn-act:grant?resource=ab&of=named"),
            Some(("grant".into(), "resource=ab&of=named".into()))
        );
        assert_eq!(
            bounced("rhtn-act:grant"),
            Some(("grant".into(), String::new()))
        );
        assert_eq!(bounced("rhtn-act:../x"), None);
        assert_eq!(bounced("https://example.com"), None);
        let grant = d.act("grant", Some("ab")).unwrap();
        assert_eq!(
            hints(grant, "resource=ab&key=stolen&of=named%20x"),
            vec![
                ("resource".to_string(), "ab".to_string()),
                ("of".to_string(), "named x".to_string())
            ]
        );
        assert_eq!(unescape("a+b%2Cc%"), "a b,c%");
    }

    #[test]
    fn the_window_says_what_it_was_asked() {
        assert_eq!(asked(r#"{"act":"own-page"}"#).unwrap(), Asked::OwnPage);
        assert_eq!(
            asked(r#"{"act":"offering","text":"fueros://x"}"#).unwrap(),
            Asked::Offering("fueros://x".into())
        );
        assert_eq!(
            asked(r#"{"act":"grant","resource":"ab","fields":[["roles","reader"]]}"#).unwrap(),
            Asked::Act {
                name: "grant".into(),
                resource: Some("ab".into()),
                fields: vec![("roles".into(), "reader".into())]
            }
        );
        assert_eq!(
            machine(&["203.0.113.9:7447".into()]).as_deref(),
            Some("203.0.113.9")
        );
        assert_eq!(
            machine(&["[2001:db8::1]:7447".into()]).as_deref(),
            Some("2001:db8::1")
        );
        assert_eq!(machine(&["nonsense".into()]), None);
        assert!(said_script("grant", Some("ab"), true, "granted \"x\"").contains("\\\"x\\\""));
        assert!(
            fill_script("install", None, &[("name".into(), "Ledger".into())])
                .starts_with("fill(\"install\", null")
        );
    }
}
