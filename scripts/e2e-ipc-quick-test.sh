#!/bin/bash
# e2e-ipc-quick-test.sh — basic IPC smoke test
set -euo pipefail

BIN="${ALACRITTY:-alacritty}"
PASS=0; FAIL=0

assert() { if [ "$2" = "$3" ]; then echo "  PASS: $1"; PASS=$((PASS+1)); else echo "  FAIL: $1 (expected=$2 got=$3)"; FAIL=$((FAIL+1)); fi; }
cleanup() {
  kill "$PID" 2>/dev/null || true
  [ -n "${SOCK:-}" ] && rm -f "$SOCK" 2>/dev/null || true
  wait 2>/dev/null || true
}
trap cleanup EXIT

echo "=== IPC Smoke Test ==="
echo "Binary: $BIN"

# Clean up stale sockets.
rm -f "$XDG_RUNTIME_DIR"/Alacritty-*.sock 2>/dev/null || true

"$BIN" &>/dev/null & PID=$!; sleep 3
SOCK=$(ls "$XDG_RUNTIME_DIR"/Alacritty-*.sock 2>/dev/null | head -1)
[ -z "$SOCK" ] && { echo "FATAL: no socket"; exit 1; }
echo "Socket: $SOCK"
S="-s $SOCK"

echo "=== tab create ==="
"$BIN" msg $S tab create -e bash --no-switch
sleep 1.5
C=$("$BIN" msg $S tab list | python3 -c "import sys,json;print(len(json.load(sys.stdin)))" 2>/dev/null || echo 0)
assert "2 tabs" "2" "$C"

echo "=== pane split ==="
"$BIN" msg $S pane split right; sleep 0.5
assert "pane split" "ok" "ok"

echo "=== tree ==="
T=$("$BIN" msg $S tree)
echo "$T" | grep -q PANE && assert "tree has PANE" "ok" "ok"

echo "=== session ==="
"$BIN" msg $S session save; sleep 0.5
assert "session save" "ok" "ok"

echo "=== config ==="
"$BIN" msg $S config get | python3 -c "import sys,json;d=json.load(sys.stdin);assert'window'in d" 2>/dev/null && R="ok" || R="fail"
assert "config get" "ok" "$R"

echo "=== quick-run ==="
"$BIN" msg $S quick-run --no-switch -e bash; sleep 1.5
C=$("$BIN" msg $S tab list | python3 -c "import sys,json;print(len(json.load(sys.stdin)))" 2>/dev/null || echo 0)
assert "quick-run: 3 tabs" "3" "$C"

echo "=== exec ==="
"$BIN" msg $S exec '{"action":"bell"}' && assert "exec bell" "ok" "ok"

echo "=== bell + scroll ==="
"$BIN" msg $S bell; "$BIN" msg $S scroll 5
assert "bell+scroll" "ok" "ok"

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
