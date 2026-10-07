#!/usr/bin/env bash
# Build Workly, copy it to /Applications and link wly into ~/.local/bin.
#   scripts/install.sh              build, then install
#   scripts/install.sh --no-build   install the existing release build
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

APP=target/release/bundle/macos/Workly.app
DEST=/Applications/Workly.app
BIN="$DEST/Contents/MacOS/workly-app"
LINK="$HOME/.local/bin/wly"

if [ "${1:-}" != "--no-build" ]; then
  pnpm install --frozen-lockfile
  pnpm tauri build
fi
[ -d "$APP" ] || { echo "No build at $APP. Run without --no-build." >&2; exit 1; }

if pgrep -qf "$BIN"; then
  echo "Workly is running. Quit it (⌘Q), then run this again." >&2
  exit 1
fi

# Replaces only the app bundle; workspaces and settings live elsewhere.
rm -rf "$DEST"
ditto "$APP" "$DEST"
echo "Installed $DEST ($(codesign -dv "$DEST" 2>&1 | grep -o 'Signature=.*'))"

# Same link as Settings → Install CLI: the app binary runs as the CLI when called wly.
mkdir -p "$(dirname "$LINK")"
if [ -e "$LINK" ] && [ ! -L "$LINK" ]; then
  echo "$LINK exists and is not a link; left alone. Remove it and run again to link wly." >&2
else
  ln -sfn "$BIN" "$LINK"
  echo "Linked $LINK -> $BIN"
fi
case ":$PATH:" in
  *":$HOME/.local/bin:"*) "$LINK" --version ;;
  *) echo "Add ~/.local/bin to your PATH, e.g.: echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc" ;;
esac
