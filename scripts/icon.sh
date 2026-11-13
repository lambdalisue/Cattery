#!/usr/bin/env bash
# Render the icon's PNGs, which the app embeds, from the SVGs in
# assets/logo: resvg comes from the development shell. Each look has a
# drawing for the Dock and a heavier one for title bars and taskbars.
set -euo pipefail
cd "$(dirname "$0")/../assets/logo"
for look in light dark; do
  resvg -w 512 -h 512 "cattery-icon-${look}.svg" "cattery-icon-${look}-512.png"
  resvg -w 64 -h 64 "cattery-icon-${look}-small.svg" "cattery-icon-${look}-64.png"
done
