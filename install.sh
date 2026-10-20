#!/bin/bash
# Build Abyss and install it for the current user.
#   ./install.sh            build + install binary, launchers, setup tool and shell plugin
#   ./install.sh --setup    the above, then wire it into Omarchy (same as `abyss-setup enable`)
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release
install -Dm755 target/release/abyss ~/.local/bin/abyss
install -Dm755 plugin/abyss-launch ~/.local/bin/abyss-launch
install -Dm755 plugin/abyss-screensaver ~/.local/bin/abyss-screensaver
install -Dm755 scripts/abyss-setup ~/.local/bin/abyss-setup
# The plugin files live next to the user data so `abyss-setup plugin` works without this checkout.
mkdir -p ~/.local/share/abyss/plugin
cp plugin/*.qml plugin/manifest.json ~/.local/share/abyss/plugin/
~/.local/bin/abyss-setup plugin
~/.local/bin/abyss-setup record-upstream

if [[ ${1:-} == --setup ]]; then
  ~/.local/bin/abyss-setup enable
else
  echo
  echo "Installed. To plug it into Omarchy (plugin, bar widget, idle timer, menu): abyss-setup enable"
  echo "Check everything with: abyss doctor"
fi
