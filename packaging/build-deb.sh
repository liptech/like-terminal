#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
pkg="like-terminal_${version}_amd64"
root="build-deb/${pkg}"

cargo build --release --locked

rm -rf "${root}"
mkdir -p "${root}/DEBIAN"
mkdir -p "${root}/usr/bin"
mkdir -p "${root}/usr/share/applications"
mkdir -p "${root}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${root}/usr/share/metainfo"
mkdir -p "${root}/usr/share/doc/like-terminal"

install -m755 target/release/like-terminal "${root}/usr/bin/like-terminal"
install -m644 data/io.github.liketerminal.LikeTerminal.desktop \
  "${root}/usr/share/applications/"
install -m644 data/io.github.liketerminal.LikeTerminal.svg \
  "${root}/usr/share/icons/hicolor/scalable/apps/"
install -m644 data/io.github.liketerminal.LikeTerminal.metainfo.xml \
  "${root}/usr/share/metainfo/"
install -m644 LICENSE PRIVACY.md TERMS.md "${root}/usr/share/doc/like-terminal/"

cat > "${root}/DEBIAN/control" <<EOF
Package: like-terminal
Version: ${version}
Section: x11
Priority: optional
Architecture: amd64
Maintainer: Felipe Mendes <liptech@users.noreply.github.com>
Depends: libc6, libgcc-s1, libglib2.0-0t64 | libglib2.0-0, libgtk-4-1 (>= 4.14), libadwaita-1-0 (>= 1.5), libvte-2.91-gtk4-0 (>= 0.76), libpango-1.0-0, libdbus-1-3, openssh-client
Recommends: fish
Description: Terminal GTK4 com abas, paineis divididos e conexoes SSH
 Like Terminal e um emulador de terminal para GNOME com abas,
 paineis divididos, gerenciador de sessoes, conexoes SSH
 (senha, chave ou ssh-agent), paletas de cores e realce de
 links e de busca.
EOF

dpkg-deb --root-owner-group --build "${root}" "build-deb/like-terminal_${version}_amd64.deb" >/dev/null
echo "build-deb/like-terminal_${version}_amd64.deb"
