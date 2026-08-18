#!/bin/zsh

set -euo pipefail

if (( $# > 1 )) || (( $# == 1 && "$1" != "--replace" )); then
  print -u2 "usage: $0 [--replace]"
  exit 64
fi

script_dir="${0:A:h}"
repo_root="${script_dir:h}"
source_bundle="$repo_root/dist/Annoterm.app"
destination="/Applications/Annoterm.app"
staged_destination="/Applications/.Annoterm.app.install.$$"
backup_destination="/Applications/.Annoterm.app.backup.$$"

[[ -d "$source_bundle" ]] || {
  print -u2 "missing $source_bundle; run scripts/build-macos-app.sh first"
  exit 1
}

if [[ -e "$destination" ]]; then
  if (( $# == 0 )); then
    print -u2 "$destination already exists; rerun with --replace to replace it"
    exit 1
  fi

fi

cleanup() {
  rm -rf -- "$staged_destination"
  if [[ -e "$backup_destination" && ! -e "$destination" ]]; then
    mv "$backup_destination" "$destination"
  fi
}
trap cleanup EXIT

ditto "$source_bundle" "$staged_destination"
codesign --verify --deep --strict "$staged_destination"

if [[ -e "$destination" ]]; then
  mv "$destination" "$backup_destination"
fi
mv "$staged_destination" "$destination"
rm -rf -- "$backup_destination"
trap - EXIT

print "installed $destination"
print "Choose Annoterm in Finder's Get Info > Open with menu, then select Change All to make it the default."
