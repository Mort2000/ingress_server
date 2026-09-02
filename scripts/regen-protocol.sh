#!/usr/bin/env bash
# Regenerates crates/arena-protocol/src/generated/ from the committed .fbs
# schemas (D60). NOT needed to build: the generated Rust is committed.
# Requires planus-cli 1.3.0 (cargo install planus-cli --locked).
#
# planus 1.3 unconditionally derives ::serde::Serialize/Deserialize on every
# generated type; this script strips those two derive entries (the workspace
# has no serde dependency). Everything else is left byte-identical, and the
# result is formatted with rustfmt (planus already formats, but be safe).
set -euo pipefail
cd "$(dirname "$0")/.."

PLANUS="${PLANUS:-$HOME/.cargo/bin/planus}"
SCHEMAS="crates/arena-protocol/schemas"
OUT="crates/arena-protocol/src/generated"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

"$PLANUS" check "$SCHEMAS/protocol.fbs" "$SCHEMAS/public_snapshot.fbs" "$SCHEMAS/private_snapshot.fbs"
"$PLANUS" rust -o "$TMP/protocol.rs" "$SCHEMAS/protocol.fbs"
"$PLANUS" rust -o "$TMP/public.rs" "$SCHEMAS/public_snapshot.fbs"
"$PLANUS" rust -o "$TMP/private.rs" "$SCHEMAS/private_snapshot.fbs"

strip_serde() {
    # Remove exactly the two unconditional serde derive entries.
    sed -i '/^ *::serde::Serialize,$/d; /^ *::serde::Deserialize,$/d' "$1"
}
strip_serde "$TMP/protocol.rs"
strip_serde "$TMP/public.rs"
strip_serde "$TMP/private.rs"

rustfmt --edition 2024 "$TMP/protocol.rs" "$TMP/public.rs" "$TMP/private.rs"

mkdir -p "$OUT"
cp "$TMP/protocol.rs" "$OUT/protocol.rs"
cp "$TMP/public.rs" "$OUT/snapshot_public.rs"
cp "$TMP/private.rs" "$OUT/snapshot_private.rs"

# Fail loudly if the committed code drifted from what the schemas produce.
if ! git diff --quiet -- "$OUT" 2>/dev/null; then
    echo "regen-protocol.sh: generated code changed under $OUT (commit the result)" >&2
fi
echo "regenerated: $OUT"
