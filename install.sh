#!/bin/sh
set -eu

REPO="https://github.com/sakashimaa/rsmon.git"
BINARY="rsmon"

# check rust
if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust not found. Install it: https://rustup.rs"
    exit 1
fi

echo "Installing ${BINARY}..."
cargo install --git "${REPO}"

echo "Done! Run: ${BINARY}"