#!/bin/bash
set -euo pipefail

script_path=$(readlink -f "${BASH_SOURCE[0]}")
root=$(cd "$(dirname "$script_path")/.." && pwd)

stop_stub_session() {
  printf 'stopped\n' > "$STUB_ROOT/session"
  if [[ -s $STUB_ROOT/socket.pid ]]; then
    kill "$(<"$STUB_ROOT/socket.pid")" 2>/dev/null || true
  fi
  rm -f "$XDG_RUNTIME_DIR/abyss.sock"
  mkdir -p "$XDG_STATE_HOME/abyss"
  printf '{}\n' > "$XDG_STATE_HOME/abyss/last_session.json"
}

run_stub() {
  case ${0##*/} in
    abyss)
      [[ ${1:-} == stop ]] && stop_stub_session
      ;;
    grim)
      printf 'fake screenshot\n' > "$1"
      printf '%s\n%s\n%s\n' \
        "$1" \
        "$(stat -c %a "$(dirname "$1")")" \
        "$(stat -c %a "$1")" > "$STUB_ROOT/grim-record"
      ;;
    hyprctl)
      case ${1:-} in
        monitors) printf '[{"name":"TEST-1"}]\n' ;;
        clients)
          if grep -qx launched "$STUB_ROOT/session"; then
            printf '[{"class":"org.omarchy.screensaver"}]\n'
          else
            printf '[]\n'
          fi
          ;;
        getoption)
          if grep -qx launched "$STUB_ROOT/session"; then
            printf '{"int":1}\n'
          else
            printf '{"int":0}\n'
          fi
          ;;
      esac
      ;;
    pgrep)
      if grep -qx launched "$STUB_ROOT/session"; then
        printf '1\n'
      else
        printf '0\n'
      fi
      ;;
    quickshell)
      action=${!#}
      if [[ $action == launch ]]; then
        printf 'launched\n' > "$STUB_ROOT/session"
        touch "$XDG_STATE_HOME/abyss/stats.json"
        python3 -c 'import socket, sys, time
s = socket.socket(socket.AF_UNIX)
s.bind(sys.argv[1])
s.listen()
time.sleep(30)' "$XDG_RUNTIME_DIR/abyss.sock" &
        printf '%s\n' "$!" > "$STUB_ROOT/socket.pid"
        for _ in {1..20}; do
          [[ -S $XDG_RUNTIME_DIR/abyss.sock ]] && break
          /usr/bin/sleep 0.05
        done
      elif [[ $action == wake ]]; then
        stop_stub_session
      fi
      ;;
    sleep) ;;
    *)
      printf 'unexpected stub invocation: %s\n' "${0##*/}" >&2
      exit 1
      ;;
  esac
}

if [[ ${ABYSS_E2E_STUB:-0} == 1 ]]; then
  run_stub "$@"
  exit
fi

test_root=$(mktemp -d)
trap 'rm -rf "$test_root"' EXIT

export STUB_ROOT=$test_root/stub
export HOME=$test_root/home
export TMPDIR=$test_root/tmp
export XDG_RUNTIME_DIR=$test_root/runtime
export XDG_STATE_HOME=$test_root/state
export ABYSS_E2E_STUB=1
export HYPRLAND_INSTANCE_SIGNATURE=abyss-e2e-test

mkdir -p "$STUB_ROOT/bin" "$HOME" "$TMPDIR" "$XDG_RUNTIME_DIR" "$XDG_STATE_HOME/abyss"
chmod 755 "$TMPDIR"
printf 'stopped\n' > "$STUB_ROOT/session"

for command in abyss grim hyprctl pgrep quickshell sleep; do
  ln -s "$script_path" "$STUB_ROOT/bin/$command"
done

PATH="$STUB_ROOT/bin:$PATH" bash "$root/scripts/e2e-idle.sh" > "$test_root/output"

shot=$(sed -n '1p' "$STUB_ROOT/grim-record")
capture_dir=$(dirname "$shot")
dir_mode=$(sed -n '2p' "$STUB_ROOT/grim-record")
file_mode=$(sed -n '3p' "$STUB_ROOT/grim-record")

[[ $capture_dir == "$TMPDIR"/abyss-e2e.* ]]
[[ $dir_mode == 700 ]]
[[ $file_mode == 600 ]]
[[ ! -e $shot ]]
[[ ! -e $capture_dir ]]

printf 'e2e-idle capture is private and cleaned up\n'
