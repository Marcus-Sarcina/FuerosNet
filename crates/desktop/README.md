# `rhtn-desktop`

The operator's desktop for standing up a node: the provider's own console
in a window beside the steps for it, the token pasted back, and the launch
driven from there (`light-client-requirements.md` §10; the same
`rhtn-launch` composition the phone and `rhtn node launch` use).

## Building it

Everything below the window has no desktop dependency at all and builds
with the workspace:

```sh
cargo build -p rhtn-desktop          # the library: the page, the acts, the credential
cargo test  -p rhtn-desktop          # and its tests, which the gate runs
```

**The window is behind the `gui` feature, off by default**, so that the
gate and a continuous-integration container with no desktop libraries
build and test the rest. The window itself needs the *system's* web view
and its development files — this platform means WebKitGTK and GTK 3,
which on Debian and Ubuntu are:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev
```

and then:

```sh
cargo run -p rhtn-desktop --features gui            # tokens under $XDG_CONFIG_HOME/rhtn
cargo run -p rhtn-desktop --features gui -- /a/dir  # or where you say
```

Those two packages pull the rest of the closure (JavaScriptCore, libsoup,
cairo, pango, atk, gdk-pixbuf and their headers). Nothing is vendored and
no browser ships in this binary: the web view is the one the operating
system already updates.

**The libraries are the system's and stay the system's.** WebKitGTK and
GTK are LGPL and are linked dynamically, which is the arrangement that
licence is written for; the Rust bindings (`wry`, `tao` and their trees)
are Apache-2.0 or MIT, and `cargo deny` checks that with the rest of the
workspace.

## What it does, and what it will not do

- **The provider's console is their page in a window of ours.** Another
  origin, which this client cannot read and does not try to: a token read
  out of someone else's page would be a credential taken rather than
  given. The operator makes the token as the provider's own steps say and
  pastes it here.
- **A pasted token is tried once before it is kept** — the regions call
  every adapter has — so a bad token is refused in the provider's own
  words now rather than at the launch.
- **What is kept is kept owner-only**, one file per provider under the
  directory named on the command line, in the shape
  `rhtn node launch --credential-file` reads.
- **No credential of this project's is involved.** Nothing is registered
  with any provider on the project's behalf (an authorisation flow was
  built and withdrawn on 2026-10-25: a secret shipped in a client is
  public).

## What is not built yet

The launch itself: choosing a region and a size, minting the enrolment
token, composing the once-run script and enrolling the instance. The
phone does all of it (`OperatorActivity`'s launch section) and so does
`rhtn node launch`; this window stops at the credential.
