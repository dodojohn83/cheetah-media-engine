# cheetah-container-annexb

Annex-B byte-stream parser for H.264 (and, in a later work package, H.265).

## Scope

- Split an Annex-B byte stream into NAL units, handling 3-byte and 4-byte start codes and H.264 emulation prevention bytes.
- Cache SPS/PPS parameter sets and emit an `AvcC` decoder configuration record.
- Emit `MediaPacket` events for each NAL unit with key-frame flags.

## Allowed dependencies

- `cheetah-media-types`
- `cheetah-media-bitstream`
- Dev-only test helpers (`proptest`)

## Features

- `std` (default): enables `std`-dependent convenience constructors and `Error` impls.
- `default-features = false`: `no_std + alloc` build.
