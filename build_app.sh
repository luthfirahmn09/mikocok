#!/bin/zsh
# Build mikocokWord.app and mikocokExcel.app (one binary, two launchers) into
# dist/ and install them to ~/Applications.
#   --default     make them the default apps for .docx / .xlsx
#   --universal   build for Apple Silicon + Intel (used for the DMG)
#   --no-install  only build into dist/, don't touch ~/Applications
set -euo pipefail
cd "${0:A:h}"
DEFAULT=0 UNIVERSAL=0 INSTALL=1
for a in "$@"; do
  case $a in
    --default) DEFAULT=1 ;;
    --universal) UNIVERSAL=1 ;;
    --no-install) INSTALL=0 ;;
    *) echo "unknown option: $a" >&2; exit 1 ;;
  esac
done

rm -rf dist && mkdir dist
if (( UNIVERSAL )); then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
  cargo build --release --target aarch64-apple-darwin
  cargo build --release --target x86_64-apple-darwin
  BIN="$PWD/dist/mikocok-universal"
  lipo -create -output "$BIN" target/aarch64-apple-darwin/release/mikocok target/x86_64-apple-darwin/release/mikocok
else
  cargo build --release
  BIN="$PWD/target/release/mikocok"
fi
LSREG=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

build() {  # flavor  type-name  extension  uti
  local flavor=$1 typename=$2 ext=$3 uti=$4
  local name="mikocok$flavor" app="$PWD/dist/mikocok$flavor.app"
  osacompile -o "$app" <<AS
on run
	do shell script "MIKOCOK=$flavor " & quoted form of (POSIX path of (path to resource "mikocok")) & " >/dev/null 2>&1 &"
end run
on open theFiles
	repeat with f in theFiles
		do shell script "MIKOCOK=$flavor " & quoted form of (POSIX path of (path to resource "mikocok")) & " " & quoted form of (POSIX path of f) & " >/dev/null 2>&1 &"
	end repeat
end open
AS
  cp "$BIN" "$app/Contents/Resources/mikocok"
  python3 - "$app/Contents/Info.plist" "$name" "$flavor" "$typename" "$ext" "$uti" <<'PY'
import plistlib, sys
p, name, flavor, typename, ext, uti = sys.argv[1:]
d = plistlib.load(open(p, "rb"))
d.update({
    "CFBundleName": name, "CFBundleDisplayName": name,
    "CFBundleIdentifier": f"com.luthfi.mikocok.{flavor.lower()}",
    "LSUIElement": True,  # the launcher itself stays out of the Dock
    "CFBundleDocumentTypes": [{
        "CFBundleTypeName": typename, "CFBundleTypeExtensions": [ext],
        "LSItemContentTypes": [uti], "CFBundleTypeRole": "Editor", "LSHandlerRank": "Alternate",
    }],
})
plistlib.dump(d, open(p, "wb"))
PY
  codesign --force --deep -s - "$app"
  if (( INSTALL )); then
    mkdir -p ~/Applications && rm -rf ~/Applications/$name.app && cp -R "$app" ~/Applications/
    $LSREG -f ~/Applications/$name.app
    echo "Installed: ~/Applications/$name.app ($(du -sh ~/Applications/$name.app | cut -f1))"
  else
    echo "Built: dist/$name.app ($(du -sh "$app" | cut -f1))"
  fi
}

build Word  "Word Document"  docx org.openxmlformats.wordprocessingml.document
build Excel "Excel Workbook" xlsx org.openxmlformats.spreadsheetml.sheet

if (( DEFAULT && INSTALL )); then
  swift - <<'SW'
import Foundation
import CoreServices
for (uti, bid) in [("org.openxmlformats.wordprocessingml.document", "com.luthfi.mikocok.word"),
                   ("org.openxmlformats.spreadsheetml.sheet", "com.luthfi.mikocok.excel")] {
    let r = LSSetDefaultRoleHandlerForContentType(NSString(string: uti), LSRolesMask.all, NSString(string: bid))
    print(uti, r == 0 ? "-> \(bid)" : "error \(r)")
}
SW
fi
