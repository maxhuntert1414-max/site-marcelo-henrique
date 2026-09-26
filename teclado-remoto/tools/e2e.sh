#!/usr/bin/env bash
# Teste ponta a ponta: servidor do PC em modo headless + cliente Kotlin do app Android.
# Uso: tools/e2e.sh   (precisa de cargo e JDK 17+)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${TRK_E2E_PORT:-47900}"
DISCOVERY_PORT="${TRK_E2E_DISCOVERY_PORT:-47910}"
WORK="$(mktemp -d)"
SERVER_PID=""
cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    rm -rf "$WORK"
}
trap cleanup EXIT

(cd "$ROOT/pc-windows" && cargo build --quiet)
TRK_CONFIG_DIR="$WORK" TRK_PORT="$PORT" TRK_DISCOVERY_PORT="$DISCOVERY_PORT" TRK_AUTO_ACCEPT=1 \
    "$ROOT/pc-windows/target/debug/teclado-remoto" >"$WORK/server.out" 2>"$WORK/server.err" &
SERVER_PID=$!
for _ in $(seq 50); do
    grep -q "^READY" "$WORK/server.out" 2>/dev/null && break
    sleep 0.1
done
grep -q "^READY $PORT" "$WORK/server.out" || { echo "servidor não subiu"; cat "$WORK/server.err"; exit 1; }

(cd "$ROOT/android" && TRK_E2E_PORT="$PORT" TRK_E2E_DISCOVERY_PORT="$DISCOVERY_PORT" \
    ./gradlew :core:test --tests '*EndToEndTest*' --rerun --quiet)
sleep 0.5

expect() {
    grep -qF -- "$1" "$WORK/server.out" || {
        echo "FALTOU no servidor: $1"
        echo "--- saída do servidor ---"
        cat "$WORK/server.out"
        exit 1
    }
}
expect "PAIR_REQUEST"
expect "PAIRED Celular de teste"
expect "SESSIONS [Celular de teste@127.0.0.1]"
expect "UNICODE U+00E1 down"   # á
expect "UNICODE U+00E7 down"   # ç
expect "UNICODE U+00E3 down"   # ã
expect "UNICODE U+D83D down"   # 😀 (par substituto)
expect "UNICODE U+DE00 up"
expect "KEY 0x0D down"         # \n -> Enter
expect "KEY 0x54 down"         # Ctrl+Shift+T
expect "KEY 0x56 down"         # Ctrl+V vindo de CHAR
expect 'CLIPBOARD "do celular"'
expect "KEY 0x12 down"         # Alt segurado...
expect "KEY 0x12 up"           # ...e solto quando a conexão caiu
expect "MOUSE_MOVE -3 4"
expect "KEY 0x08 down"         # Backspace de type(2, "fim")
expect "UNICODE U+0066 down"   # f
echo "E2E OK: $(grep -c . "$WORK/server.out") linhas de eventos conferidas"
