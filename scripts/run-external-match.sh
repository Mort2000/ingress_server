#!/bin/sh
# One-shot external match: arena-server plus two client processes in one
# run, recording an IARP archive (first connection = ENL; launch order does
# not guarantee faction). Both clients MUST connect inside the handshake window
# (INITIAL_SPAWN_WINDOW + 5 s from server start — ~10 s in the default
# config, ~5.4 s in fast), so they are launched back to back here; running
# them from separate shells is what produces setup_error.
#
# usage: run-external-match.sh [options]
#   --client <path>      client binary        (../adaptive_bot/target/release/adaptive-bot)
#   --match-id <id>      match id             (m-<ppid>-<pid>-<seed>)
#   --seed <n>           game seed (decimal)  (12648430 = 0xC0FFEE)
#   --map-seed <n>       map seed             (7)
#   --config <preset>    default|fast         (default)
#   --rounds <n>         rounds               (preset default: 500 / 10)
#   --out <dir>          output root          (runs/external)
#   -h
#
# Exit status: the server's (0 normal, 3/4 spawn forfeit, 6/7 client
# forfeit, 9 setup error, 1 usage/io).

set -u

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CLIENT="$ROOT/../adaptive_bot/target/release/adaptive-bot"
MATCH_ID=""
SEED=12648430
MAP_SEED=7
CONFIG=default
ROUNDS=""
OUT="$ROOT/runs/external"

usage() {
    sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-1}"
}

while [ $# -gt 0 ]; do
    case "$1" in
        --client) CLIENT=${2:?--client needs a value}; shift 2 ;;
        --match-id) MATCH_ID=${2:?--match-id needs a value}; shift 2 ;;
        --seed) SEED=${2:?--seed needs a value}; shift 2 ;;
        --map-seed) MAP_SEED=${2:?--map-seed needs a value}; shift 2 ;;
        --config) CONFIG=${2:?--config needs a value}; shift 2 ;;
        --rounds) ROUNDS=${2:?--rounds needs a value}; shift 2 ;;
        --out) OUT=${2:?--out needs a value}; shift 2 ;;
        -h|--help) usage 0 ;;
        *) echo "unknown option: $1" >&2; usage ;;
    esac
done

case "$CONFIG" in default|fast) ;; *) echo "--config must be default or fast" >&2; exit 1 ;; esac

SERVER_BIN="$ROOT/target/release/arena-server"
[ -x "$CLIENT" ] || { echo "client not found or not executable: $CLIENT (run: cargo build --release --manifest-path ../adaptive_bot/Cargo.toml)" >&2; exit 1; }
[ -x "$SERVER_BIN" ] || { echo "arena-server not found: $SERVER_BIN (run: cargo build --release)" >&2; exit 1; }

[ -n "$MATCH_ID" ] || MATCH_ID="m-$$-$SEED"
SOCKET="/tmp/ingress-arena-$MATCH_ID.sock"
rm -f "$SOCKET"
mkdir -p "$OUT"

cleanup() {
    [ -n "${RES_PID:-}" ] && kill "$RES_PID" 2>/dev/null
    [ -n "${ENL_PID:-}" ] && kill "$ENL_PID" 2>/dev/null
    [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null
}
trap cleanup EXIT INT TERM

# -- server ---------------------------------------------------------------
"$SERVER_BIN" --socket "$SOCKET" --seed "$SEED" --map-seed "$MAP_SEED" \
    --config "$CONFIG" ${ROUNDS:+"--rounds" "$ROUNDS"} \
    --match-id "$MATCH_ID" --out "$OUT" &
SERVER_PID=$!

# -- clients (back to back, first connect = ENL) ---------------------------
i=0
while [ ! -S "$SOCKET" ]; do
    kill -0 "$SERVER_PID" 2>/dev/null || break
    i=$((i + 1))
    [ "$i" -gt 50 ] && { echo "socket never appeared" >&2; break; }
    sleep 0.1
done
"$CLIENT" "$SOCKET" "$SEED" &
ENL_PID=$!
"$CLIENT" "$SOCKET" "$SEED" &
RES_PID=$!

wait "$ENL_PID"; ENL_RC=$?
wait "$RES_PID"; RES_RC=$?
wait "$SERVER_PID"; SERVER_RC=$?

trap - EXIT INT TERM
echo "server exit=$SERVER_RC first_client exit=$ENL_RC second_client exit=$RES_RC"
echo "artifacts in $OUT/$MATCH_ID"

if [ "$SERVER_RC" -ne 0 ]; then
    exit "$SERVER_RC"
fi

exit 0
