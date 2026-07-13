# cheetah-media-engine

Engine orchestration, state machine, and pipeline planner for Cheetah Media Engine Web v1.

## Responsibility

- Player lifecycle state machine.
- Backend selection based on `CapabilityProbe` results.
- Resource budgets and pipeline planning.

## Constraints

- Depends only on `cheetah-media-types` and `cheetah-media-backend-api`.
- Forbids `unsafe_code`.
- Does not directly call DOM, WebCodecs, MSE, or other platform APIs.
