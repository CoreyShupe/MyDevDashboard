#!/usr/bin/env bash
# Generate the macOS app icon (AppIcon.icns) from the SVG source of truth.
#
#   static/assets/icon/AppIcon.svg  --(sips raster)-->  1024 master PNG
#     --(sips, all iconset sizes)--> AppIcon.iconset  --(iconutil)--> AppIcon.icns
#
# The icon is self-drawn from the design system (see AppIcon.svg), so there's no external
# art. macOS-only (uses `sips` + `iconutil`) — see AGENTS.md §14. We rasterize with `sips`,
# NOT QuickLook (`qlmanage`): qlmanage flattens the SVG's transparency onto opaque white,
# leaving an ugly white border, whereas `sips` renders SVG natively AND preserves alpha.
# The committed AppIcon.icns is what `dev-dash bundle` copies into the bundle; re-run this
# whenever you edit AppIcon.svg.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

ICON_DIR="$REPO_ROOT/static/assets/icon"
SVG="$ICON_DIR/AppIcon.svg"
ICNS="$ICON_DIR/AppIcon.icns"

if [[ ! -f "$SVG" ]]; then
  echo "icon-gen: error: missing source SVG at $SVG" >&2
  exit 1
fi

# Scratch workspace (cleaned on exit) for the master PNG + the .iconset tree.
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
ICONSET="$WORK/AppIcon.iconset"
mkdir -p "$ICONSET"

# 1. Rasterize the SVG to a 1024 master with `sips` (renders SVG natively AND keeps alpha; see
#    the header note on why NOT qlmanage). Then normalize to exactly 1024 in case the source
#    viewBox ever changes.
echo "icon-gen: rasterizing $SVG -> 1024 master …"
MASTER="$WORK/master.png"
sips -s format png "$SVG" --out "$MASTER" >/dev/null 2>&1 || true
if [[ ! -f "$MASTER" ]]; then
  echo "icon-gen: error: sips did not rasterize $SVG (is this macOS?)" >&2
  exit 1
fi
sips -z 1024 1024 "$MASTER" >/dev/null

# 2. Downscale the master into every size macOS wants in an .iconset (1x + @2x pairs).
#    name=<file base>  px=<pixel size>
emit() { sips -z "$2" "$2" "$MASTER" --out "$ICONSET/$1.png" >/dev/null; }
echo "icon-gen: generating iconset sizes …"
emit icon_16x16        16
emit icon_16x16@2x     32
emit icon_32x32        32
emit icon_32x32@2x     64
emit icon_128x128     128
emit icon_128x128@2x  256
emit icon_256x256     256
emit icon_256x256@2x  512
emit icon_512x512     512
emit icon_512x512@2x 1024

# 3. Pack the iconset into a multi-resolution .icns.
echo "icon-gen: packing -> $ICNS"
iconutil -c icns "$ICONSET" -o "$ICNS"

# 4. Also emit a standalone 512 PNG for the app to EMBED (src/main.rs `include_bytes!` +
#    `with_icon`). eframe sets a DEFAULT egui icon on macOS unless handed one, overriding even
#    the bundle's .icns — so the running app embeds this and hands it to eframe to own its Dock
#    icon on every launch path (bundle, `cargo run`, `dev-dash open`). See AGENTS.md §14.
EMBED_PNG="$ICON_DIR/AppIcon-512.png"
sips -z 512 512 "$MASTER" --out "$EMBED_PNG" >/dev/null
echo "icon-gen: wrote embed PNG -> $EMBED_PNG"

echo "icon-gen: done -> $ICNS (+ $EMBED_PNG)"
