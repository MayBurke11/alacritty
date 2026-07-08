#!/bin/bash
# e2e-tabs-ipc-test.sh — automated tab IPC commands test
set -euo pipefail

ALACRITTY="${ALACRITTY:-./target/release/alacritty}"
PASS=0
FAIL=0

red() { echo -e "\033[31m$*\033[0m"; }
green() { echo -e "\033[32m$*\033[0m"; }

assert_ok() {
  local desc="$1"
  if [ "$2" = "ok" ]; then
    green "  PASS: $desc"
    PASS=$((PASS + 1))
  else
    red "  FAIL: $desc ($2)"
    FAIL=$((FAIL + 1))
  fi
}

cleanup() {
  [ -n "${CAPTURE_PID:-}" ] && kill "$CAPTURE_PID" 2>/dev/null || true
  [ -n "${ALACRITTY_PID:-}" ] && kill "$ALACRITTY_PID" 2>/dev/null || true
  wait 2>/dev/null || true
}
trap cleanup EXIT

echo "=== E2E Tab IPC Test ==="
echo ""

rm -f /tmp/Alacritty-*.log /tmp/alacritty-e2e-tabs-ipc.log

# Launch alacritty
echo "[1] Launching alacritty..."
"$ALACRITTY" -vv &
ALACRITTY_PID=$!

# Capture logs
(
  while kill -0 "$ALACRITTY_PID" 2>/dev/null; do
    for f in /tmp/Alacritty-*.log; do
      [ -f "$f" ] && cp "$f" /tmp/alacritty-e2e-tabs-ipc.log 2>/dev/null
    done
    sleep 0.3
  done
) &
CAPTURE_PID=$!

echo "[2] Waiting for IPC socket..."
SOCKET=""
for i in $(seq 1 40); do
  # Socket is directly in $XDG_RUNTIME_DIR, using prefix "Alacritty-<display>-"
  for f in $XDG_RUNTIME_DIR/Alacritty-*.sock /tmp/Alacritty-*.sock; do
    [ -S "$f" ] && SOCKET="$f" && break 2
  done
  sleep 0.25
done
[ -z "$SOCKET" ] && { red "ERROR: socket not found in $XDG_RUNTIME_DIR"; exit 1; }
green "  Socket found: $SOCKET"
SOCKET_ARG="-s $SOCKET"

# ====== Test 1: list-tabs ======
echo ""
echo "=== list-tabs ==="
echo "[3] List tabs (expect 1 tab)"
OUT=$("$ALACRITTY" msg $SOCKET_ARG list-tabs 2>&1) || OUT="error:$?"
RESULT=$(echo "$OUT" | python3 -c "import sys,json; tabs=json.load(sys.stdin); print('ok' if len(tabs)==1 else f'got {len(tabs)} tabs')" 2>/dev/null || echo "parse_err")
assert_ok "list-tabs returns 1 tab" "$RESULT"

# ====== Test 2: create-tab -e ======
echo ""
echo "=== create-tab ==="
echo "[4] Create tab with htop"
"$ALACRITTY" msg $SOCKET_ARG create-tab -e htop 2>&1 || true
sleep 1.5

echo "[5] Verify tabs count = 2"
OUT=$("$ALACRITTY" msg $SOCKET_ARG list-tabs 2>&1)
RESULT=$(echo "$OUT" | python3 -c "import sys,json; tabs=json.load(sys.stdin); print('ok' if len(tabs)==2 else f'got {len(tabs)} tabs')" 2>/dev/null || echo "parse_err")
assert_ok "list-tabs returns 2 tabs" "$RESULT"

# ====== Test 3: create-tab --no-switch ======
echo ""
echo "=== create-tab --no-switch ==="
echo "[6] Create background tab (--no-switch)"
"$ALACRITTY" msg $SOCKET_ARG create-tab --no-switch -e bash 2>&1 || true
sleep 1.5

echo "[7] Verify tabs count = 3"
OUT=$("$ALACRITTY" msg $SOCKET_ARG list-tabs 2>&1)
RESULT=$(echo "$OUT" | python3 -c "import sys,json; tabs=json.load(sys.stdin); print('ok' if len(tabs)==3 else f'got {len(tabs)} tabs')" 2>/dev/null || echo "parse_err")
assert_ok "list-tabs returns 3 tabs" "$RESULT"

echo "[8] Verify active tab is still #2 (no-switch worked — stayed on htop)"
RESULT=$(echo "$OUT" | python3 -c "import sys,json; tabs=json.load(sys.stdin); active=[t['index'] for t in tabs if t['active']]; print('ok' if active and active[0]==2 else f'active={active}')" 2>/dev/null || echo "parse_err")
assert_ok "active tab stayed on #2 after --no-switch" "$RESULT"

# ====== Test 4: select-tab ======
echo ""
echo "=== select-tab ==="
echo "[9] Select tab 2"
"$ALACRITTY" msg $SOCKET_ARG select-tab 2 2>&1 || true
sleep 0.5

echo "[10] Verify active tab is #2"
OUT=$("$ALACRITTY" msg $SOCKET_ARG list-tabs 2>&1)
RESULT=$(echo "$OUT" | python3 -c "import sys,json; tabs=json.load(sys.stdin); active=[t['index'] for t in tabs if t['active']]; print('ok' if active and active[0]==2 else f'active={active}')" 2>/dev/null || echo "parse_err")
assert_ok "active tab is #2 after select-tab" "$RESULT"

# ====== Test 5: close-tab ======
echo ""
echo "=== close-tab ==="
echo "[11] Close tab 3"
"$ALACRITTY" msg $SOCKET_ARG close-tab 3 2>&1 || true
sleep 1.5

echo "[12] Verify tabs count = 2"
OUT=$("$ALACRITTY" msg $SOCKET_ARG list-tabs 2>&1)
RESULT=$(echo "$OUT" | python3 -c "import sys,json; tabs=json.load(sys.stdin); print('ok' if len(tabs)==2 else f'got {len(tabs)} tabs')" 2>/dev/null || echo "parse_err")
assert_ok "list-tabs returns 2 tabs after close-tab" "$RESULT"

# ====== Test 6: tab creation with command verified via log ======
echo ""
echo "=== verify create-tab -e htop in log ==="
grep -q '\[tabs\] created tab' /tmp/alacritty-e2e-tabs-ipc.log 2>/dev/null && RESULT="ok" || RESULT="no log entries"
assert_ok "tab creation logged" "$RESULT"

# ====== Cleanup ======
echo ""
echo "[13] Exit alacritty"
kill "$ALACRITTY_PID" 2>/dev/null || true
wait "$ALACRITTY_PID" 2>/dev/null || true

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
if [ "$FAIL" -gt 0 ]; then
  echo ""
  echo "=== Full log ==="
  grep -i "\[tabs\]" /tmp/alacritty-e2e-tabs-ipc.log 2>/dev/null || echo "(no tab events)"
  exit 1
fi
