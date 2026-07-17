# cheetah-media-c-bindings

Stable C ABI bindings for the Cheetah media engine.

## Scope

This crate exposes an opaque `CheetahPlayer` handle and a small control surface
that Qt, Android, iOS and other native hosts can call through a stable
C interface. It is a thin FFI layer over the platform-neutral
`cheetah-media-engine` state machine.

## Allowed dependencies

- `cheetah-media-engine` (platform-neutral engine state machine)
- `std` for `CString`, `CStr` and `Box` allocation at the FFI boundary

## Forbidden dependencies

- No Qt, GTK, Cocoa, Android or platform UI types.
- No transport, decoder or renderer implementation.
- No `serde`/`serde_json` across the C ABI boundary (use typed descriptors only).

## Crate type

`cdylib`, `staticlib` and `rlib` so that:

- C/C++ hosts can link the static or dynamic library.
- Rust integration tests can call the public API as `rlib`.

## Public entry points

See `include/cheetah_media.h` for the generated C header.

Current functions:

- `cheetah_player_version()` -> `*const c_char`
- `cheetah_player_create(*mut *mut CheetahPlayer) -> c_int`
- `cheetah_player_destroy(*mut *mut CheetahPlayer) -> c_int`
- `cheetah_player_state(*const CheetahPlayer) -> *const c_char`

## Safety

All `unsafe` blocks are restricted to the FFI boundary and documented with
`SAFETY` comments. Panics are not propagated across the ABI boundary: all
functions return stable result codes.

## Feature flags

- `std` (default): enables `std` dependencies in `cheetah-media-engine`.
