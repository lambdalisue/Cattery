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

# macOS before 26 shows the icns, the light drawing.
icons=$(mktemp -d)
trap 'rm -rf "$icons"' EXIT
for size in 128 256 512 1024; do
  resvg -w "$size" -h "$size" "$root/assets/logo/cattery-icon-light.svg" "$icons/$size.png"
done
perl "$root/scripts/icns.pl" "$contents/Resources/cattery.icns" \
  ic07="$icons/128.png" ic08="$icons/256.png" ic09="$icons/512.png" ic10="$icons/1024.png"

# macOS 26 draws the icon from the Icon Composer document instead, light or
# dark as the icon style is. Only Xcode's actool
# compiles it, so a bundle made without Xcode, as Nix makes it, keeps the
# icns alone. The icns stays the one macOS before 26 shows: the one actool
# writes too, whatever it is told, is drawn in macOS 26's style.
if xcrun --find actool >/dev/null 2>&1; then
  mkdir "$icons/compiled"
  xcrun actool "$root/assets/logo/cattery.icon" \
    --compile "$icons/compiled" \
    --app-icon cattery \
    --platform macosx \
    --target-device mac \
    --minimum-deployment-target 11.0 \
    --include-all-app-icons \
    --enable-on-demand-resources NO \
    --development-region en \
    --standalone-icon-behavior none \
    --output-partial-info-plist "$icons/partial.plist" >/dev/null
  cp "$icons/compiled/Assets.car" "$contents/Resources/Assets.car"
fi
