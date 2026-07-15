#!/usr/bin/env bash
set -euo pipefail

# Run the Web v1 acceptance evidence pipeline:
# 1. Ensure real media fixtures are generated.
# 2. Build the web demo.
# 3. Run the Playwright playback matrix.
# 4. Collect evidence JSON into testing/fixtures/evidence/.
# 5. Optionally run API report and benchmark data collection.
#
# Usage:
#   scripts/run-acceptance.sh
#   REGENERATE=1 scripts/run-acceptance.sh
#   SKIP_BENCH=1 scripts/run-acceptance.sh

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

# Load Node version pinned by .nvmrc if nvm is available.
if [ -s "$HOME/.nvm/nvm.sh" ] && [ -r .nvmrc ]; then
  # shellcheck source=/dev/null
  source "$HOME/.nvm/nvm.sh" && nvm use
fi

# Validate the fixture manifest and Rust testkit.
cargo test -p cheetah-media-testkit

# Generate fixtures only if missing or if REGENERATE is set.
if [ "${REGENERATE:-0}" = "1" ] || [ ! -d "testing/fixtures/media/h264-http-fmp4" ]; then
  if ! command -v ffmpeg >/dev/null 2>&1 || ! command -v ffprobe >/dev/null 2>&1; then
    echo "ERROR: ffmpeg and ffprobe are required to generate fixtures." >&2
    exit 1
  fi
  node scripts/generate-fixtures.mjs
else
  echo "[acceptance] fixtures already present; set REGENERATE=1 to regenerate"
fi

# Build the demo so the preview server and playback harness are up to date.
echo "[acceptance] building web-demo..."
corepack pnpm --filter @cheetah-media/web-demo build

# Run the real-media playback matrix in Chromium.
echo "[acceptance] running playback matrix..."
corepack pnpm --filter @cheetah-media/browser-tests test -- --project=chromium --reporter=list

# Collect evidence artifacts.
EVIDENCE_DIR="testing/fixtures/evidence/$(date -u +%Y%m%d-%H%M%S)"
mkdir -p "$EVIDENCE_DIR"
find tests/browser/test-results -path '*/attachments/playback-evidence-*.json' -print0 2>/dev/null \
  | xargs -0 -I {} cp {} "$EVIDENCE_DIR/"
echo "[acceptance] copied playback evidence to $EVIDENCE_DIR"

# Optional: collect benchmark raw data and report.
if [ "${SKIP_BENCH:-0}" != "1" ]; then
  echo "[acceptance] collecting benchmark data..."
  mkdir -p "$EVIDENCE_DIR/bench"
  cargo bench -p cheetah-media-types --features std
  find target/criterion -name 'sample.json' -print0 2>/dev/null | while IFS= read -r -d '' f; do
    bench=$(basename "$(dirname "$(dirname "$f")")")
    cp "$f" "$EVIDENCE_DIR/bench/${bench}-sample.json"
  done || true
  find target/criterion -name 'estimates.json' -print0 2>/dev/null | while IFS= read -r -d '' f; do
    bench=$(basename "$(dirname "$(dirname "$f")")")
    cp "$f" "$EVIDENCE_DIR/bench/${bench}-estimates.json"
  done || true
  node scripts/generate-benchmark-report.mjs
  cp docs/web-v1-handoff/benchmark-report.md "$EVIDENCE_DIR/"
fi

# Optional: generate the API report.
if [ "${SKIP_API:-0}" != "1" ]; then
  echo "[acceptance] generating API report..."
  node scripts/generate-api-report.mjs
  cp docs/web-v1-handoff/api-report.md "$EVIDENCE_DIR/"
fi

echo "[acceptance] done. Evidence in $EVIDENCE_DIR"
