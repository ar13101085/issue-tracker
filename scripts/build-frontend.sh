#!/usr/bin/env bash
# Builds the Next.js static export into frontend/out (embedded by the server binary).
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
export PATH="$HOME/.nvm/versions/node/v24.19.0/bin:$PATH"
cd "$ROOT/frontend"
pnpm install --frozen-lockfile
pnpm build
echo "frontend built → $ROOT/frontend/out"
