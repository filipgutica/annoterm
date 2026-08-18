#!/bin/zsh

set -euo pipefail

if (( $# > 1 )) || { (( $# == 1 )) && [[ "$1" != "--host-only" ]]; }; then
  print -u2 "usage: $0 [--host-only]"
  exit 64
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  print -u2 "Annoterm.app can only be built on macOS."
  exit 1
fi

host_only=0
if (( $# == 1 )); then
  host_only=1
fi

required_commands=(cargo osacompile plutil codesign)
if (( ! host_only )); then
  required_commands+=(lipo)
fi

for command in "${required_commands[@]}"; do
  command -v "$command" >/dev/null || {
    print -u2 "required command not found: $command"
    exit 1
  }
done

script_dir="${0:A:h}"
repo_root="${script_dir:h}"
output_dir="$repo_root/dist"
bundle="$output_dir/Annoterm.app"

if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
  if [[ "$CARGO_TARGET_DIR" = /* ]]; then
    target_dir="$CARGO_TARGET_DIR"
  else
    target_dir="$repo_root/$CARGO_TARGET_DIR"
  fi
else
  target_dir="$repo_root/target"
fi

cd "$repo_root"
if (( host_only )); then
  cargo build --locked --release --bin annoterm
  binaries=("$target_dir/release/annoterm")
else
  binaries=()
  for target in aarch64-apple-darwin x86_64-apple-darwin; do
    if ! cargo build --locked --release --bin annoterm --target "$target"; then
      print -u2 "could not build $target; install it with: rustup target add $target"
      exit 1
    fi

    binaries+=("$target_dir/$target/release/annoterm")
  done
fi

for binary in "${binaries[@]}"; do
  [[ -x "$binary" ]] || {
    print -u2 "built annoterm binary not found: $binary"
    exit 1
  }
done

mkdir -p "$output_dir"
rm -rf -- "$bundle"
osacompile -o "$bundle" "$repo_root/macos/AnnotermLauncher.applescript"
launcher_executable="$(plutil -extract CFBundleExecutable raw "$bundle/Contents/Info.plist")"
cp "$repo_root/macos/Info.plist" "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleExecutable $launcher_executable" "$bundle/Contents/Info.plist"
mkdir -p "$bundle/Contents/Resources/bin"
if (( host_only )); then
  install -m 755 "$binaries[1]" "$bundle/Contents/Resources/bin/annoterm"
else
  lipo -create "${binaries[@]}" -output "$bundle/Contents/Resources/bin/annoterm"
fi

plutil -lint "$bundle/Contents/Info.plist" >/dev/null
codesign --force --deep --sign - "$bundle"

if (( host_only )); then
  print "built host-only $bundle"
else
  print "built universal $bundle"
fi
