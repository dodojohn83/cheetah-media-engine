# cheetah-media-timeline

Media timeline and synchronization primitives.

## Responsibility

- Maintain an ordered `Timeline` of `MediaTime` entries.
- Provide `next_after_ms` for playhead positioning.

## Constraints

- Forbids `unsafe_code`.
- Depends only on `cheetah-media-types`.
