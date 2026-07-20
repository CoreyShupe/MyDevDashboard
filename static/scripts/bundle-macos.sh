#!/usr/bin/env bash
# Build a macOS .app bundle for the Dev Dashboard.
#
# Output: builds/macos/DevDashboard.app  (a real, double-clickable app bundle).
#
# Design (AGENTS.md §10 / task spec):
#   * The bundle's executable is a SYMLINK to the release target, not a self-contained
#     binary copy — so a rebuild (`cargo build --release`) is picked up with no re-bundle,
#     and the bundle stays tiny. The link is ABSOLUTE (to this repo's target/release/<bin>)
#     so it's a single clear path, not a fragile chain of `../`.
#   * The bundle's CFBundleExecutable is a LAUNCHER SCRIPT (not the binary directly). Finder
#     launches apps with cwd=/, but the app loads its `.env` via dotenvy from the working
#     directory. So the launcher cd's into the bundle (Contents/Resources, where we copy the
#     `.env`) before running the symlinked binary — that's how `.env` is found.
#   * The launcher does NOT loop on the in-app Restart (exit 86): in a Finder-launched bundle
#     the relaunch doesn't work, so Restart just quits. Restart-relaunch stays a `dev-dash open`
#     (dev) feature only. The launcher only cd's for `.env` and execs the binary.
#   * We copy the repo's `.env` into the bundle so it is self-contained w.r.t. config.
set -euo pipefail

# These scripts live at static/scripts/, so the repo root is two levels up (matches _common.sh).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

# Binary name from Cargo.toml (the release artifact target/release/<bin>).
BIN_NAME="$(grep -m1 '^name' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
RELEASE_BIN="$REPO_ROOT/target/release/$BIN_NAME"
ICNS_SRC="$REPO_ROOT/static/assets/icon/AppIcon.icns"

APP_NAME="DevDashboard"
OUT_DIR="$REPO_ROOT/builds/macos"
APP="$OUT_DIR/$APP_NAME.app"
CONTENTS="$APP/Contents"
MACOS_DIR="$CONTENTS/MacOS"
RES_DIR="$CONTENTS/Resources"

# 1. Build the release binary the bundle will point at.
echo "bundle: building release binary ($BIN_NAME) …"
cargo build --release

if [[ ! -f "$RELEASE_BIN" ]]; then
  echo "bundle: error: expected release binary at $RELEASE_BIN after build" >&2
  exit 1
fi

# 2. Clean + recreate the bundle skeleton (idempotent — always a fresh bundle).
echo "bundle: assembling $APP …"
rm -rf "$APP"
mkdir -p "$MACOS_DIR" "$RES_DIR"

# 3. Info.plist — CFBundleExecutable points at the launcher script below.
cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>Dev Dashboard</string>
    <key>CFBundleIdentifier</key>
    <string>io.github.coreyshupe.devdashboard</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

# 4. Symlink to the release target (NOT a copy). Absolute path — one clear target, and a
#    rebuild is reflected immediately since the bundle just points at target/release/.
ln -sfn "$RELEASE_BIN" "$MACOS_DIR/$BIN_NAME"

# 4b. App icon. Copy the committed AppIcon.icns (matching CFBundleIconFile above) into
#     Resources; regenerate it from the SVG source first if it's missing (self-healing).
if [[ ! -f "$ICNS_SRC" ]]; then
  echo "bundle: AppIcon.icns missing — generating it from the SVG source …"
  "$SCRIPT_DIR/icon-gen.sh"
fi
if [[ -f "$ICNS_SRC" ]]; then
  cp "$ICNS_SRC" "$RES_DIR/AppIcon.icns"
  echo "bundle: copied AppIcon.icns -> $RES_DIR/AppIcon.icns"
else
  echo "bundle: warning: no app icon; the bundle will use the generic default icon" >&2
fi

# 5. Launcher script = the bundle's CFBundleExecutable. It resolves its own location, cd's into
#    Contents/Resources (where `.env` lives) so dotenvy finds config, then execs the symlinked
#    binary. Finder launches with cwd=/, so the cd is what makes `.env` load correctly. It does
#    NOT loop on the Restart exit code (86): relaunch doesn't work in a Finder-launched bundle,
#    so Restart simply quits here (that stays a `dev-dash open` dev feature). The symlink means
#    an external `cargo build --release` is still picked up on the next launch.
#    The placeholder below is substituted after the (quoted) heredoc so the body stays literal.
cat > "$MACOS_DIR/$APP_NAME" <<'LAUNCH'
#!/usr/bin/env bash
set -euo pipefail

# Resolve this launcher's real directory (Contents/MacOS), following any symlinks.
SOURCE="${BASH_SOURCE[0]}"
while [ -L "$SOURCE" ]; do
  DIR=$( cd -- "$( dirname -- "$SOURCE" )" &>/dev/null && pwd )
  SOURCE=$( readlink "$SOURCE" )
  [[ $SOURCE != /* ]] && SOURCE="$DIR/$SOURCE"
done
HERE=$( cd -- "$( dirname -- "$SOURCE" )" &>/dev/null && pwd )

# cd into the bundle's Resources dir, which holds the copied `.env`, so dotenvy loads it.
cd "$HERE/../Resources"

# Run the app, replacing this launcher process (no relaunch loop — Restart just quits).
exec "$HERE/__BIN_NAME__" "$@"
LAUNCH
# Substitute the real binary name into the launcher.
sed -i '' -e "s/__BIN_NAME__/$BIN_NAME/" "$MACOS_DIR/$APP_NAME"
chmod +x "$MACOS_DIR/$APP_NAME"

# 6. Copy the repo's `.env` into the bundle so it carries its own config.
if [[ -f "$REPO_ROOT/.env" ]]; then
  cp "$REPO_ROOT/.env" "$RES_DIR/.env"
  echo "bundle: copied .env -> $RES_DIR/.env"
else
  echo "bundle: warning: no .env at repo root; the app will error until one exists in the bundle" >&2
fi

echo "bundle: done -> $APP"

# 7. Optional install: `bundle copy` also drops the bundle into /Applications so Spotlight and
#    Launchpad find it. The binary symlink is ABSOLUTE (§4), so the installed copy still resolves
#    it; `cp -R` preserves the symlink (does not follow it). We replace any existing install.
if [[ "${1:-}" == "copy" ]]; then
  DEST="/Applications/$APP_NAME.app"
  echo "bundle: installing -> $DEST"
  rm -rf "$DEST"
  if cp -R "$APP" "$DEST" 2>/dev/null; then
    # Nudge LaunchServices so Finder/Spotlight pick up the (possibly changed) icon + metadata.
    /usr/bin/touch "$DEST"
    echo "bundle: installed. Find it in Spotlight/Launchpad as \"Dev Dashboard\", or:  open \"$DEST\""
  else
    echo "bundle: error: could not write to /Applications (permission?). The bundle is still at" >&2
    echo "        $APP — copy it manually, e.g.:  sudo cp -R \"$APP\" /Applications/" >&2
    exit 1
  fi
else
  echo "bundle: launch with:  open \"$APP\"   (or 'dev-dash bundle copy' to install to /Applications)"
fi
