#!/usr/bin/env bash
# Session toggle for Logi Options+ (does NOT uninstall LaunchAgent plists).
# Usage: logi-mode.sh <disable|enable|status|enter-drums|exit-drums>
set -euo pipefail

UID_NUM="$(id -u)"
GUI_DOMAIN="gui/${UID_NUM}"

# Common Options+ agent locations (system-wide is typical on recent installs).
CANDIDATE_PLISTS=(
  "/Library/LaunchAgents/com.logi.optionsplus.plist"
  "${HOME}/Library/LaunchAgents/com.logi.optionsplus.plist"
)

find_plist() {
  for p in "${CANDIDATE_PLISTS[@]}"; do
    if [[ -f "$p" ]]; then
      echo "$p"
      return 0
    fi
  done
  return 1
}

kill_logi_procs() {
  local names=(
    logioptionsplus
    logioptionsplus_agent
    logioptionsplus_updater
    LogiPluginService
    "Logi Options+"
    logioptionsplus_app
  )
  local n
  for n in "${names[@]}"; do
    killall "$n" 2>/dev/null || true
  done
  # Catch leftover helpers matching logioptions / LogiPlugin
  pkill -f -i 'logioptionsplus' 2>/dev/null || true
  pkill -f -i 'LogiPluginService' 2>/dev/null || true
}

disable_logi() {
  local plist
  if plist="$(find_plist)"; then
    launchctl bootout "${GUI_DOMAIN}" "$plist" 2>/dev/null || true
  else
    echo "warn: Options+ LaunchAgent plist not found; killing processes only" >&2
  fi
  kill_logi_procs
  sleep 0.4
  kill_logi_procs
  echo "Logi Options+ disabled for this session"
}

enable_logi() {
  local plist
  if plist="$(find_plist)"; then
    launchctl bootstrap "${GUI_DOMAIN}" "$plist" 2>/dev/null \
      || launchctl load "$plist" 2>/dev/null \
      || true
    echo "Logi Options+ LaunchAgent bootstrapped: $plist"
  else
    echo "error: could not find com.logi.optionsplus.plist" >&2
    # Best-effort: open the app
    open -a "logioptionsplus" 2>/dev/null \
      || open -a "Logi Options+" 2>/dev/null \
      || true
  fi
}

status_logi() {
  echo "=== launchctl (logi) ==="
  launchctl list 2>/dev/null | grep -i logi || echo "(none)"
  echo "=== processes ==="
  if command -v pgrep >/dev/null 2>&1; then
    pgrep -lf -i 'logioptions|LogiPlugin' 2>/dev/null || echo "(none)"
  else
    echo "(pgrep unavailable)"
  fi
  if plist="$(find_plist)"; then
    echo "plist: ${plist}"
  else
    echo "plist: (not found)"
  fi
}

cmd="${1:-status}"
case "$cmd" in
  disable|off)
    disable_logi
    ;;
  enable|on)
    enable_logi
    ;;
  enter-drums)
    disable_logi
    # Options+ helpers sometimes respawn once; second pass after a beat.
    sleep 0.6
    kill_logi_procs
    sleep 0.3
    kill_logi_procs
    echo "Logi Options+ parked for drums mode"
    ;;
  exit-drums)
    enable_logi
    ;;
  status)
    status_logi
    ;;
  *)
    echo "Usage: $0 {disable|enable|status|enter-drums|exit-drums}" >&2
    exit 1
    ;;
esac
