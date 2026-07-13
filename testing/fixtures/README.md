# Test Fixtures

This directory contains the fixture manifest and small in-repo fixtures for the Cheetah Media Engine workspace.

## Manifest

`manifest.json` is the single source of truth for fixture metadata and is read by `crates/cheetah-media-testkit`.

Each fixture record includes:

- `id` and `description`
- `source` (type, URL, generator, commit, hash)
- `license` and `hash`
- Protocol, codec, resolution, frame rate, sample rate, channels, duration
- `anomaly` for boundary/negative/corrupt cases
- `expected` for the expected parser/player output

## Large files

Large binary fixtures are not stored in this repository. They are placed in object storage and referenced in the manifest by `hash` and `source.url`. The `hash` is verified by `cheetah-media-testkit` before use.

## Prohibited data

No fixture may contain credentials, device identifiers, geographic information, personal images, or unauthorized surveillance footage.
