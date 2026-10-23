#!/bin/bash
# Remove Abyss from the current user. Game data is kept unless --purge is given.
set -euo pipefail
cd "$(dirname "$0")"

BIN_DIR=${ABYSS_BIN_DIR:-$HOME/.local/bin}
DATA_DIR=${XDG_DATA_HOME:-$HOME/.local/share}/abyss
STATE_DIR=${ABYSS_STATE_DIR:-${XDG_STATE_HOME:-$HOME/.local/state}/abyss}
APP_CONFIG_DIR=${ABYSS_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/abyss}
SETUP_BIN=$BIN_DIR/abyss-setup

if [[ -x $SETUP_BIN ]]; then
  "$SETUP_BIN" disable
elif command -v abyss-setup >/dev/null; then
  abyss-setup disable
else
  scripts/abyss-setup disable
fi
rm -rf "${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins/io.github.cesarfilho.abyss" "$DATA_DIR"
rm -f "$BIN_DIR/abyss" "$BIN_DIR/abyss-launch" "$BIN_DIR/abyss-screensaver" "$SETUP_BIN"
if [[ ${1:-} == --purge ]]; then
  rm -rf "$STATE_DIR" "$APP_CONFIG_DIR"
  echo "Removed Abyss and all its data."
else
  echo "Removed Abyss. Your progress is in $STATE_DIR (use --purge to delete it)."
fi
