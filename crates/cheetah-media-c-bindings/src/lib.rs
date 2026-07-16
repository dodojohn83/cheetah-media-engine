//! Stable C ABI for the Cheetah media engine.
//!
//! This crate exposes a small, opaque C interface on top of the platform-
//! neutral `cheetah-media-engine` state machine. Unsafe code is restricted to
//! the audited FFI boundary.

use std::ffi::{CStr, c_char, c_int};
use std::ptr;

use cheetah_media_engine::Engine;

/// Result code returned by every C ABI function.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum CheetahResult {
    /// Operation completed successfully.
    Ok = 0,
    /// A required pointer argument was null.
    NullPtr = 1,
    /// The player is in an invalid state for this operation.
    InvalidState = 2,
    /// Input data was malformed or out of range.
    InvalidData = 3,
    /// The requested capability is not supported in this build.
    NotSupported = 4,
    /// An internal error occurred; see diagnostics for details.
    InternalError = 5,
}

impl CheetahResult {
    /// Convert the result to a stable integer code.
    #[must_use]
    pub const fn code(self) -> c_int {
        self as c_int
    }
}

/// Opaque player handle.
///
/// The internal layout is not exposed to C; callers only receive a pointer.
pub struct CheetahPlayer {
    /// Underlying platform-neutral engine state machine.
    _engine: Engine,
}

impl CheetahPlayer {
    /// Create a new player in the idle state.
    fn new() -> Self {
        Self {
            _engine: Engine::new(),
        }
    }
}

/// Return the engine version as a null-terminated UTF-8 string.
///
/// # Safety
///
/// The returned pointer is valid for the lifetime of the loaded library and
/// must not be freed by the caller.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cheetah_player_version() -> *const c_char {
    // SAFETY: `VERSION` is a static, null-terminated string literal.
    VERSION.as_ptr() as *const c_char
}

static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

/// Create a new player instance.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to a `*mut CheetahPlayer`. On
/// success the handle is written to `*player`. On failure `*player` is set to
/// `NULL` and a non-zero result is returned.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cheetah_player_create(player: *mut *mut CheetahPlayer) -> c_int {
    if player.is_null() {
        return CheetahResult::NullPtr.code();
    }
    let handle = Box::into_raw(Box::new(CheetahPlayer::new()));
    // SAFETY: `player` is non-null and points to writable memory.
    unsafe {
        *player = handle;
    }
    CheetahResult::Ok.code()
}

/// Destroy a player instance and release all associated resources.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to a handle returned by
/// `cheetah_player_create` or to `NULL`. After this call `*player` is set to
/// `NULL`. Calling twice with the same address is safe.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cheetah_player_destroy(player: *mut *mut CheetahPlayer) -> c_int {
    if player.is_null() {
        return CheetahResult::NullPtr.code();
    }
    // SAFETY: `player` is non-null; atomically swap the stored pointer to NULL
    // and read the previous value, then drop the handle.
    let handle = unsafe { ptr::replace(player, ptr::null_mut::<CheetahPlayer>()) };
    if handle.is_null() {
        return CheetahResult::Ok.code();
    }
    // SAFETY: `handle` was obtained from `Box::into_raw` and is not null.
    unsafe {
        let _ = Box::from_raw(handle);
    }
    CheetahResult::Ok.code()
}

/// Return the current player state as a null-terminated UTF-8 string.
///
/// # Safety
///
/// `player` must be either `NULL` or a valid handle returned by
/// `cheetah_player_create`. The returned pointer is valid until the next call
/// on the same handle that may mutate state and must not be freed.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cheetah_player_state(player: *const CheetahPlayer) -> *const c_char {
    // SAFETY: `player.as_ref()` only dereferences a pointer that the caller
    // claims is valid; a null pointer yields `None` and returns NULL.
    let Some(player) = (unsafe { player.as_ref() }) else {
        return ptr::null();
    };
    state_cstr(player._engine.state())
}

fn state_cstr(state: cheetah_media_engine::PlayerState) -> *const c_char {
    // SAFETY: All returned string literals are static and null-terminated.
    match state {
        cheetah_media_engine::PlayerState::Idle => c"idle".as_ptr(),
        cheetah_media_engine::PlayerState::Loading => c"loading".as_ptr(),
        cheetah_media_engine::PlayerState::Preroll => c"preroll".as_ptr(),
        cheetah_media_engine::PlayerState::Playing => c"playing".as_ptr(),
        cheetah_media_engine::PlayerState::Paused => c"paused".as_ptr(),
        cheetah_media_engine::PlayerState::Rebuffering => c"rebuffering".as_ptr(),
        cheetah_media_engine::PlayerState::Stopping => c"stopping".as_ptr(),
        cheetah_media_engine::PlayerState::Failed => c"failed".as_ptr(),
        cheetah_media_engine::PlayerState::Destroyed => c"destroyed".as_ptr(),
    }
}

/// Query the `NUL`-terminated length of `cheetah_player_version()` in bytes.
///
/// # Safety
///
/// This function is safe to call from any thread and returns a byte count that
/// excludes the terminating `NUL`.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cheetah_player_version_length() -> usize {
    // SAFETY: `VERSION` is a static null-terminated string.
    let cstr = unsafe { CStr::from_bytes_with_nul_unchecked(VERSION.as_bytes()) };
    cstr.to_bytes().len()
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]

    use super::*;

    #[test]
    fn version_is_static_and_null_terminated() {
        // SAFETY: `cheetah_player_version` returns a valid, null-terminated string.
        let ptr = unsafe { cheetah_player_version() };
        assert!(!ptr.is_null());
        // SAFETY: `cheetah_player_version` returns a valid, null-terminated string.
        let cstr = unsafe { CStr::from_ptr(ptr) };
        let s = cstr.to_str().expect("version is valid UTF-8");
        assert!(!s.is_empty());
        assert_eq!(s, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn create_and_destroy_round_trip() {
        let mut player: *mut CheetahPlayer = ptr::null_mut();
        // SAFETY: `player` is a valid pointer to a null handle.
        let res = unsafe { cheetah_player_create(&mut player) };
        assert_eq!(res, CheetahResult::Ok.code());
        assert!(!player.is_null());

        // SAFETY: `player` is a valid handle.
        let state = unsafe { cheetah_player_state(player) };
        assert!(!state.is_null());

        // SAFETY: `player` is a valid pointer to a handle.
        let res = unsafe { cheetah_player_destroy(&mut player) };
        assert_eq!(res, CheetahResult::Ok.code());
        assert!(player.is_null());
    }

    #[test]
    fn create_with_null_player_returns_null_ptr() {
        // SAFETY: Passing a null pointer is an explicit test input.
        let res = unsafe { cheetah_player_create(ptr::null_mut()) };
        assert_eq!(res, CheetahResult::NullPtr.code());
    }

    #[test]
    fn destroy_with_null_address_returns_null_ptr() {
        // SAFETY: Passing a null pointer is an explicit test input.
        let res = unsafe { cheetah_player_destroy(ptr::null_mut()) };
        assert_eq!(res, CheetahResult::NullPtr.code());
    }

    #[test]
    fn destroy_null_handle_is_ok() {
        let mut player: *mut CheetahPlayer = ptr::null_mut();
        // SAFETY: `player` points to a null handle.
        let res = unsafe { cheetah_player_destroy(&mut player) };
        assert_eq!(res, CheetahResult::Ok.code());
        assert!(player.is_null());
    }

    #[test]
    fn double_destroy_is_safe() {
        let mut player: *mut CheetahPlayer = ptr::null_mut();
        // SAFETY: `player` is valid and initially null.
        let res = unsafe { cheetah_player_create(&mut player) };
        assert_eq!(res, CheetahResult::Ok.code());
        // SAFETY: `player` holds a valid handle.
        let res = unsafe { cheetah_player_destroy(&mut player) };
        assert_eq!(res, CheetahResult::Ok.code());
        // SAFETY: `player` now holds a null handle; double destroy should be safe.
        let res = unsafe { cheetah_player_destroy(&mut player) };
        assert_eq!(res, CheetahResult::Ok.code());
        assert!(player.is_null());
    }

    #[test]
    fn state_after_destroy_is_null() {
        let mut player: *mut CheetahPlayer = ptr::null_mut();
        // SAFETY: `player` is valid.
        unsafe { cheetah_player_create(&mut player) };
        // SAFETY: `player` holds a valid handle.
        unsafe { cheetah_player_destroy(&mut player) };
        // SAFETY: `player` is now null; state must return NULL without crashing.
        let state = unsafe { cheetah_player_state(player) };
        assert!(state.is_null());
    }

    #[test]
    fn version_length_matches_strlen() {
        // SAFETY: `cheetah_player_version_length` is safe to call.
        let len = unsafe { cheetah_player_version_length() };
        // SAFETY: `cheetah_player_version` returns a valid, null-terminated string.
        let ptr = unsafe { cheetah_player_version() };
        // SAFETY: `cheetah_player_version` returns a valid, null-terminated string.
        let cstr = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(len, cstr.to_bytes().len());
    }
}
