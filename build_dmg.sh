#!/bin/zsh
# Build dist/mikocok-<version>.dmg: a universal (Apple Silicon + Intel) disk
# image with both apps and an Applications shortcut. No Rust needed to use it.
set -euo pipefail
cd "${0:A:h}"
./build_app.sh --universal --no-install

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
DMG="dist/mikocok-$VERSION.dmg"
STAGE=$(mktemp -d)
cp -R dist/mikocokWord.app dist/mikocokExcel.app "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "mikocok" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
rm -rf "$STAGE"
echo "Built: $DMG ($(du -h "$DMG" | cut -f1))"
shasum -a 256 "$DMG"
