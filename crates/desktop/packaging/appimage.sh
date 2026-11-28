#!/bin/bash
# **The desktop as an AppImage**: the releasable build with its window,
# named `fuerosnet` [author, 2026-11-27], in one file that runs where it is
# put. The web view and GTK stay the system's, as the Debian package has
# them (the README says why), so it runs on a system with WebKitGTK 4.1
# and GTK 3 installed and bundles neither.
#
#    crates/desktop/packaging/appimage.sh [<out dir>]   # target/appimage by default
#
# **The tool and its runtime, pinned and checked before use**: both from
# the AppImage project's own releases, each digest the one GitHub publishes
# for the asset, taken on the day they were pinned [2026-11-27]. The
# runtime is handed over rather than fetched, so a build reaches nothing.
# A bump changes a file and its digest together.
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
CRATES=$(cd "$HERE/../.." && pwd)
OUT=${1:-$CRATES/target/appimage}
NAME=fuerosnet
TOOLS=${APPIMAGETOOL_DIR:-$HOME/opt/appimagetool}
TOOL_SHA256=e8c1169c4af66c3184d7ee110713d4be04befe671fa358e2c1591ee840553b03
RUNTIME_SHA256=617aa77314d57c8b8e4fae9c3f0b35af7b30d5529cb69e98f64f92510ed98a46

say() { printf 'appimage.sh: %s\n' "$*"; }

TOOL=$TOOLS/appimagetool-x86_64.AppImage
RUNTIME=$TOOLS/runtime-x86_64
for f in "$TOOL" "$RUNTIME"; do
  [ -f "$f" ] || { say "$f is needed (github.com/AppImage/appimagetool, AppImage/type2-runtime)"; exit 1; }
done
echo "$TOOL_SHA256  $TOOL" | sha256sum -c --quiet - || { say "$TOOL is not the pinned appimagetool"; exit 1; }
echo "$RUNTIME_SHA256  $RUNTIME" | sha256sum -c --quiet - || { say "$RUNTIME is not the pinned runtime"; exit 1; }
[ "$(uname -m)" = x86_64 ] || { say "the pinned runtime is x86_64's"; exit 1; }

cd "$CRATES"
say "building the releasable desktop"
cargo build -q --release -p rhtn-desktop --features gui
BIN=$CRATES/target/release/rhtn-desktop
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' desktop/Cargo.toml | head -1)

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
APP=$WORK/$NAME.AppDir

install -Dm755 "$BIN" "$APP/usr/bin/$NAME"
install -Dm644 brand/icon.png "$APP/$NAME.png"
# a script rather than a link, so the binary runs under its own name and
# says `fuerosnet` in its usage rather than `AppRun`
printf '#!/bin/sh\nexec "$(dirname "$(readlink -f "$0")")/usr/bin/%s" "$@"\n' "$NAME" > "$APP/AppRun"
chmod 755 "$APP/AppRun"
cat > "$APP/$NAME.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=FuerosNet
Comment=A trust network metered against physical presence
Exec=$NAME
Icon=$NAME
Terminal=false
Categories=Network;
EOF
chmod 644 "$APP/$NAME.desktop"

mkdir -p "$OUT"
IMAGE=$OUT/$NAME-$VERSION-x86_64.AppImage
# run from a copy, the download keeping no execute bit of its own, and
# extracted and run, so the tool needs no FUSE on the machine that builds
install -m755 "$TOOL" "$WORK/appimagetool"
ARCH=x86_64 "$WORK/appimagetool" --appimage-extract-and-run --no-appstream \
  --runtime-file "$RUNTIME" "$APP" "$IMAGE" > "$WORK/tool.log" 2>&1 \
  || { cat "$WORK/tool.log"; say "appimagetool refused"; exit 1; }
say "written $IMAGE"
