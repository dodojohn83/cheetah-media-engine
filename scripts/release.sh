#!/usr/bin/env bash
# Release helper for the Cheetah Media Engine JS packages.
# Usage:
#   scripts/release.sh 0.2.0        # bump, build, sbom, pack (dry-run)
#   scripts/release.sh 0.2.0 --publish  # bump, build, sbom, publish to npm
set -euo pipefail

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
  echo "Usage: $0 <version> [--publish]"
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PUBLISH=false
if [ "${2:-}" = "--publish" ]; then
  PUBLISH=true
fi

cd "$ROOT"

echo "[release] bumping workspace JS packages to $VERSION"
for pkg in packages/runtime packages/web packages/components; do
  node -e "
    const fs = require('node:fs');
    const p = process.argv[1];
    const version = process.argv[2];
    if (!version) { console.error('missing version'); process.exit(1); }
    const j = JSON.parse(fs.readFileSync(p, 'utf8'));
    j.version = version;
    fs.writeFileSync(p, JSON.stringify(j, null, 2) + '\\n');
  " "$pkg/package.json" "$VERSION"
done

echo "[release] installing and building"
source ~/.nvm/nvm.sh && nvm use
corepack pnpm install
corepack pnpm -r build

echo "[release] generating SBOMs"
"$ROOT/scripts/generate-sbom.sh"

echo "[release] dry-run packaging"
for pkg in packages/runtime packages/web packages/components; do
  (cd "$pkg" && corepack pnpm publish --dry-run --no-git-checks --access public)
done

if [ "$PUBLISH" = true ]; then
  if [ -z "${NPM_TOKEN:-}" ]; then
    echo "NPM_TOKEN is required for publish"
    exit 1
  fi
  echo "[release] publishing"
  # pnpm uses NPM_TOKEN from env when NODE_AUTH_TOKEN is not set
  export NPM_TOKEN
  for pkg in packages/runtime packages/web packages/components; do
    (cd "$pkg" && corepack pnpm publish --no-git-checks --access public)
  done
fi

echo "[release] done"
