#!/usr/bin/env bash
# Package a release build of Cattery the way a GitHub Release carries it:
#   bundle.sh <version> <executable> <dist>
# macOS gets Cattery.app in a disk image, Linux a tarball, Windows a zip,
# each with the licences of what Cattery carries. The release runs this with
# the tag's version; CI runs it with a placeholder, so a release is never the
# first run.
set -euo pipefail

version=$1
executable=$2
dist=$3
root="$(cd "$(dirname "$0")/.." && pwd)"

arch=$(uname -m)
case "$arch" in
  arm64 | aarch64) arch=aarch64 ;;
  x86_64 | amd64) arch=x86_64 ;;
esac

mkdir -p "$dist"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

licenses() {
  mkdir -p "$1/themes/LICENSES"
  cp "$root/LICENSE" "$1/LICENSE"
  cp "$root/crates/cattery/themes/README.md" "$1/themes/README.md"
  cp "$root/crates/cattery/themes/LICENSES/"* "$1/themes/LICENSES/"
}

# A build made in the Nix development shell refers into /nix/store, which
# no machine without that store has; it must not be what a release ships.
refuse_nix_store() {
  if grep -q /nix/store <<<"$1"; then
    echo "error: $executable refers into /nix/store; build it without the Nix shell" >&2
    echo "$1" >&2
    exit 1
  fi
}

case "$(uname -s)" in
  Darwin)
    refuse_nix_store "$(otool -L "$executable")"
    app="$stage/Cattery/Cattery.app"
    "$root/scripts/macos-app.sh" "$executable" "$version" "$app"
    licenses "$app/Contents/Resources/licenses"
    # Signed by the identity CATTERY_SIGN_IDENTITY names, else by "Cattery
    # Release" where the keychain has it (scripts/macos/import-identity.sh),
    # else ad hoc. Every release is signed by the same one, so an update
    # keeps what macOS let the app do; none is a Developer ID, which the
    # Homebrew cask accounts for.
    identity="${CATTERY_SIGN_IDENTITY:-}"
    if [ -z "$identity" ]; then
      # Captured before grepping: under pipefail, grep -q closing the pipe
      # early would fail security instead.
      identities="$(security find-identity -p codesigning)"
      if grep -qF '"Cattery Release"' <<<"$identities"; then
        identity="Cattery Release"
      else
        identity="-"
        echo "no \"Cattery Release\" in the keychain; signing ad hoc" >&2
      fi
    fi
    codesign --force --deep --timestamp=none --sign "$identity" "$app"
    ln -s /Applications "$stage/Cattery/Applications"
    hdiutil create -quiet -volname Cattery -srcfolder "$stage/Cattery" -ov \
      -format UDZO "$dist/Cattery_${version}_${arch}.dmg"
    ;;
  Linux)
    refuse_nix_store "$(readelf -l "$executable" 2>/dev/null; readelf -d "$executable" 2>/dev/null)"
    name="cattery-${version}-linux-${arch}"
    mkdir -p "$stage/$name"
    install -m755 "$executable" "$stage/$name/cattery"
    cp "$root/README.md" "$stage/$name/"
    licenses "$stage/$name/licenses"
    tar -czf "$dist/$name.tar.gz" -C "$stage" "$name"
    ;;
  MINGW* | MSYS* | CYGWIN*)
    name="cattery-${version}-windows-${arch}"
    mkdir -p "$stage/$name"
    cp "$executable" "$stage/$name/cattery.exe"
    cp "$root/README.md" "$stage/$name/"
    licenses "$stage/$name/licenses"
    (cd "$stage" && 7z a -bso0 "$name.zip" "$name")
    mv "$stage/$name.zip" "$dist/"
    ;;
  *)
    echo "error: no bundle for $(uname -s)" >&2
    exit 1
    ;;
esac
ls -l "$dist"
