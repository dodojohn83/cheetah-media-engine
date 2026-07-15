#!/usr/bin/env bash
# Web v1 integration smoke test.
# Builds the demo, starts the preview server, and runs the Playwright E2E suite.
# For full acceptance, point the player at real media endpoints and extend the
# Playwright tests in tests/browser/tests/integration-matrix.spec.ts.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

source ~/.nvm/nvm.sh && nvm use

echo "[smoke] building workspace"
corepack pnpm install --frozen-lockfile
corepack pnpm -r build

echo "[smoke] running browser E2E (isolated + non-isolated, chromium/firefox/webkit)"
cd "$ROOT/tests/browser"
corepack pnpm exec playwright test --reporter=line

echo "[smoke] done"
