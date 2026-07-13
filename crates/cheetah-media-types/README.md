# cheetah-media-types

Shared media types used by the Cheetah media engine.

## Responsibility

- Codec identifiers (`CodecId`).
- Track kind (`TrackKind`).
- Media timestamps (`MediaTime`) and timescale helpers.

## Constraints

- `no_std` compatible when `std` feature is disabled.
- Forbids `unsafe_code`.
- Does not depend on parser, container, or network crates.
