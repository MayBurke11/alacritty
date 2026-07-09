#!/bin/bash
# e2e-session-test.sh — session save/restore + menu tests
set -euo pipefail

ALACRITTY="${ALACRITTY:-./target/debug/alacritty}"
TEST_CONFIG="${TEST_CONFIG:-/tmp/menu-e2e-test.toml}"
PASS=0; FAIL=0

red()   { echo -e "\033[31m$*\033[0m"; }
green() { echo -e "\033[32m$*\033[0m"; }
assert_ok() {
  local desc="$1" ok="$2"
  case "$ok" in
    ok*) green "  PASS: $desc ($ok)"; PASS=$((PASS + 1)) ;;
    *)   red "  FAIL: $desc ($ok)"; FAIL=$((FAIL + 1)) ;;
  esac
}

cleanup() {
  jobs -p | xargs -r kill 2>/dev/null || true
  sleep 0.5
  wait 2>/dev/null || true
}
trap cleanup EXIT

echo "=== E2E Session Save/Restore & Menu Test ==="
echo ""

rm -f /tmp/Alacritty-*.log /run/user/1000/Alacritty-:0-*.sock 2>/dev/null || true

# ====== Launch ======
echo "[1] Launching..."
"$ALACRITTY" --config-file "$TEST_CONFIG" &
ALACRITTY_PID=$!

echo "[2] Waiting for socket (PID=$ALACRITTY_PID)..."
SOCKET="$XDG_RUNTIME_DIR/Alacritty-:0-$ALACRITTY_PID.sock"
for i in $(seq 1 60); do
  [ -S "$SOCKET" ] && break
  sleep 0.25
done
if [ ! -S "$SOCKET" ]; then
  red "ERROR: socket not found"; exit 1
fi
green "  Socket ready"
SOCK="-s $SOCKET"

# ====== Test 1: Initial state ======
echo ""
echo "=== Test 1: Initial state ==="
OUT=$("$ALACRITTY" msg $SOCK list-tabs 2>&1)
COUNT=$(echo "$OUT" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
assert_ok "at least 1 tab on start" "$([ "$COUNT" -ge 1 ] && echo ok || echo "got $COUNT")"

# ====== Test 2: Create tabs ======
echo ""
echo "=== Test 2: Create tabs ==="
INITIAL=$COUNT
for i in 1 2; do
  "$ALACRITTY" msg $SOCK create-tab 2>/dev/null || true
  sleep 1
done
OUT=$("$ALACRITTY" msg $SOCK list-tabs 2>&1)
COUNT=$(echo "$OUT" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
assert_ok "2 new tabs created" "$([ "$COUNT" -eq $((INITIAL + 2)) ] && echo ok || echo "$INITIAL→$COUNT")"

# ====== Test 3: Save via IPC ======
echo ""
echo "=== Test 3: Save via IPC ==="
JSON=$("$ALACRITTY" msg $SOCK save-tabs 2>&1)
VALID=$(echo "$JSON" | python3 -c "import sys,json; s=json.load(sys.stdin); assert 'tabs' in s; assert 'active_tab' in s; print('ok')" 2>/dev/null || echo "invalid")
assert_ok "save-tabs returns valid JSON" "$VALID"

mkdir -p /tmp/alacritty-e2e-sessions
echo "$JSON" > /tmp/alacritty-e2e-sessions/saved.json
assert_ok "session saved to file" "$([ -s /tmp/alacritty-e2e-sessions/saved.json ] && echo ok || echo 'missing')"

# ====== Test 4: Menu SAVE action ======
echo ""
echo "=== Test 4: Menu SAVE action ==="
WID=$(xdotool search --pid "$ALACRITTY_PID" --class Alacritty 2>/dev/null | head -1 || echo "")
if [ -n "$WID" ]; then
  xdotool windowactivate "$WID" 2>/dev/null || true
  sleep 0.3
  # Ctrl+G, s (SESS), s (SAVE)
  xdotool key --window "$WID" ctrl+g; sleep 0.3
  xdotool key --window "$WID" s; sleep 0.3
  xdotool key --window "$WID" s; sleep 1.5
  # Check sessions dir
  SDIR="$HOME/.config/alacritty/sessions"
  SFILES=$(ls "$SDIR"/*.json 2>/dev/null | wc -l)
  assert_ok "menu SAVE created session file" "$([ "$SFILES" -gt 0 ] && echo "ok ($SFILES files)" || echo "no files in $SDIR")"
else
  assert_ok "menu SAVE (window found)" "no window"
fi

# ====== Test 5: Menu TAB KILL ======
echo ""
echo "=== Test 5: Menu TAB KILL ==="
if [ -n "$WID" ]; then
  BEFORE=$("$ALACRITTY" msg $SOCK list-tabs 2>&1 | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
  xdotool windowactivate "$WID" 2>/dev/null || true; sleep 0.3
  # Ctrl+G, t (TAB), k (KILL)
  xdotool key --window "$WID" ctrl+g; sleep 0.3
  xdotool key --window "$WID" t; sleep 0.3
  xdotool key --window "$WID" k; sleep 1.5
  AFTER=$("$ALACRITTY" msg $SOCK list-tabs 2>&1 | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
  assert_ok "menu KILL closed a tab" "$([ "$AFTER" -lt "$BEFORE" ] && echo "ok ($BEFORE→$AFTER)" || echo "$BEFORE→$AFTER")"
fi

# ====== Test 6: Restore ======
echo ""
echo "=== Test 6: Restore via --restore ==="
cleanup  # kill current instance

"$ALACRITTY" --config-file "$TEST_CONFIG" --restore /tmp/alacritty-e2e-sessions/saved.json &
ALACRITTY_PID=$!
sleep 3

if kill -0 "$ALACRITTY_PID" 2>/dev/null; then
  SOCKET="$XDG_RUNTIME_DIR/Alacritty-:0-$ALACRITTY_PID.sock"
  for i in $(seq 1 30); do
    [ -S "$SOCKET" ] && break
    sleep 0.25
  done
  if [ -S "$SOCKET" ]; then
    RESTORED=$("$ALACRITTY" msg -s "$SOCKET" list-tabs 2>&1 | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
    assert_ok "restored tabs" "$([ "$RESTORED" -ge 2 ] && echo "ok ($RESTORED tabs)" || echo "got $RESTORED")"
  else
    assert_ok "restore socket" "not found"
  fi
else
  assert_ok "restore process started" "process died"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -gt 0 ] && exit 1 || exit 0
