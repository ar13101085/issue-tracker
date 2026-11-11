#!/usr/bin/env bash
# Builds the single Linux executable (frontend embedded, static musl binary) and the CLI.
#   DIST_DIR  where to put the binaries (default: ./dist)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
TARGET=x86_64-unknown-linux-musl
"$ROOT/scripts/build-frontend.sh"
cd "$ROOT"
rustup target list --installed | grep -qx "$TARGET" || rustup target add "$TARGET"
# `ring` compiles C code; clang can target musl without a musl-gcc toolchain.
export CC_x86_64_unknown_linux_musl="${CC_x86_64_unknown_linux_musl:-clang}"
export CFLAGS_x86_64_unknown_linux_musl="${CFLAGS_x86_64_unknown_linux_musl:---target=$TARGET}"
cargo build --release --target "$TARGET"
DIST="${DIST_DIR:-$ROOT/dist}"
mkdir -p "$DIST"
for bin in issue-tracker it; do
  cp "$ROOT/target/$TARGET/release/$bin" "$DIST/$bin"
  echo "binary → $DIST/$bin ($(du -h "$DIST/$bin" | cut -f1), static)"
done
