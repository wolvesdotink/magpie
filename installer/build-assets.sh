#!/usr/bin/env bash
# Regenerate DMG installer assets from the SVG source.
#
# Run this after editing dmg-background.svg. The rendered PNGs and the
# multi-rep TIFF are committed to the repo so a normal `bash scripts/build-macos.sh`
# does not depend on rsvg-convert being installed.
#
# Requires:
#   brew install librsvg   (provides rsvg-convert)
#   tiffutil               (ships with macOS)

set -euo pipefail

cd "$(dirname "$0")"

SRC="dmg-background.svg"
PNG_1X="dmg-background.png"
PNG_2X="dmg-background@2x.png"
TIFF="dmg-background.tiff"

command -v rsvg-convert >/dev/null || {
    echo "rsvg-convert not found. Install with: brew install librsvg" >&2
    exit 1
}
command -v tiffutil >/dev/null || {
    echo "tiffutil not found (should ship with macOS)." >&2
    exit 1
}

echo "Rendering $PNG_1X (660x400)..."
rsvg-convert -w 660  -h 400 "$SRC" -o "$PNG_1X"

echo "Rendering $PNG_2X (1320x800)..."
rsvg-convert -w 1320 -h 800 "$SRC" -o "$PNG_2X"

# Build a multi-rep TIFF that carries both the @1x and @2x rasters.
# tiffutil reads PNG inputs directly on modern macOS; if it ever stops, fall
# back to converting via `sips -s format tiff` first.
echo "Combining into $TIFF..."
tiffutil -cathidpicheck "$PNG_1X" "$PNG_2X" -out "$TIFF"

echo
echo "Done. Wrote:"
echo "  $PNG_1X"
echo "  $PNG_2X"
echo "  $TIFF"
