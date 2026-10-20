#!/bin/bash
# Build abyss, install the binary + launchers + shell plugin.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release
install -Dm755 target/release/abyss ~/.local/bin/abyss
install -Dm755 plugin/abyss-launch ~/.local/bin/abyss-launch
install -Dm755 plugin/abyss-screensaver ~/.local/bin/abyss-screensaver
dest=~/.config/omarchy/plugins/io.github.cesarfilho.abyss
mkdir -p "$dest" && cp plugin/*.qml plugin/manifest.json "$dest"/
echo "Done. Enable: omarchy plugin enable io.github.cesarfilho.abyss && omarchy bar put io.github.cesarfilho.abyss --section right"
echo "Then set idle.screensaver to 0 in ~/.config/omarchy/shell.json so the built-in one does not also fire."
