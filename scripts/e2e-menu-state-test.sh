#!/bin/bash
# e2e-menu-state-test.sh — verify menu state transitions via debug logs
# Simulates key presses (xdotool) and checks [menu-sync] log entries.
set -euo pipefail

ALACRITTY="${ALACRITTY:-./target/debug/alacritty}"
TEST_CONFIG="${TEST_CONFIG:-/tmp/menu-e2e-test.toml}"
LOG=""
PASS=0; FAIL=0

red()   { echo -e "\033[31m$*\033[0m"; }
green() { echo -e "\033[32m$*\033[0m"; }

assert_contains() {
  local desc="$1" haystack="$2" needle="$3"
  if echo "$haystack" | grep -qF "$needle"; then
    green "  PASS: $desc"; PASS=$((PASS + 1))
  else
    red "  FAIL: $desc (expected '$needle' not found)"; FAIL=$((FAIL + 1))
  fi
}

assert_not_contains() {
  local desc="$1" haystack="$2" needle="$3"
  if echo "$haystack" | grep -qF "$needle"; then
    red "  FAIL: $desc (unexpected '$needle' found)"; FAIL=$((FAIL + 1))
  else
    green "  PASS: $desc"; PASS=$((PASS + 1))
  fi
}

# Get AFTER labels from most recent menu-sync log entry
last_sync_labels() {
  grep '\[menu-sync\].*AFTER' "$LOG" 2>/dev/null | tail -1 | sed 's/.*labels:\(\[.*\]\).*/\1/' | tr -d '[]' || true
}

all_sync_labels() {
  grep '\[menu-sync\].*AFTER' "$LOG" 2>/dev/null | while read line; do
    echo "$line" | sed 's/.*labels:\(\[.*\]\).*/\1/'
  done || true
}

cleanup() {
  [ -n "${ALACRITTY_PID:-}" ] && kill "$ALACRITTY_PID" 2>/dev/null || true
  sleep 0.5
  wait 2>/dev/null || true
}
trap cleanup EXIT

echo "=== E2E Menu State Machine Test ==="
echo ""

rm -f /run/user/1000/Alacritty-:0-*.sock 2>/dev/null || true

# ====== Launch ======
echo "[0] Launching alacritty..."
"$ALACRITTY" --config-file "$TEST_CONFIG" -vv &
ALACRITTY_PID=$!

# Wait for window
echo "    waiting for window..."
WID=""
for i in $(seq 1 60); do
  WID=$(xdotool search --pid "$ALACRITTY_PID" --class Alacritty 2>/dev/null | head -1 || true)
  [ -n "$WID" ] && break
  sleep 0.25
done
[ -z "$WID" ] && { red "ERROR: window not found"; exit 1; }

# Find log file
sleep 2
LOG=$(ls -t /tmp/Alacritty-*.log 2>/dev/null | head -1)
[ -z "$LOG" ] && { red "ERROR: log file not found"; exit 1; }
green "  Window: $WID, Log: $LOG"

# Give window time to fully initialise
sleep 1

sleep 0.3

# ====== Test 1: Initial state (no menu-sync entries = locked by default) ======
echo ""
echo "=== Test 1: Initial state ==="
L=$(last_sync_labels)
# Initial state is locked; no menu ops have fired yet so log may be empty
if [ -z "$L" ]; then
  green "  PASS: initial state (no menu ops yet, locked by default)"; PASS=$((PASS + 1))
else
  assert_contains "initial has LOCKED" "$L" "LOCKED"
fi

# ====== Test 2: Ctrl+G unlocks ======
echo ""
echo "=== Test 2: Unlock ==="
xdotool key --window "$WID" ctrl+g
sleep 0.5
L=$(last_sync_labels)
assert_contains "after Ctrl+G: has ACTIVE" "$L" "ACTIVE"
assert_contains "after Ctrl+G: has TAB"   "$L" "TAB"
assert_contains "after Ctrl+G: has SESS"  "$L" "SESS"

# ====== Test 3: 't' → TAB submenu ======
echo ""
echo "=== Test 3: TAB submenu ==="
xdotool key --window "$WID" t
sleep 0.5
L=$(last_sync_labels)
assert_contains "TAB submenu: has BACK"   "$L" "BACK"
assert_contains "TAB submenu: has CREATE" "$L" "CREATE"
assert_contains "TAB submenu: has KILL"   "$L" "KILL"

# ====== Test 4: Escape back to top ======
echo ""
echo "=== Test 4: Escape back ==="
xdotool key --window "$WID" Escape
sleep 0.5
L=$(last_sync_labels)
assert_contains "after Escape: ACTIVE" "$L" "ACTIVE"
assert_contains "after Escape: TAB"    "$L" "TAB"

# ====== Test 5: Ctrl+G locks ======
echo ""
echo "=== Test 5: Lock ==="
xdotool key --window "$WID" ctrl+g
sleep 0.5
L=$(last_sync_labels)
assert_contains "lock has LOCKED" "$L" "LOCKED"
assert_not_contains "lock no TAB"   "$L" "TAB"
assert_not_contains "lock no SESS"  "$L" "SESS"

# ====== Test 6: Unlock → must show top-level ======
echo ""
echo "=== Test 6: Unlock again → top level ==="
xdotool key --window "$WID" ctrl+g
sleep 0.5
L=$(last_sync_labels)
assert_contains "unlock: ACTIVE"  "$L" "ACTIVE"
assert_not_contains "unlock: no BACK (not submenu)" "$L" "BACK"

# ====== Test 7: Navigate deep, lock, unlock → top level ======
echo ""
echo "=== Test 7: Deep nav → lock → unlock → top ==="
xdotool key --window "$WID" s  # SESS
sleep 0.3
xdotool key --window "$WID" l  # LOAD (enters list mode)
sleep 0.5
xdotool key --window "$WID" ctrl+g  # lock
sleep 0.3
xdotool key --window "$WID" ctrl+g  # unlock
sleep 0.5
L=$(last_sync_labels)
assert_contains "deep nav: ACTIVE" "$L" "ACTIVE"
assert_not_contains "deep nav: no BACK" "$L" "BACK"

# ====== Test 8: Create tab via menu ======
echo ""
echo "=== Test 8: Create tab ==="
xdotool key --window "$WID" t  # TAB
sleep 0.3
xdotool key --window "$WID" c  # CREATE
sleep 1.5
L=$(last_sync_labels)
assert_contains "after CREATE: LOCKED" "$L" "LOCKED"

# ====== Test 9: Kill tab via menu ======
echo ""
echo "=== Test 9: Kill tab ==="
xdotool key --window "$WID" ctrl+g
sleep 0.3
xdotool key --window "$WID" t
sleep 0.3
xdotool key --window "$WID" k
sleep 1.5
L=$(last_sync_labels)
assert_contains "after KILL: LOCKED" "$L" "LOCKED"

# ====== Cleanup ======
kill "$ALACRITTY_PID" 2>/dev/null || true
wait "$ALACRITTY_PID" 2>/dev/null || true

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -gt 0 ] && exit 1 || exit 0
