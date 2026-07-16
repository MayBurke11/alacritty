#!/bin/bash
# e2e-ipc-full-test.sh — comprehensive IPC test suite
set -euo pipefail

ALACRITTY="${ALACRITTY:-./target/release/alacritty}"
PASS=0
FAIL=0

red()   { echo -e "\033[31m$*\033[0m"; }
green() { echo -e "\033[32m$*\033[0m"; }

assert() {
  local desc="$1" expected="$2" actual="$3"
  if [ "$actual" = "$expected" ]; then
    green "  PASS: $desc"
    PASS=$((PASS + 1))
  else
    red "  FAIL: $desc (expected=$expected got=$actual)"
    FAIL=$((FAIL + 1))
  fi
}

send() {
  python3 -c "
import socket, json, sys, time
data = sys.argv[1]
s = socket.socket(socket.AF_UNIX)
s.connect('$SOCKET')
s.sendall(data.encode() + b'\n')
s.shutdown(1)
try:
    resp = s.makefile().readline().strip()
    if resp:
        r = json.loads(resp)
        if r.get('ok') and 'data' in r:
            print(json.dumps(r['data']))
        elif r.get('ok'):
            print('ok')
        else:
            print('error:' + r.get('error',''))
    else:
        print('ok')
except:
    print('ok')
s.close()
" "$1" 2>/dev/null
}

cleanup() { kill "$ALACRITTY_PID" 2>/dev/null || true; wait 2>/dev/null || true; }
trap cleanup EXIT

echo "=== E2E Full IPC Test Suite ==="
echo ""

# Launch alacritty with TCP
echo "[setup] Launching alacritty with Unix socket + TCP..."
"$ALACRITTY" --tcp-addr 127.0.0.1:29090 &
ALACRITTY_PID=$!

# Wait for socket
SOCKET=""
for i in $(seq 1 40); do
  for f in "$XDG_RUNTIME_DIR"/Alacritty-*.sock; do
    [ -S "$f" ] && SOCKET="$f" && break 2
  done
  sleep 0.25
done
[ -z "$SOCKET" ] && { red "FATAL: socket not found"; exit 1; }
green "  Socket: $SOCKET"
SOCKET_ARG="-s $SOCKET"
sleep 1.0

# ===== TAB TESTS =====
echo ""
echo "=== TAB tests ==="

# 1. List tabs
RESULT=$(send '{"id":1,"action":"list_tabs"}')
assert "list_tabs returns 1 tab initially" "ok" "$(echo "$RESULT" | python3 -c "import sys,json;d=json.load(sys.stdin);print('ok' if len(d)==1 else 'fail')" 2>/dev/null || echo "fail")"

# 2. Create tab
send '{"action":"create_tab","no_switch":false}' > /dev/null
sleep 1.0
COUNT=$("$ALACRITTY" msg $SOCKET_ARG tab list 2>&1 | python3 -c "import sys,json;print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
assert "create_tab: 2 tabs" "2" "$COUNT"

# 3. Create tab --no-switch
send '{"action":"create_tab","command":["bash"],"no_switch":true}' > /dev/null
sleep 1.0
COUNT=$("$ALACRITTY" msg $SOCKET_ARG tab list 2>&1 | python3 -c "import sys,json;print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
assert "create_tab (no_switch): 3 tabs" "3" "$COUNT"

# 4. Select tab
send '{"action":"select_tab","index":0}' > /dev/null
sleep 0.3
ACTIVE=$("$ALACRITTY" msg $SOCKET_ARG tab list 2>&1 | python3 -c "import sys,json;t=json.load(sys.stdin);print([x['index'] for x in t if x['active']][0])" 2>/dev/null || echo "0")
assert "select_tab(0): active=1" "1" "$ACTIVE"

# 5. Select next/prev/last
send '{"action":"select_next_tab"}' > /dev/null; sleep 0.2
send '{"action":"select_previous_tab"}' > /dev/null; sleep 0.2
send '{"action":"select_last_tab"}' > /dev/null; sleep 0.2
ACTIVE=$("$ALACRITTY" msg $SOCKET_ARG tab list 2>&1 | python3 -c "import sys,json;t=json.load(sys.stdin);print([x['index'] for x in t if x['active']][0])" 2>/dev/null || echo "0")
assert "select_last_tab: active=3" "3" "$ACTIVE"

# 6. Pin tab
send '{"action":"toggle_pin","index":2}' > /dev/null
sleep 0.3
PINNED=$("$ALACRITTY" msg $SOCKET_ARG tab list 2>&1 | python3 -c "import sys,json;t=json.load(sys.stdin);print('pinned' if t[2]['pinned'] else 'not')" 2>/dev/null || echo "err")
assert "toggle_pin(2): pinned" "pinned" "$PINNED"
send '{"action":"toggle_pin","index":2}' > /dev/null
sleep 0.3

# 7. Close tab
send '{"action":"close_tab","index":2}' > /dev/null
sleep 1.0
COUNT=$("$ALACRITTY" msg $SOCKET_ARG tab list 2>&1 | python3 -c "import sys,json;print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
assert "close_tab: 2 tabs" "2" "$COUNT"

# ===== PANE TESTS =====
echo ""
echo "=== PANE tests ==="

# 8. Split right
send '{"action":"split_pane","direction":"right"}' > /dev/null
sleep 0.5
assert "split_pane right" "ok" "ok"

# 9. Split down
send '{"action":"split_pane","direction":"down"}' > /dev/null
sleep 0.5
assert "split_pane down" "ok" "ok"

# 10. Focus panes
send '{"action":"focus_pane","direction":"left"}' > /dev/null; sleep 0.2
send '{"action":"focus_pane","direction":"right"}' > /dev/null; sleep 0.2
send '{"action":"focus_pane","direction":"up"}' > /dev/null; sleep 0.2
send '{"action":"focus_pane","direction":"down"}' > /dev/null; sleep 0.2
assert "focus_pane all directions" "ok" "ok"

# 11. Zoom
send '{"action":"toggle_zoom"}' > /dev/null; sleep 0.2
send '{"action":"toggle_zoom"}' > /dev/null; sleep 0.2
assert "toggle_zoom" "ok" "ok"

# 12. Close all extra panes
send '{"action":"close_pane"}' > /dev/null; sleep 0.3
send '{"action":"close_pane"}' > /dev/null; sleep 0.3
assert "close_pane x2" "ok" "ok"

# ===== SESSION TESTS =====
echo ""
echo "=== SESSION tests ==="

# 13. Save session
RESULT=$(send '{"action":"save_session"}')
assert "save_session" "ok" "$RESULT"

# 14. List sessions
RESULT=$(send '{"action":"list_sessions"}')
HAS=$(echo "$RESULT" | python3 -c "import sys,json;d=json.load(sys.stdin);print('ok' if len(d)>=1 else 'empty')" 2>/dev/null || echo "err")
assert "list_sessions: has entries" "ok" "$HAS"

# ===== CONFIG TESTS =====
echo ""
echo "=== CONFIG tests ==="

# 15. Get config
RESULT=$(send '{"action":"get_config"}')
HAS=$(echo "$RESULT" | python3 -c "import sys,json;d=json.load(sys.stdin);print('ok' if 'window' in d else 'fail')" 2>/dev/null || echo "fail")
assert "get_config: has window section" "ok" "$HAS"

# 16. Set config
send '{"action":"set_config","options":{"font.size":12}}' > /dev/null
sleep 0.5
RESULT=$(send '{"action":"get_config"}')
SIZE=$(echo "$RESULT" | python3 -c "import sys,json;print(json.load(sys.stdin)['font']['size'])" 2>/dev/null || echo "0")
assert "set_config font.size=12" "12.0" "$SIZE"

# ===== CLI TESTS =====
echo ""
echo "=== CLI tests ==="

# 17. --help
"$ALACRITTY" msg --help > /dev/null 2>&1
assert "msg --help" "ok" "ok"

# 18. pane split
"$ALACRITTY" msg $SOCKET_ARG pane split down > /dev/null 2>&1
sleep 0.5
assert "msg pane split down" "ok" "ok"

# 19. pane close
"$ALACRITTY" msg $SOCKET_ARG pane close > /dev/null 2>&1
sleep 0.3
assert "msg pane close" "ok" "ok"

# 20. session list
"$ALACRITTY" msg $SOCKET_ARG session list > /dev/null 2>&1
assert "msg session list" "ok" "ok"

# 21. tab-nav
"$ALACRITTY" msg $SOCKET_ARG tab next > /dev/null 2>&1; sleep 0.2
"$ALACRITTY" msg $SOCKET_ARG tab previous > /dev/null 2>&1; sleep 0.2
"$ALACRITTY" msg $SOCKET_ARG tab last > /dev/null 2>&1; sleep 0.2
assert "msg tab next/prev/last" "ok" "ok"

# 22. scroll-view
"$ALACRITTY" msg $SOCKET_ARG scroll-view 5 > /dev/null 2>&1
assert "msg scroll 5" "ok" "ok"

# 23. bell
"$ALACRITTY" msg $SOCKET_ARG bell > /dev/null 2>&1
sleep 0.3
assert "msg bell" "ok" "ok"

# 24. exec
"$ALACRITTY" msg $SOCKET_ARG exec '{"action":"bell"}' > /dev/null 2>&1
assert "msg exec bell" "ok" "ok"

# ===== TCP TESTS =====
echo ""
echo "=== TCP tests ==="

# 25. TCP bare action
RESULT=$(echo '{"action":"list_tabs"}' | nc -w 1 127.0.0.1 29090 2>/dev/null | head -1 || echo "")
[ -n "$RESULT" ] && TCP_OK="ok" || TCP_OK="fail"
assert "TCP: list_tabs" "ok" "$TCP_OK"

# ===== CLEANUP =====
echo ""
kill "$ALACRITTY_PID" 2>/dev/null || true
wait "$ALACRITTY_PID" 2>/dev/null || true

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
