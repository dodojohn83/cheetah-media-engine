# cheetah-container-isobmff

ISOBMFF / MP4 / fMP4 box parser.

## Responsibility

- Parse box header and expose `BoxHeader`.
- Support 32-bit and 64-bit box sizes.

## Constraints

- Forbids `unsafe_code`.
- Depends on `cheetah-media-bitstream` and `cheetah-media-types`.
