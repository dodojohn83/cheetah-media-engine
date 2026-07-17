# cheetah-media-bitstream

Byte and bit-level reader for media container parsers.

## Responsibility

- `ByteCursor` for big-endian integer reads with bounds checking.
- No `std` required; works with `alloc` only.

## Constraints

- Forbids `unsafe_code`.
- Does not depend on `std`, async, HTTP, or logging.
