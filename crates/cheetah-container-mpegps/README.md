# cheetah-container-mpegps

MPEG-2 Program Stream (MPEG-PS) container parser.

## Scope

- Parse pack headers and PES packets from an MPEG-PS byte stream.
- Emit `MediaPacket` events for video (H.264/H.265) and audio (AAC) tracks.
- Delegate video elementary stream slicing to `cheetah-container-annexb`.
- Reuse the generic PES header parser from `cheetah-container-mpegts` to avoid duplicate PES parsing logic.

## Allowed dependencies

- `cheetah-media-types`
- `cheetah-media-bitstream`
- `cheetah-container-annexb`
- `cheetah-container-mpegts`

## Features

- `std` (default): enables `std`-dependent convenience constructors and `Error` impls.
- `default-features = false`: `no_std + alloc` build.
