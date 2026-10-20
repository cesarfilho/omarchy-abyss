#!/bin/bash
# shellcheck disable=SC2016,SC2034
# End-to-end check of the idle path: shell plugin -> launcher -> one window per monitor -> wake -> clean exit.
#
#   scripts/e2e-idle.sh          trigger through the plugin's IPC handler (fast, ~40 s)
#   scripts/e2e-idle.sh --real   wait for the real idle timer; do not touch the keyboard or mouse
#   scripts/e2e-idle.sh --clean  also delete the progress this run earned
#
# It temporarily lifts stay-awake (restored on exit) because the plugin refuses to launch while it is on.
set -uo pipefail

real=0 clean=0
for a in "$@"; do
  case $a in --real) real=1 ;; --clean) clean=1 ;; *) echo "unknown option $a" >&2; exit 2 ;; esac
done

OMARCHY_PATH=${OMARCHY_PATH:-/usr/share/omarchy}
STAY=$HOME/.local/state/omarchy/indicators/stay-awake
STATE=${XDG_STATE_HOME:-$HOME/.local/state}/abyss
SOCK=${XDG_RUNTIME_DIR:-/tmp}/abyss.sock
fails=0

ok() { printf '  ✓ %s\n' "$1"; }
bad() { printf '  ✗ %s\n' "$1"; fails=$((fails + 1)); }
check() { if eval "$2"; then ok "$1"; else bad "$1"; fi; }
windows() { hyprctl clients -j | jq '[.[] | select(.class == "org.omarchy.screensaver")] | length'; }
abyss_procs() { pgrep -xc abyss || true; }
wait_until() { # seconds, command
  local end=$((SECONDS + $1))
  while ((SECONDS < end)); do eval "$2" && return 0; sleep 0.5; done
  return 1
}
ipc() { quickshell ipc -p "$OMARCHY_PATH/shell" call abyss.service "$1" >/dev/null 2>&1; }

command -v hyprctl >/dev/null && [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]] || { echo "needs a running Hyprland session" >&2; exit 2; }
command -v abyss >/dev/null || { echo "abyss is not installed" >&2; exit 2; }
if [[ $(windows) != 0 || $(abyss_procs) != 0 ]]; then echo "a screensaver is already running; stop it first (abyss stop)" >&2; exit 2; fi

monitors=$(hyprctl monitors -j | jq length)
restore_stay=0
if [[ -e $STAY ]]; then
  mv "$STAY" "$STAY.e2e-saved" && restore_stay=1
  trap '[[ $restore_stay == 1 ]] && mv "$STAY.e2e-saved" "$STAY"; abyss stop >/dev/null 2>&1' EXIT
else
  trap 'abyss stop >/dev/null 2>&1' EXIT
fi

echo "monitors: $monitors"
echo "1. launch"
if ((real)); then
  echo "   waiting for the idle timer (150 s) — hands off the keyboard and mouse"
  wait_until 220 '[[ $(windows) -gt 0 ]]' || bad "the idle timer never launched the screensaver"
else
  ipc launch
  wait_until 20 '[[ $(windows) -ge '"$monitors"' ]]' || true
fi
wait_until 5 '[[ $(abyss_procs) -ge '"$monitors"' ]]' || true
check "one window per monitor ($monitors)" '[[ $(windows) == '"$monitors"' ]]'
check "one abyss process per monitor" '[[ $(abyss_procs) == '"$monitors"' ]]'
check "primary is serving the socket" '[[ -S $SOCK ]]'
check "the other monitors are subscribed" '[[ $(ss -xH state connected 2>/dev/null | grep -c "abyss.sock") -ge '"$((monitors - 1))"' ]]'

echo "2. run for 15 s"
sleep 15
mkdir -p "${TMPDIR:-/tmp}"
shot=${TMPDIR:-/tmp}/abyss-e2e.png
grim "$shot" 2>/dev/null && ok "screenshot: $shot" || bad "grim failed"
check "progress is being saved (stats.json updated)" '[[ -n $(find "$STATE/stats.json" -mmin -1 2>/dev/null) ]]'
check "pointer hidden while the screensaver runs" '[[ $(hyprctl getoption cursor:invisible -j | jq -r ".int // .bool") =~ ^(1|true)$ ]]'

echo "3. wake"
if ((real)); then
  echo "   move the mouse now"
  wait_until 60 '[[ $(windows) == 0 ]]' || true
else
  ipc wake
fi
wait_until 15 '[[ $(windows) == 0 && $(abyss_procs) == 0 ]]' || true
check "all windows closed" '[[ $(windows) == 0 ]]'
check "all abyss processes gone" '[[ $(abyss_procs) == 0 ]]'
check "socket removed" '[[ ! -e $SOCK ]]'
check "pointer visible again" '[[ ! $(hyprctl getoption cursor:invisible -j | jq -r ".int // .bool") =~ ^(1|true)$ ]]'
check "session summary written" '[[ -s $STATE/last_session.json ]]'
check "no crash logged" '[[ ! -s $STATE/crash.log ]]'

((clean)) && rm -rf "$STATE" && echo "(progress from this run deleted)"
echo
if ((fails)); then echo "FAILED: $fails check(s)"; exit 1; fi
echo "PASSED"
