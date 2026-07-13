# cheetah-media-core

Facade crate for the shared Cheetah media core.

## Responsibility

- Re-export all core public crates under a stable facade.
- Provide `VERSION` constant.

## Constraints

- Forbids `unsafe_code`.
- Re-exports only; does not add its own logic or duplicate dependencies.
