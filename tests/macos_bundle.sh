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

assert_file_contains() {
  local needle="$1"
  local file="$2"

  [[ "$(<"$file")" == *"$needle"* ]] || {
    print -u2 "missing expected text in $file: $needle"
    exit 1
  }
}

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
assert_file_contains '<string>md</string>' "$plist"
assert_file_contains '<string>markdown</string>' "$plist"

assert_file_contains 'quoted form of coreBinary' "$launcher_source"
assert_file_contains 'quoted form of markdownPath' "$launcher_source"
assert_file_contains 'tell application "/System/Applications/Utilities/Terminal.app"' "$launcher_source"
assert_file_contains 'on error errorMessage number errorNumber' "$launcher_source"
assert_file_contains 'errorNumber is -1743' "$launcher_source"
assert_file_contains 'System Settings > Privacy & Security > Automation' "$launcher_source"
assert_file_contains '--host-only' "$repo_root/scripts/build-macos-app.sh"
assert_file_contains 'lipo -create' "$repo_root/scripts/build-macos-app.sh"
assert_file_contains 'codesign --verify --deep --strict "$staged_destination"' "$repo_root/scripts/install-macos-app.sh"
assert_file_contains 'mv "$destination" "$backup_destination"' "$repo_root/scripts/install-macos-app.sh"
assert_file_contains 'mv "$backup_destination" "$destination"' "$repo_root/scripts/install-macos-app.sh"

codesign --verify --deep --strict "$bundle"

print "macOS app bundle is valid: $bundle"
