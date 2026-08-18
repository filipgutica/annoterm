#!/bin/zsh

set -euo pipefail

if (( $# < 1 || $# > 2 )) || { (( $# == 2 )) && [[ "$2" != "--host-only" ]]; }; then
  print -u2 "usage: $0 <Annoterm.app> [--host-only]"
  exit 64
fi

bundle="$1"
host_only=false
if (( $# == 2 )); then
  host_only=true
fi
plist="$bundle/Contents/Info.plist"
test_dir="${0:A:h}"
repo_root="${test_dir:h}"
launcher_source="$repo_root/macos/AnnotermLauncher.applescript"

[[ -d "$bundle" ]] || {
  print -u2 "missing app bundle: $bundle"
  exit 1
}
launcher_executable="$(plutil -extract CFBundleExecutable raw "$plist")"
[[ -x "$bundle/Contents/MacOS/$launcher_executable" ]] || {
  print -u2 "missing compiled AppleScript launcher"
  exit 1
}
[[ -x "$bundle/Contents/Resources/bin/annoterm" ]] || {
  print -u2 "missing bundled annoterm binary"
  exit 1
}

if "$host_only"; then
  lipo -archs "$bundle/Contents/Resources/bin/annoterm" >/dev/null
else
  binary_architectures="$(lipo -archs "$bundle/Contents/Resources/bin/annoterm")"
  [[ " $binary_architectures " == *" arm64 "* ]]
  [[ " $binary_architectures " == *" x86_64 "* ]]
fi

plutil -lint "$plist" >/dev/null

[[ "$(plutil -extract CFBundleIdentifier raw "$plist")" == "com.annoterm.app" ]]
[[ "$(plutil -extract CFBundlePackageType raw "$plist")" == "APPL" ]]
[[ "$(plutil -extract CFBundleDocumentTypes.0.CFBundleTypeRole raw "$plist")" == "Editor" ]]
[[ "$(plutil -extract CFBundleDocumentTypes.0.LSHandlerRank raw "$plist")" == "Alternate" ]]
[[ "$(plutil -extract CFBundleDocumentTypes.0.LSItemContentTypes.0 raw "$plist")" == "net.daringfireball.markdown" ]]
[[ "$(plutil -extract UTImportedTypeDeclarations.0.UTTypeIdentifier raw "$plist")" == "net.daringfireball.markdown" ]]
rg -Fq '<string>md</string>' "$plist"
rg -Fq '<string>markdown</string>' "$plist"

rg -Fq 'quoted form of coreBinary' "$launcher_source"
rg -Fq 'quoted form of markdownPath' "$launcher_source"
rg -Fq 'tell application "/System/Applications/Utilities/Terminal.app"' "$launcher_source"
rg -Fq 'on error errorMessage number errorNumber' "$launcher_source"
rg -Fq 'errorNumber is -1743' "$launcher_source"
rg -Fq 'System Settings > Privacy & Security > Automation' "$launcher_source"
rg -Fq -- '--host-only' "$repo_root/scripts/build-macos-app.sh"
rg -Fq 'lipo -create' "$repo_root/scripts/build-macos-app.sh"
rg -Fq 'codesign --verify --deep --strict "$staged_destination"' "$repo_root/scripts/install-macos-app.sh"
rg -Fq 'mv "$destination" "$backup_destination"' "$repo_root/scripts/install-macos-app.sh"
rg -Fq 'mv "$backup_destination" "$destination"' "$repo_root/scripts/install-macos-app.sh"

codesign --verify --deep --strict "$bundle"

print "macOS app bundle is valid: $bundle"
