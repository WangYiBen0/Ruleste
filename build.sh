#!/usr/bin/env bash
# Click-to-build: compile the host binary and every Wasm entity plugin, then
# stage the resulting `.wasm` files into the `plugins/` folder next to the
# host executable (the game's default plugin dir).
#
# Usage: ./build.sh [--release]
set -euo pipefail
cd "$(dirname "$0")"

PROFILE=debug
if [[ "${1:-}" == "--release" || "${RULESTE_PROFILE:-}" == "release" ]]; then
    PROFILE=release
fi
TARGET_DIR="${CARGO_TARGET_DIR:-target}"
WASM_DIR="$TARGET_DIR/wasm32-unknown-unknown/release"
STAGING_DIR="$TARGET_DIR/$PROFILE/plugins"

echo "==> Building host ($PROFILE)"
if [[ "$PROFILE" == "release" ]]; then
    cargo build --release
else
    cargo build
fi

echo "==> Building Wasm plugins"
for manifest in plugins/*/Cargo.toml; do
    crate=$(sed -nE 's/^name *= *"(.*)".*/\1/p' "$manifest" | head -1)
    echo "    $crate"
    cargo build -p "$crate" --target wasm32-unknown-unknown --release
done

echo "==> Staging .wasm -> $STAGING_DIR"
mkdir -p "$STAGING_DIR"
cp "$WASM_DIR"/ruleste_*.wasm "$STAGING_DIR"/

echo "Done; host + $(ls "$STAGING_DIR"/*.wasm | wc -l) plugins ready under $STAGING_DIR"
