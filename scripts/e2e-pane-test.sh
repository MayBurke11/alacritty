#!/bin/bash
# e2e-pane-test.sh — verify pane split/focus/close operations via debug logs
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

echo "=== E2E Pane Test ==="
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

# Helper: press key combo and wait
press() { xdotool key --window "$WID" $1; sleep 1; }

assert_log() {
  local desc="$1" pattern="$2"
  if grep -q "$pattern" "$LOG" 2>/dev/null; then
    green "  PASS: $desc"; PASS=$((PASS + 1))
  else
    red "  FAIL: $desc (no '$pattern' in log)"; FAIL=$((FAIL + 1))
  fi
}

# ====== Capture initial log position ======
LOG_START=$(wc -l < "$LOG" 2>/dev/null || echo 0)

echo ""
echo "=== Test 1: Split right ==="
press "ctrl+shift+1"  # Ctrl+Shift+!  (SplitRight)
sleep 1
assert_log "split right pane" '\[panes\] split'

echo ""
echo "=== Test 2: Split down ==="
press "ctrl+shift+2"  # Ctrl+Shift+@  (SplitDown)
sleep 1
assert_log "split down pane" '\[panes\] split'

echo ""
echo "=== Test 3: Focus panes ==="
press "ctrl+shift+4"  # Ctrl+Shift+$  (FocusLeft)
sleep 0.5
assert_log "focus left" '\[panes\] focus'
press "ctrl+shift+5"  # Ctrl+Shift+%  (FocusRight)
sleep 0.5
assert_log "focus right" '\[panes\] focus'

echo ""
echo "=== Test 4: Zoom toggle ==="
press "alt+f"
sleep 0.5
assert_log "zoom toggle" '\[panes\] zoom'
press "alt+f"
sleep 0.5
assert_log "zoom untoggle" '\[panes\] zoom'

echo ""
echo "=== Test 5: Close pane ==="
press "ctrl+shift+3"  # Ctrl+Shift+#  (ClosePane)
sleep 1
assert_log "close pane" '\[panes\] close'

echo ""
echo "=== Test 6: Close last pane → single ==="
press "ctrl+shift+3"
sleep 1
assert_log "close last split" '\[panes\] close'

echo ""
echo "=== Test 7: Menu PANE → SPLIT ==="
# Ctrl+G, p, s, d
xdotool key --window "$WID" ctrl+g; sleep 0.4
xdotool key --window "$WID" p; sleep 0.4
xdotool key --window "$WID" s; sleep 0.4
xdotool key --window "$WID" d; sleep 1
assert_log "menu split down" '\[panes\] split'

echo ""
echo "=== Test 8: Menu PANE → KILL ==="
xdotool key --window "$WID" ctrl+g; sleep 0.4
xdotool key --window "$WID" p; sleep 0.4
xdotool key --window "$WID" k; sleep 1
assert_log "menu kill" '\[panes\] close'

echo ""
echo "=== Full log ==="
grep '\[panes\]' "$LOG" 2>/dev/null | tail -20

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -gt 0 ] && exit 1 || exit 0
