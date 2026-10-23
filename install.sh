#!/bin/bash
# Build Abyss and install it for the current user.
#   ./install.sh            build + install binary, launchers, setup tool and shell plugin
#   ./install.sh --setup    the above, then wire it into Omarchy (same as `abyss-setup enable`)
set -euo pipefail
cd "$(dirname "$0")"

BIN_DIR=${ABYSS_BIN_DIR:-$HOME/.local/bin}
DATA_DIR=${XDG_DATA_HOME:-$HOME/.local/share}/abyss

cargo build --release
install -Dm755 target/release/abyss "$BIN_DIR/abyss"
install -Dm755 bin/abyss-launch "$BIN_DIR/abyss-launch"
install -Dm755 bin/abyss-screensaver "$BIN_DIR/abyss-screensaver"
install -Dm755 scripts/abyss-setup "$BIN_DIR/abyss-setup"
# The plugin files live next to the user data so `abyss-setup plugin` works without this checkout.
mkdir -p "$DATA_DIR/plugin"
cp *.qml manifest.json "$DATA_DIR/plugin/"
"$BIN_DIR/abyss-setup" plugin
"$BIN_DIR/abyss-setup" record-upstream

if [[ ${1:-} == --setup ]]; then
  "$BIN_DIR/abyss-setup" enable
else
  echo
  echo "Installed. To plug it into Omarchy (plugin, bar widget, idle timer, menu): abyss-setup enable"
  echo "Check everything with: abyss doctor"
fi
