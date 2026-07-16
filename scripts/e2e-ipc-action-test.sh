#!/bin/bash
# e2e-ipc-action-test.sh — test unified Action JSON via raw socket
set -euo pipefail

ALACRITTY="${ALACRITTY:-./target/release/alacritty}"
PASS=0
FAIL=0

red()   { echo -e "\033[31m$*\033[0m"; }
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

send_action() {
  python3 -c "
import socket, json, sys
data = sys.argv[1]
s = socket.socket(socket.AF_UNIX)
s.connect('$SOCKET')
s.sendall(data.encode() + b'\n')
s.close()
" "$1" 2>/dev/null
}

list_tabs_count() {
  local out
  out=$("$ALACRITTY" msg $SOCKET_ARG list-tabs 2>&1)
  echo "$out" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0"
}

cleanup() {
  [ -n "${CAPTURE_PID:-}" ] && kill "$CAPTURE_PID" 2>/dev/null || true
  [ -n "${ALACRITTY_PID:-}" ] && kill "$ALACRITTY_PID" 2>/dev/null || true
  wait 2>/dev/null || true
}
trap cleanup EXIT

echo "=== E2E IPC Action Test ==="
echo ""

# Launch alacritty
echo "[1] Launching alacritty..."
"$ALACRITTY" &
ALACRITTY_PID=$!

# Wait for socket
echo "[2] Waiting for IPC socket..."
SOCKET=""
for i in $(seq 1 40); do
  for f in "$XDG_RUNTIME_DIR"/Alacritty-*.sock; do
    [ -S "$f" ] && SOCKET="$f" && break 2
  done
  sleep 0.25
done
[ -z "$SOCKET" ] && { red "ERROR: socket not found"; exit 1; }
green "  Socket: $SOCKET"
SOCKET_ARG="-s $SOCKET"

# ====== Test 1: CreateTab via Action JSON ======
echo ""
echo "=== create_tab ==="
echo "[3] Send create_tab action"
send_action '{"action":"create_tab","command":["htop"],"no_switch":false}'
sleep 1.5

COUNT=$(list_tabs_count)
assert_ok "create_tab: tabs=$COUNT (expected 2)" "$([ "$COUNT" = "2" ] && echo ok || echo "got $COUNT")"

# ====== Test 2: select_tab ======
echo ""
echo "=== select_tab ==="
echo "[4] Send select_tab action (index 0)"
send_action '{"action":"select_tab","index":0}'
sleep 0.5

RESULT=$("$ALACRITTY" msg $SOCKET_ARG list-tabs 2>&1 | python3 -c "import sys,json;t=json.load(sys.stdin);print('ok' if t[0]['active'] else 'not-active')" 2>/dev/null || echo "err")
assert_ok "select_tab: tab 0 active" "$RESULT"

# ====== Test 3: split_pane ======
echo ""
echo "=== split_pane ==="
echo "[5] Send split_pane right"
send_action '{"action":"split_pane","direction":"right"}'
sleep 1.0

# ====== Test 4: focus_pane ======
echo ""
echo "=== focus_pane ==="
echo "[6] Send focus_pane left"
send_action '{"action":"focus_pane","direction":"left"}'
sleep 0.5

# ====== Test 5: zoom ======
echo ""
echo "=== toggle_zoom ==="
echo "[7] Send toggle_zoom"
send_action '{"action":"toggle_zoom"}'
sleep 0.3
send_action '{"action":"toggle_zoom"}'
sleep 0.3

# ====== Test 6: close_pane ======
echo ""
echo "=== close_pane ==="
echo "[8] Send close_pane"
send_action '{"action":"close_pane"}'
sleep 0.5

# ====== Test 7: save_session ======
echo ""
echo "=== save_session ==="
echo "[9] Send save_session"
send_action '{"action":"save_session"}'
sleep 0.5

SESSION_FILE=$(ls -t ~/.config/alacritty/sessions/session-*.json 2>/dev/null | head -1)
assert_ok "save_session: file=$SESSION_FILE" "$([ -n "$SESSION_FILE" ] && echo ok || echo 'no file')"

# ====== Test 8: select_next_tab ======
echo ""
echo "=== select_next_tab ==="
echo "[10] Send select_next_tab"
send_action '{"action":"select_next_tab"}'
sleep 0.3
send_action '{"action":"select_previous_tab"}'
sleep 0.3

# ====== Test 9: close_tab ======
echo ""
echo "=== close_tab ==="
echo "[11] Send close_tab index 1"
send_action '{"action":"close_tab","index":1}'
sleep 1.0
COUNT=$(list_tabs_count)
assert_ok "close_tab: tabs=$COUNT (expected 1)" "$([ "$COUNT" = "1" ] && echo ok || echo "got $COUNT")"

# ====== Test 10: create_tab --no-switch ======
echo ""
echo "=== create_tab no_switch ==="
echo "[12] Send create_tab (no_switch)"
send_action '{"action":"create_tab","command":["bash"],"no_switch":true}'
sleep 1.5

OUT=$("$ALACRITTY" msg $SOCKET_ARG list-tabs 2>&1)
RESULT=$(echo "$OUT" | python3 -c "import sys,json;t=json.load(sys.stdin);active=[x['index'] for x in t if x['active']];print('ok' if active and active[0]==1 else f'active={active}')" 2>/dev/null || echo "err")
assert_ok "create_tab (no_switch): tab-1 still active" "$RESULT"

# ====== Test 11: bell ======
echo ""
echo "=== bell ==="
echo "[13] Send bell"
send_action '{"action":"bell"}'
sleep 0.3

# ====== Cleanup ======
echo ""
echo "[14] Exit alacritty"
kill "$ALACRITTY_PID" 2>/dev/null || true
wait "$ALACRITTY_PID" 2>/dev/null || true

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
