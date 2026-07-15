# Web v1 Rollback Guide

## Rollback triggers

- Regression in a released core tag or npm version.
- Codec pack hash mismatch or malformed WASM binary.
- CDN immutable path corruption or misconfiguration.
- Security advisory in a dependency or container.

## Rollback order

Roll back in the reverse of the release order:

1. **npm dist-tag** — move `latest` to the previous published version.
2. **CDN immutable path** — revert the CDN pointer; do not overwrite the old path.
3. **Codec pack** — publish a new pack version or yank the broken one; do not delete.
4. **Core tag** — if the engine/ABI changed, revert to the previous compatible tag.
5. **Server facade** — if the media server protocol changed, revert `dodojohn83/cheetah-signaling`.

## npm rollback

```bash
# Identify the previous version
npm view @cheetah-media/web versions --json

# Set latest dist-tag back
npm dist-tag add @cheetah-media/web@<previous> latest

# Optionally deprecate the broken version with a reason
npm deprecate @cheetah-media/web@<broken> "rollback due to ..."
```

Do **not** unpublish a version that may be in use by installed clients; deprecate
instead so lockfiles and installed consumers still resolve.

## CDN rollback

For a versioned CDN path such as `https://cdn.example.com/cheetah-media/0.2.0/`:

```bash
# Point the alias back to the previous version
aws s3 cp s3://bucket/cheetah-media/0.1.0/ s3://bucket/cheetah-media/latest/ --recursive
```

Never overwrite `0.2.0/`; keep it available for clients that already loaded it.

## Codec pack rollback

Codec packs are identified by `manifest.json` fields `version`, `abi_version` and
`hash`. To roll back:

```bash
# Publish a corrected manifest with the previous pack version
# The loader selects packs by hash, so old clients continue to use the old pack
# until the SDK version containing the corrected manifest is deployed.
```

See `codec-packs/ffmpeg-wasm/manifest.json` and `codec-packs/ffmpeg-wasm/README.md`.

## Core / server rollback

1. Revert `dodojohn83/cheetah-media-engine` to the previous release candidate tag.
2. Rebuild and run CI (`rust`, `web`, `Devin Review`).
3. If the server facade (`dodojohn83/cheetah-signaling`) protocol changed, revert
   it to the matching commit.

## Forensics and SBOM

After any rollback:

- Preserve the broken SBOMs in `target/sbom/` (if generated during release).
- Preserve CI logs and diagnostics exports.
- File an issue linked from `known-limitations.md`.

## Rollback drill checklist

- [ ] Verify `latest` dist-tag resolves to the intended version.
- [ ] Verify CDN alias serves the intended files.
- [ ] Verify codec pack manifest points to the intended hash.
- [ ] Run `scripts/integration-smoke.sh` end-to-end.
- [ ] Confirm no open clients are pinned to the broken version.
