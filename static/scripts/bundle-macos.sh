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
#   * The launcher honors the in-app Restart (exit 86), relaunching just like `dev-dash open`.
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

# The in-app "Restart" button exits with this code; the launcher loop below relaunches on it.
# MUST match RESTART_EXIT_CODE in src/main.rs (and `restart_code` in dev-dash).
RESTART_CODE=86

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

# 5. Launcher script = the bundle's CFBundleExecutable. It resolves its own location, cd's into
#    Contents/Resources (where `.env` lives) so dotenvy finds config, then runs the symlinked
#    binary in a loop that relaunches on the Restart exit code (86). Finder launches with cwd=/,
#    so the cd is what makes `.env` load correctly. We relaunch (rather than rebuild like
#    `dev-dash open` prod) because a Finder-launched app has a minimal PATH and can't rely on
#    `cargo`; the symlink means any external rebuild is picked up on the next relaunch anyway.
#    Placeholders below are substituted after the (quoted) heredoc so the body stays literal.
cat > "$MACOS_DIR/$APP_NAME" <<'LAUNCH'
#!/usr/bin/env bash
# NOTE: no `set -e` — we intentionally inspect the binary's non-zero exit codes.
set -uo pipefail

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

BIN="$HERE/__BIN_NAME__"
RESTART_CODE=__RESTART_CODE__

# Run the app; on the in-app Restart (exit 86) relaunch, otherwise exit with its code.
while true; do
  "$BIN" "$@"
  code=$?
  [ "$code" -eq "$RESTART_CODE" ] || exit "$code"
done
LAUNCH
# Substitute the real binary name + restart code into the launcher.
sed -i '' \
  -e "s/__BIN_NAME__/$BIN_NAME/" \
  -e "s/__RESTART_CODE__/$RESTART_CODE/" \
  "$MACOS_DIR/$APP_NAME"
chmod +x "$MACOS_DIR/$APP_NAME"

# 6. Copy the repo's `.env` into the bundle so it carries its own config.
if [[ -f "$REPO_ROOT/.env" ]]; then
  cp "$REPO_ROOT/.env" "$RES_DIR/.env"
  echo "bundle: copied .env -> $RES_DIR/.env"
else
  echo "bundle: warning: no .env at repo root; the app will error until one exists in the bundle" >&2
fi

echo "bundle: done -> $APP"
echo "bundle: launch with:  open \"$APP\""
