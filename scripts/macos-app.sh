#!/usr/bin/env bash
# Assemble Cattery.app around a built executable:
#   macos-app.sh <executable> <version> <Cattery.app>
# The Nix package and the release bundle both run this, so the app users
# install from either is put together the same way. Needs resvg and perl.
set -euo pipefail

executable=$1
version=$2
app=$3
root="$(cd "$(dirname "$0")/.." && pwd)"

contents="$app/Contents"
mkdir -p "$contents/MacOS" "$contents/Resources"
install -m755 "$executable" "$contents/MacOS/cattery"
perl -pe "s/\@VERSION\@/$version/g" "$root/assets/macos/Info.plist" >"$contents/Info.plist"

# The bundle's icon is the light drawing; the Dock shows the one for the
# theme in force once Cattery runs.
icons=$(mktemp -d)
trap 'rm -rf "$icons"' EXIT
for size in 128 256 512 1024; do
  resvg -w "$size" -h "$size" "$root/assets/logo/cattery-icon-light.svg" "$icons/$size.png"
done
perl "$root/scripts/icns.pl" "$contents/Resources/cattery.icns" \
  ic07="$icons/128.png" ic08="$icons/256.png" ic09="$icons/512.png" ic10="$icons/1024.png"
