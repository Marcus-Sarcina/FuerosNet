#!/bin/bash
# **The desktop as a Debian package**: the releasable build with its
# window, its binary at /usr/bin as `fuerosnet` [author, 2026-11-27: "use
# fuerosnet"], a launcher entry and its icon, and the
# libraries it links named as its dependencies by `dpkg-shlibdeps`, so
# what it needs is read off the binary rather than kept by hand.
#
#    crates/desktop/packaging/deb.sh [<out dir>]     # target/deb by default
#
# Recommended, not required: `openssh-client`, which administering a node
# this identity runs reaches it through, and `sane-utils`, whose
# `scanimage` answers a resource's scan. The maintainer named is whoever
# builds it, from their git identity.
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
CRATES=$(cd "$HERE/../.." && pwd)
OUT=${1:-$CRATES/target/deb}
NAME=fuerosnet

say() { printf 'deb.sh: %s\n' "$*"; }

for tool in dpkg-deb dpkg-shlibdeps fakeroot; do
  command -v "$tool" > /dev/null || { say "$tool is needed"; exit 1; }
done

cd "$CRATES"
say "building the releasable desktop"
cargo build -q --release -p rhtn-desktop --features gui
BIN=$CRATES/target/release/rhtn-desktop

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' desktop/Cargo.toml | head -1)
ARCH=$(dpkg --print-architecture)
WHO="$(git config user.name) <$(git config user.email)>"

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
STAGE=$WORK/stage

install -Dm755 "$BIN" "$STAGE/usr/bin/$NAME"
install -Dm644 brand/icon.png "$STAGE/usr/share/pixmaps/$NAME.png"
install -d "$STAGE/usr/share/applications"
cat > "$STAGE/usr/share/applications/$NAME.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=FuerosNet
Comment=A trust network metered against physical presence
Exec=$NAME
Icon=$NAME
Terminal=false
Categories=Network;
EOF
chmod 644 "$STAGE/usr/share/applications/$NAME.desktop"

# **what it links, as the distribution names it**: dpkg-shlibdeps reads
# the binary's NEEDED entries and wants a source package to answer for
mkdir -p "$WORK/debian"
printf 'Source: %s\n' "$NAME" > "$WORK/debian/control"
DEPENDS=$(cd "$WORK" && dpkg-shlibdeps -O -e "$STAGE/usr/bin/$NAME" 2> /dev/null \
  | sed -n 's/^shlibs:Depends=//p')
[ -n "$DEPENDS" ] || { say "dpkg-shlibdeps named no dependencies"; exit 1; }

install -d "$STAGE/DEBIAN"
cat > "$STAGE/DEBIAN/control" <<EOF
Package: $NAME
Version: $VERSION
Architecture: $ARCH
Maintainer: $WHO
Depends: $DEPENDS
Recommends: openssh-client, sane-utils
Section: net
Priority: optional
Description: FuerosNet desktop client
 A participant's desktop for the Reconfigurable-Hierarchic Trust Network:
 its identities, conversations, subnets and their resources, paired from
 a phone or restored from the phrase.
EOF

mkdir -p "$OUT"
DEB=$OUT/${NAME}_${VERSION}_${ARCH}.deb
fakeroot dpkg-deb --build --root-owner-group "$STAGE" "$DEB" > /dev/null
say "written $DEB"
