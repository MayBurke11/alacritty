#!/bin/bash
# e2e-pane-test.sh — verify pane operations via modal menu (menu-only, no Ctrl+Shift shortcuts)
set -euo pipefail

ALACRITTY="${ALACRITTY:-./target/debug/alacritty}"
TEST_CONFIG="${TEST_CONFIG:-/tmp/menu-e2e-test.toml}"
PASS=0; FAIL=0

red()   { echo -e "\033[31m$*\033[0m"; }
green() { echo -e "\033[32m$*\033[0m"; }

cleanup() {
  [ -n "${ALACRITTY_PID:-}" ] && kill "$ALACRITTY_PID" 2>/dev/null || true
  sleep 0.5; wait 2>/dev/null || true
}
trap cleanup EXIT

echo "=== E2E Pane Test (menu-only) ==="
echo ""

rm -f /run/user/1000/Alacritty-:0-*.sock 2>/dev/null || true
rm -f /tmp/Alacritty-*.log 2>/dev/null || true

# ====== Launch ======
echo "[0] Launching alacritty..."
"$ALACRITTY" --config-file "$TEST_CONFIG" -vv &
ALACRITTY_PID=$!

echo "    waiting for window..."
WID=""
for i in $(seq 1 60); do
  WID=$(xdotool search --pid "$ALACRITTY_PID" --class Alacritty 2>/dev/null | head -1 || true)
  [ -n "$WID" ] && break; sleep 0.25
done
[ -z "$WID" ] && { red "ERROR: window not found"; exit 1; }

sleep 2
LOG=$(ls -t /tmp/Alacritty-*.log 2>/dev/null | head -1)
[ -z "$LOG" ] && { red "ERROR: log not found"; exit 1; }
green "  Window: $WID, Log: $LOG"

# Helper: press key combo via xdotool
press() { xdotool key --window "$WID" $1; sleep 0.4; }

menu() {
  # Enter menu flow: Ctrl+G, then letter keys for each level
  # Usage: menu "p" "s" "r"  →  Ctrl+G, p, s, r
  press "ctrl+g"
  for key in "$@"; do
    press "$key"
  done
  sleep 0.5
}

assert_log() {
  local desc="$1" pattern="$2"
  if grep -q "$pattern" "$LOG" 2>/dev/null; then
    green "  PASS: $desc"; PASS=$((PASS + 1))
  else
    red "  FAIL: $desc (no '$pattern' in log)"; FAIL=$((FAIL + 1))
  fi
}

echo ""
echo "=== Test 1: Split right (menu: Ctrl+G, p, s, r) ==="
menu "p" "s" "r"
assert_log "split right via menu" '\[panes\] split'

echo ""
echo "=== Test 2: Split down (menu: Ctrl+G, p, s, d) ==="
menu "p" "s" "d"
assert_log "split down via menu" '\[panes\] split'

echo ""
echo "=== Test 3: Focus right (Alt+→ still works) ==="
press "alt+Right"
sleep 0.3
assert_log "focus right via Alt+→" '\[panes\] focus'

echo ""
echo "=== Test 4: Zoom toggle (Alt+F still works) ==="
press "alt+f"
sleep 0.3
assert_log "zoom on" '\[panes\] zoom'
press "alt+f"
sleep 0.3
assert_log "zoom off" '\[panes\] zoom'

echo ""
echo "=== Test 5: Resize mode (menu: Ctrl+G, p, r) ==="
menu "p" "r"
assert_log "resize mode on" 'resize mode: true'

echo ""
echo "=== Test 6: Kill pane (menu: Ctrl+G, p, k) ==="
menu "p" "k"
assert_log "kill via menu" '\[panes\] close'

echo ""
echo "=== Test 7: Resize off (menu: Ctrl+G, p, r) ==="
menu "p" "r"
assert_log "resize mode off" 'resize mode: false'

echo ""
echo "=== Test 8: Kill last split → single pane ==="
menu "p" "k"
assert_log "kill last split" '\[panes\] close'

echo ""
echo "=== Test 9: Full menu flow TAB → CREATE ==="
menu "t" "c"
sleep 1.5
assert_log "tab created via menu" '\[tabs\] created tab'

echo ""
echo "=== Test 10: Full menu flow TAB → KILL ==="
menu "t" "k"
sleep 1
assert_log "tab killed via menu" '\[tabs\] closed tab'

echo ""
echo "=== Full log ==="
grep '\[panes\]\|\[tabs\]\|resize mode' "$LOG" 2>/dev/null | tail -20

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -gt 0 ] && exit 1 || exit 0
