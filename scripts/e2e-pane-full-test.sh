#!/bin/bash
# e2e-pane-full-test.sh — automated: split × 4, resize × 4, focus × 4, close × 2
# Verifies via [panes] debug logs that every operation succeeds.
set -euo pipefail

ALACRITTY="${ALACRITTY:-./target/debug/alacritty}"
TEST_CONFIG="${TEST_CONFIG:-/tmp/menu-e2e-test.toml}"
PASS=0; FAIL=0

red()   { echo -e "\033[31m$*\033[0m"; }
green() { echo -e "\033[32m$*\033[0m"; }

cleanup() { [ -n "${PID:-}" ] && kill "$PID" 2>/dev/null; sleep 0.5; wait 2>/dev/null; }
trap cleanup EXIT

assert_log() {
  if grep -q "$2" "$LOG" 2>/dev/null; then green "  PASS: $1"; PASS=$((PASS+1))
  else red "  FAIL: $1 (no '$2')"; FAIL=$((FAIL+1)); fi
}

echo "=== E2E Pane Full Test ==="
rm -f /run/user/1000/Alacritty-:0-*.sock /tmp/Alacritty-*.log 2>/dev/null

# ===== Launch =====
"$ALACRITTY" --config-file "$TEST_CONFIG" -v &
PID=$!
for i in $(seq 1 60); do WID=$(xdotool search --pid $PID --class Alacritty 2>/dev/null|head -1); [ -n "$WID" ] && break; sleep 0.25; done
[ -z "$WID" ] && { red "no window"; exit 1; }
sleep 2; LOG=$(ls -t /tmp/Alacritty-*.log 2>/dev/null|head -1)
green "WID=$WID PID=$PID"

menu() { xdotool key --window "$WID" ctrl+g; sleep 0.3; for k in "$@"; do xdotool key --window "$WID" "$k"; sleep 0.3; done; sleep 0.5; }

# ===== Test 1: Split all 4 directions =====
echo "=== Split ==="
menu "p" "s" "r"; assert_log "split right" '\[panes\] split Horizontal'
menu "p" "s" "d"; assert_log "split down" '\[panes\] split Vertical'
menu "p" "s" "d"; assert_log "split down #2" '\[panes\] split Vertical'
xdotool key --window "$WID" alt+Left; sleep 0.3
menu "p" "s" "d"; assert_log "split down #3" '\[panes\] split Vertical'

# ===== Test 2: Resize all 4 directions =====
echo "=== Resize ==="
menu "p" "r"; sleep 0.3  # enter resize mode
xdotool key --window "$WID" alt+Right; sleep 0.5
xdotool key --window "$WID" alt+Left; sleep 0.5
xdotool key --window "$WID" alt+Down; sleep 0.5
xdotool key --window "$WID" alt+Up; sleep 0.5
xdotool key --window "$WID" Escape; sleep 0.3  # exit resize
assert_log "resize mode on" 'resize mode: true'
assert_log "resize mode off" 'resize mode: false'

# ===== Test 3: Focus all 4 directions =====
echo "=== Focus ==="
xdotool key --window "$WID" alt+Right; sleep 0.3
assert_log "focus right" '\[panes\] focus'
xdotool key --window "$WID" alt+Down; sleep 0.3
assert_log "focus down" '\[panes\] focus'
xdotool key --window "$WID" alt+Up; sleep 0.3
assert_log "focus up" '\[panes\] focus'
xdotool key --window "$WID" alt+Left; sleep 0.3
assert_log "focus left" '\[panes\] focus'

# ===== Test 4: Close panes until 1 remains =====
echo "=== Close ==="
menu "p" "k"; sleep 0.8; assert_log "close pane" '\[panes\] close'
menu "p" "k"; sleep 0.8; assert_log "close pane #2" '\[panes\] close'
menu "p" "k"; sleep 0.8; assert_log "close pane #3" '\[panes\] close'

# ===== Test 5: Zoom =====
echo "=== Zoom ==="
menu "p" "f"; sleep 0.5; assert_log "zoom on" '\[panes\] zoom'
menu "p" "f"; sleep 0.5; assert_log "zoom off" '\[panes\] zoom'

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -gt 0 ] && exit 1 || exit 0
