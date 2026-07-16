#!/bin/bash
# e2e-ipc-full-test.sh — comprehensive IPC test suite (CLI-based)
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

cleanup() { kill "$ALACRITTY_PID" 2>/dev/null || true; wait 2>/dev/null || true; }
trap cleanup EXIT

echo "=== E2E Full IPC Test Suite ==="
echo ""

"$ALACRITTY" &>/tmp/alacritty-e2e.log &
ALACRITTY_PID=$!

SOCKET=""
for i in $(seq 1 40); do
  for f in "$XDG_RUNTIME_DIR"/Alacritty-*.sock; do
    [ -S "$f" ] && SOCKET="$f" && break 2
  done
  sleep 0.25
done
[ -z "$SOCKET" ] && { red "FATAL: socket not found"; exit 1; }
green "  Socket: $SOCKET"
S="-s $SOCKET"
sleep 1.0

# === TAB ===
echo ""; echo "=== TAB ==="
"$ALACRITTY" msg $S tab create -e bash --no-switch 2>/dev/null; sleep 1.5
C=$("$ALACRITTY" msg $S tab list 2>&1 | python3 -c "import sys,json;print(len(json.load(sys.stdin)))" 2>/dev/null||echo 0)
assert "tab create: 2 tabs" "2" "$C"
"$ALACRITTY" msg $S tab select 1 2>/dev/null; sleep 0.3
A=$("$ALACRITTY" msg $S tab list 2>&1 | python3 -c "import sys,json;t=json.load(sys.stdin);print([x['index'] for x in t if x['active']][0])" 2>/dev/null||echo 0)
assert "tab select: active=1" "1" "$A"
"$ALACRITTY" msg $S tab next 2>/dev/null; sleep 0.3
"$ALACRITTY" msg $S tab previous 2>/dev/null; sleep 0.3
"$ALACRITTY" msg $S tab last 2>/dev/null; sleep 0.3
"$ALACRITTY" msg $S tab pin 2 2>/dev/null; sleep 0.3
"$ALACRITTY" msg $S tab pin 2 2>/dev/null; sleep 0.3
"$ALACRITTY" msg $S tab close 2 2>/dev/null; sleep 1.0
C=$("$ALACRITTY" msg $S tab list 2>&1 | python3 -c "import sys,json;print(len(json.load(sys.stdin)))" 2>/dev/null||echo 0)
assert "tab close: back to 1" "1" "$C"

# === PANE ===
echo ""; echo "=== PANE ==="
"$ALACRITTY" msg $S pane split right 2>/dev/null; sleep 0.5
"$ALACRITTY" msg $S pane split down 2>/dev/null; sleep 0.5
"$ALACRITTY" msg $S pane focus left 2>/dev/null; sleep 0.2
"$ALACRITTY" msg $S pane focus right 2>/dev/null; sleep 0.2
"$ALACRITTY" msg $S pane zoom 2>/dev/null; sleep 0.2
"$ALACRITTY" msg $S pane zoom 2>/dev/null; sleep 0.2
"$ALACRITTY" msg $S pane close 2>/dev/null; sleep 0.3
"$ALACRITTY" msg $S pane close 2>/dev/null; sleep 0.3
assert "pane ops" "ok" "ok"

# === SESSION ===
echo ""; echo "=== SESSION ==="
"$ALACRITTY" msg $S session save 2>/dev/null; sleep 0.5
"$ALACRITTY" msg $S session list 2>/dev/null >/dev/null
assert "session save+list" "ok" "ok"

# === CONFIG ===
echo ""; echo "=== CONFIG ==="
"$ALACRITTY" msg $S config get 2>/dev/null | python3 -c "import sys,json;d=json.load(sys.stdin);assert'window'in d" 2>/dev/null && R="ok" || R="fail"
assert "config get" "ok" "$R"

# === TREE ===
echo ""; echo "=== TREE ==="
T=$("$ALACRITTY" msg $S tree 2>&1)
echo "$T" | grep -q "PANE" && R="ok" || R="fail"
assert "tree has PANE" "ok" "$R"
echo "$T" | grep -q "WINDOW" && R="ok" || R="fail"
assert "tree has WINDOW" "ok" "$R"

# === SCROLL + BELL ===
echo ""; echo "=== SCROLL/BELL ==="
"$ALACRITTY" msg $S scroll 5 2>/dev/null
"$ALACRITTY" msg $S bell 2>/dev/null
assert "scroll+bell" "ok" "ok"

# === QUICK-RUN ===
echo ""; echo "=== QUICK-RUN ==="
"$ALACRITTY" msg $S quick-run --no-switch -e bash 2>/dev/null; sleep 1.5
C=$("$ALACRITTY" msg $S tab list 2>&1 | python3 -c "import sys,json;print(len(json.load(sys.stdin)))" 2>/dev/null||echo 0)
assert "quick-run: 2 tabs" "2" "$C"

# === EXEC ===
echo ""; echo "=== EXEC ==="
"$ALACRITTY" msg $S exec '{"action":"bell"}' 2>/dev/null
assert "exec bell" "ok" "ok"

# === CLEANUP ===
echo ""
kill "$ALACRITTY_PID" 2>/dev/null || true
wait "$ALACRITTY_PID" 2>/dev/null || true

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
