# cheetah-media-pipeline-core

Core media pipeline planner and scheduler.

## Responsibility

- `Pipeline` that wires a `Decoder` and optional `Renderer`.
- Provide feed/flush entry points.

## Constraints

- Forbids `unsafe_code`.
- Depends on `cheetah-media-abi`, `cheetah-media-timeline`, and container crates.
