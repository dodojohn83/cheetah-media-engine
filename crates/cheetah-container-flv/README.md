# cheetah-container-flv

FLV container format parser and writer.

## Responsibility

- Parse FLV file header and tag header.
- Map tag types to audio/video track kinds.
- Provide no_std-compatible entry points.

## Constraints

- Forbids `unsafe_code`.
- Depends on `cheetah-media-bitstream` and `cheetah-media-types`.
