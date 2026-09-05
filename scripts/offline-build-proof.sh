#!/usr/bin/env bash
# P4-BUILD offline proof (D72): builds the workspace from the committed
# vendor/ closure with NO network, NO cargo caches, and NO inherited
# environment:
#
#   - fresh CARGO_HOME and fresh CARGO_TARGET_DIR under a mktemp dir
#     (no registry cache, no build cache reuse),
#   - env -i (only PATH/HOME/CARGO_* survive; every proxy and mirror
#     variable is dropped),
#   - the REAL toolchain cargo on PATH (no rustup proxy involved — the
#     pinned toolchain directory from rust-toolchain.toml),
#   - --locked (asserts Cargo.lock is untouched) and --offline.
#
# Evidence is recorded under runs/builds/<id>/: the build log, the
# fresh-environment capture, the binary hashes, and the provenance
# manifest. The pinned toolchain must be pre-provisioned via rustup
# (the one out-of-band network step; see docs/design/build-provenance.md).
set -euo pipefail
cd "$(dirname "$0")/.."
REPO="$PWD"

PINNED_CHANNEL="$(grep -m1 '^channel' rust-toolchain.toml | sed 's/channel *= *"\(.*\)"/\1/')"
TOOLCHAIN_DIR="$HOME/.rustup/toolchains/${PINNED_CHANNEL}-x86_64-unknown-linux-gnu"
REAL_CARGO="$TOOLCHAIN_DIR/bin/cargo"
if [[ ! -x "$REAL_CARGO" ]]; then
    echo "offline-build-proof.sh: pinned toolchain not provisioned:" >&2
    echo "  rustup toolchain install $PINNED_CHANNEL --profile minimal --component rustfmt" >&2
    exit 1
fi
echo "pinned toolchain: $PINNED_CHANNEL ($(rustc -V))"

PROOF="$(mktemp -d /tmp/ingress-offline-proof.XXXXXX)"
trap 'rm -rf "$PROOF"' EXIT
mkdir -p "$PROOF/cargo-home" "$PROOF/target"

echo "== fresh env: CARGO_HOME=$PROOF/cargo-home TARGET=$PROOF/target"
echo "== env -i build --release --locked --offline"
set +e
env -i \
    PATH="$TOOLCHAIN_DIR/bin:/usr/bin:/bin" \
    HOME="$PROOF/cargo-home" \
    CARGO_HOME="$PROOF/cargo-home" \
    CARGO_TARGET_DIR="$PROOF/target" \
    "$REAL_CARGO" build --release --locked --offline \
    >"$PROOF/build.log" 2>&1
STATUS=$?
set -e
if [[ $STATUS -ne 0 ]]; then
    echo "== BUILD FAILED (exit $STATUS); last 30 lines:" >&2
    tail -30 "$PROOF/build.log" >&2
    exit $STATUS
fi
echo "== build OK"

# Record the evidence: hashes of the fresh products + the provenance
# manifest of THIS tree. The manifest hashes the committed closure
# (vendor/ included), so it identifies the audited inputs that produced
# these binaries.
EVIDENCE="$REPO/runs/builds"
mkdir -p "$EVIDENCE"
( cd "$PROOF/target/release" && sha256sum arena-server nearest-client hybrid-client probe-client \
    provenance audit sandbox-runner sb-malloc-bomb sb-spin sb-net-probe sb-fs-probe sb-env-report \
    > "$PROOF/binary-hashes.txt" )

# The provenance tool itself was just built offline; use IT to record
# the manifest (its target dir flag points at the fresh build).
env -i \
    PATH="$TOOLCHAIN_DIR/bin:/usr/bin:/bin" \
    HOME="$PROOF/cargo-home" \
    CARGO_HOME="$PROOF/cargo-home" \
    RUSTUP_TOOLCHAIN="$PINNED_CHANNEL" \
    "$PROOF/target/release/provenance" \
        --root "$REPO" --target-dir "$PROOF/target" --profile release \
        --out-dir "$PROOF/builds" > "$PROOF/manifest.log"

BUILD_ID="$(awk '/^build-id/{print $2}' "$PROOF/manifest.log")"
DEST="$EVIDENCE/$BUILD_ID"
mkdir -p "$DEST"
cp "$PROOF/build.log" "$DEST/build.log"
cp "$PROOF/binary-hashes.txt" "$DEST/binary-hashes.txt"
cp "$PROOF/manifest.log" "$DEST/manifest.log"
cp "$PROOF/builds/$BUILD_ID/manifest.json" "$DEST/manifest.json"
cat > "$DEST/env.txt" <<EOF
env -i PATH="$TOOLCHAIN_DIR/bin:/usr/bin:/bin" HOME=fresh CARGO_HOME=fresh
cargo build --release --locked --offline
toolchain=$PINNED_CHANNEL
cargo=$("$REAL_CARGO" -V)
rustc=$("$TOOLCHAIN_DIR/bin/rustc" -Vv | head -4 | tr '\n' ' ')
EOF

echo "== evidence recorded: $DEST"
echo "build-id: $BUILD_ID"
grep -E "^(tree-hash|lock-hash|files-hashed)" "$PROOF/manifest.log"
