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

## A resource's window

A launcher card on a subnet's page frames a resource's page in a window of its own,
under the policy `light-client-requirements.md` §8 sets and the phone
already keeps (`src/frame.rs` says how each rule is met here):

- an origin per resource (`rhtnr://<keyhash in two labels>.rhtn.invalid`)
  and a web context per resource under `frames/<keyhash>/` in the
  desktop's directory, so a resource's storage is its own and is kept;
- every load inside that origin carried by the kernel through the node
  that served the catalog entry, bodies included; a navigation anywhere
  else not followed, and anything fetched from anywhere else refused by
  the policy put on every answer;
- script only where the page's own policy names `script-src` and the
  person allowed it for that resource, asked once in the main window and
  remembered; `rhtn.request` is published into a page that runs, with the
  phone's answers, so a page written for the phone runs here unchanged;
- in a field-test build, the web inspector, for whoever is writing the
  page.

**What the engine then does with it is checked in the engine**, since no
unit test can see it:

```sh
cargo run -p rhtn-desktop --features gui --example frame_check
```

opens two canned resources on the display — one allowed to run code and
trying everything a page might, one whose code was refused — and prints a
line per rule and exits non-zero where one did not hold. It opens windows,
so it is run by hand and not by the gate.

**A typed ask (§8.1) is declined** with *this device has no such
utility*, which is an answer the resource sees, and the person is told
what was asked: this desktop offers none of the device's utilities yet.

**An answer the desktop does not show is offered for saving**, as the
phone's *Save to a file* does: where the page moves to one (a navigation,
told by an `accept` leading with `text/html`), the frame names its
declared type and size, and the main window asks whether to save it; a
yes opens the toolkit's save dialog with a name suggested from the
resource and the declared type. A script's own request gets the bytes,
which are its to read.

## The node an identity runs

**A restored desktop administers it; a paired one leaves it to the phone**
[author, 2026-11-26]. A subnet's options say *Administer the node* where
this key runs one (it published the endpoint record on the line it holds).
Pressed, the desktop derives the host control key from the seed it holds,
opens an ssh tunnel to the machine its endpoint record names, and reaches
the instance's surface on that machine's own loopback, where an act needs
no act token (`operate.rs`). A paired desktop holds no seed and derives no
key, and says so.

The window it opens draws **every act the instance declares**
(`GET /node/acts`), the node's own and each resource's, as the phone's
`ActActivity` draws them: the same checks said before anything is sent,
the same body, and the machine's restricted key or this desktop's signed
runner derived for the one act that brings it. After an act it reads the
declaration again, since an install or a removal changes what may be asked.

- **An offering pasted fills the install** — a resource minted for it, this
  identity its owner, the distributor's image, authority, port and name,
  and the manifest in its field to be read — and installs nothing until
  the person sends it.
- **The instance's own page**, from the button at the top, is framed with
  script refused by policy, a navigation off its one origin not followed,
  and each control on it bouncing to the operator window as the act it
  names, its values filled in as hints the person confirms.

`cargo run -p rhtn-desktop --example operate_check -- <machine> <control key
file> [<runner key file>]` reaches a live instance the way the window does,
reads and draws its declaration, fetches its page, and with the runner key
sends `running`: the bench's check, not the gate's, which reaches no
machine.

## What is not built yet

The launch itself: choosing a region and a size, minting the enrolment
token, composing the once-run script and enrolling the instance. The
phone does all of it (`OperatorActivity`'s launch section) and so does
`rhtn node launch`; this window stops at the credential.
