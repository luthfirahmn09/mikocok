#!/bin/zsh
# Build mikocokWord.app and mikocokExcel.app (one binary, two launchers) and
# install them to ~/Applications. Pass --default to make them the default
# apps for .docx / .xlsx.
set -euo pipefail
cd "${0:A:h}"
cargo build --release
rm -rf dist && mkdir dist
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
  cp target/release/mikocok "$app/Contents/Resources/mikocok"
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
  mkdir -p ~/Applications && rm -rf ~/Applications/$name.app && cp -R "$app" ~/Applications/
  $LSREG -f ~/Applications/$name.app
  echo "Installed: ~/Applications/$name.app ($(du -sh ~/Applications/$name.app | cut -f1))"
}

build Word  "Word Document"  docx org.openxmlformats.wordprocessingml.document
build Excel "Excel Workbook" xlsx org.openxmlformats.spreadsheetml.sheet

if [[ "${1:-}" == "--default" ]]; then
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
