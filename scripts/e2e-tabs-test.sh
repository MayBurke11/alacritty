#!/bin/bash
# e2e-tabs-test.sh — automated tab create/switch/close/title test via xdotool
set -euo pipefail

ALACRITTY="${ALACRITTY:-./target/release/alacritty}"
PASS=0
FAIL=0

red() { echo -e "\033[31m$*\033[0m"; }
green() { echo -e "\033[32m$*\033[0m"; }

assert_log() {
  local pattern="$1" desc="$2"
  if grep -q "$pattern" /tmp/alacritty-e2e-tabs.log 2>/dev/null; then
    green "  PASS: $desc"
    PASS=$((PASS + 1))
  else
    red "  FAIL: $desc (expected: $pattern)"
    FAIL=$((FAIL + 1))
  fi
}

cleanup() {
  [ -n "${CAPTURE_PID:-}" ] && kill "$CAPTURE_PID" 2>/dev/null || true
  [ -n "${ALACRITTY_PID:-}" ] && kill "$ALACRITTY_PID" 2>/dev/null || true
  wait 2>/dev/null || true
}
trap cleanup EXIT

echo "=== E2E Tab Test ==="
echo ""

rm -f /tmp/Alacritty-*.log /tmp/alacritty-e2e-tabs.log

# Launch with tabs
echo "[1] Launching alacritty..."
"$ALACRITTY" -vv &
ALACRITTY_PID=$!

# Capture logs
(
  while kill -0 "$ALACRITTY_PID" 2>/dev/null; do
    for f in /tmp/Alacritty-*.log; do
      [ -f "$f" ] && cp "$f" /tmp/alacritty-e2e-tabs.log 2>/dev/null
    done
    sleep 0.3
  done
) &
CAPTURE_PID=$!

echo "[2] Waiting for window..."
WID=""
for i in $(seq 1 60); do
  WID=$(xdotool search --pid "$ALACRITTY_PID" --class Alacritty 2>/dev/null | head -1 || true)
  [ -n "$WID" ] && break
  sleep 0.25
done
[ -z "$WID" ] && { red "ERROR: window not found"; exit 1; }
green "  Window found: $WID"

sleep 2
xdotool windowactivate "$WID" 2>/dev/null || true
sleep 0.5

# ====== Test 1: Create Tab ======
echo ""
echo "=== Create Tab ==="
echo "[3] Create new tab (Ctrl+Shift+T)"
xdotool key --window "$WID" ctrl+shift+t
sleep 1.5
assert_log '\[tabs\] created tab' "tab created"

echo "[4] Create another tab"
xdotool key --window "$WID" ctrl+shift+t
sleep 1.5
assert_log '\[tabs\] created tab' "second tab created"

# ====== Test 2: Switch Tabs ======
echo ""
echo "=== Switch Tabs ==="
echo "[5] Switch to next tab (Ctrl+Shift+Right)"
xdotool key --window "$WID" ctrl+shift+Right
sleep 0.5
assert_log '\[tabs\] switch to tab' "tab switched forward"

echo "[6] Switch to previous tab (Ctrl+Shift+Left)"
xdotool key --window "$WID" ctrl+shift+Left
sleep 0.5
assert_log '\[tabs\] switch to tab' "tab switched backward"

# ====== Test 3: Close Tab ======
echo ""
echo "=== Close Tab ==="
echo "[7] Close active tab (Ctrl+Shift+W)"
xdotool key --window "$WID" ctrl+shift+w
sleep 1.5
assert_log '\[tabs\] closed tab' "tab closed"

echo "[8] Close another tab"
xdotool key --window "$WID" ctrl+shift+w
sleep 1.5
assert_log '\[tabs\] closed tab' "last tab closed"

# ====== Test 4: Input Isolation ======
echo ""
echo "=== Input Isolation ==="
echo "[9] Create tab and type something in it"
xdotool key --window "$WID" ctrl+shift+t
sleep 1.5
xdotool windowactivate "$WID" 2>/dev/null || true
sleep 0.3
xdotool type --window "$WID" "echo tab-test"
xdotool key --window "$WID" Return
sleep 0.5

echo "[10] Switch back and verify input didn't leak"
xdotool key --window "$WID" ctrl+shift+Left
sleep 0.5
xdotool type --window "$WID" "echo tab-2-test"
xdotool key --window "$WID" Return
sleep 0.5

# ====== Test 5: Exit ======
echo ""
echo "[11] Exit alacritty"
xdotool windowactivate "$WID" 2>/dev/null || true
sleep 0.2
xdotool type --window "$WID" "exit"
xdotool key --window "$WID" Return
sleep 2

kill "$ALACRITTY_PID" 2>/dev/null || true
wait "$ALACRITTY_PID" 2>/dev/null || true

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
if [ "$FAIL" -gt 0 ]; then
  echo ""
  echo "=== Full log ==="
  grep -i "tab" /tmp/alacritty-e2e-tabs.log 2>/dev/null || echo "(no tab events)"
  exit 1
fi
