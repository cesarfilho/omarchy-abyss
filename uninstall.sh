#!/bin/bash
# Remove Abyss from the current user. Game data is kept unless --purge is given.
set -euo pipefail
cd "$(dirname "$0")"

if command -v abyss-setup >/dev/null; then
  abyss-setup disable
else
  scripts/abyss-setup disable
fi
rm -rf ~/.config/omarchy/plugins/io.github.cesarfilho.abyss "${XDG_DATA_HOME:-$HOME/.local/share}/abyss"
rm -f ~/.local/bin/{abyss,abyss-launch,abyss-screensaver,abyss-setup}
if [[ ${1:-} == --purge ]]; then
  rm -rf "${XDG_STATE_HOME:-$HOME/.local/state}/abyss" "${XDG_CONFIG_HOME:-$HOME/.config}/abyss"
  echo "Removed Abyss and all its data."
else
  echo "Removed Abyss. Your progress is in ~/.local/state/abyss (use --purge to delete it)."
fi
