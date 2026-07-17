#!/usr/bin/env bash
# Generate SPDX / CycloneDX SBOM artifacts for release.
# Requires: cargo-cyclonedx, npm >=10.9 (for npm sbom with package-lock.json)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${ROOT}/target/sbom"
mkdir -p "$OUT"

echo "[sbom] Rust workspace (CycloneDX per crate)"
find "$ROOT/crates" -name '*.cdx.json' -delete 2>/dev/null || true
(cd "$ROOT" && cargo cyclonedx --format json >/dev/null 2>&1) || true
find "$ROOT/crates" -name '*.cdx.json' -print0 2>/dev/null | while IFS= read -r -d '' f; do
  name=$(basename "$f")
  mv "$f" "$OUT/${name%.cdx.json}.cyclonedx.json"
done

echo "[sbom] npm packages (SPDX) — requires a package-lock.json per package"
for pkg in packages/runtime packages/web packages/components; do
  name=$(basename "$pkg")
  if [ -f "$ROOT/$pkg/package-lock.json" ]; then
    (cd "$ROOT/$pkg" && npm sbom --sbom-format=spdx --sbom-type=library --package-lock-only > "$OUT/$name.spdx.json")
  else
    echo "[sbom] skipping $name: no package-lock.json (run npm install in $pkg to generate one)" >&2
  fi
done

echo "[sbom] WASM / codec pack manifests"
cp "$ROOT/codec-packs/ffmpeg-wasm/manifest.json" "$OUT/ffmpeg-wasm-manifest.json" 2>/dev/null || true

echo "[sbom] outputs in $OUT"
ls -la "$OUT"
