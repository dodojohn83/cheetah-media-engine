# cheetah-container-annexb

Annex-B byte-stream parser for H.264 and H.265.

## Scope

- Split an Annex-B byte stream into NAL units, handling 3-byte and 4-byte start codes and H.264/H.265 emulation prevention bytes.
- Cache H.264 SPS/PPS and emit an `AvcC` decoder configuration record.
- Cache H.265 VPS/SPS/PPS and emit an `HvcC` (`HevcC`) decoder configuration record.
- Emit `MediaPacket` events for each NAL unit with key-frame flags (H.264 IDR, H.265 IRAP/IDR).

## Allowed dependencies

- `cheetah-media-types`
- `cheetah-media-bitstream`
- Dev-only test helpers (`proptest`)

## Features

- `std` (default): enables `std`-dependent convenience constructors and `Error` impls.
- `default-features = false`: `no_std + alloc` build.
