# cheetah-container-mpegts

MPEG-TS transport stream parser.

## Responsibility

- Parse 188-byte TS packet header.
- Extract PID, payload-unit-start indicator, adaptation-field control, and continuity counter.

## Constraints

- Forbids `unsafe_code`.
- Depends on `cheetah-media-bitstream` and `cheetah-media-types`.
