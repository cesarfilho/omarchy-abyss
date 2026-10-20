#!/bin/bash
# Shows how plugin/abyss-launch differs from the stock omarchy-launch-screensaver it was copied from.
# Run after `omarchy update` when `abyss doctor` says the stock launcher changed.
here=$(cd "$(dirname "$0")/.." && pwd)
up=$(command -v omarchy-launch-screensaver) || { echo "omarchy-launch-screensaver not found" >&2; exit 1; }
diff -u --label "stock: $up" --label "abyss: plugin/abyss-launch" "$up" "$here/plugin/abyss-launch"
