//! Stable C ABI for the Cheetah media engine.
//!
//! This crate exposes a small, opaque C interface on top of the platform-
//! neutral `cheetah-media-engine` state machine. Unsafe code is restricted to
//! the audited FFI boundary.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr;

use cheetah_media_engine::{
    Engine, EngineCommand, EngineError, EngineEvent, LoadRequest, PlayerState,
};

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

/// Event delivered to a C event callback.
///
/// All pointer fields are valid only for the duration of the callback and must
/// not be freed or retained by the caller.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CheetahEvent {
    /// Event type string, e.g. `state_changed`, `error`, `eof`.
    pub event_type: *const c_char,
    /// Track identifier, if any.
    pub track_id: *const c_char,
    /// Human-readable payload or error context.
    pub message: *const c_char,
    /// Stable error code when `event_type` is `error`, otherwise `0`.
    pub error_code: u32,
}

/// C callback invoked synchronously when the engine produces events.
///
/// # Safety
///
/// Called on the thread that invoked the control function. The `event` pointer
/// and all of its string fields are valid only until the callback returns and
/// must not be retained or freed. The callback must not call back into the same
/// player from a different thread.
///
/// A `NULL` pointer disables callbacks.
pub type CheetahEventCallback = Option<
    unsafe extern "C" fn(
        player: *const CheetahPlayer,
        event: *const CheetahEvent,
        userdata: *mut c_void,
    ),
>;

/// Opaque player handle.
///
/// The internal layout is not exposed to C; callers only receive a pointer.
pub struct CheetahPlayer {
    /// Underlying platform-neutral engine state machine.
    engine: Engine,
    /// Optional event callback registered by the host.
    callback: CheetahEventCallback,
    /// Opaque host context passed to every callback invocation.
    userdata: *mut c_void,
    /// Most recent configuration JSON, if any.
    config: Option<String>,
}

impl CheetahPlayer {
    /// Create a new player in the idle state.
    fn new() -> Self {
        Self {
            engine: Engine::new(),
            callback: None,
            userdata: ptr::null_mut(),
            config: None,
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
    // SAFETY: `player` is non-null; replace the stored pointer with NULL in a
    // single operation, read the previous value, then drop the handle.
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
    state_cstr(player.engine.state())
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

/// Register or replace the event callback for a player.
///
/// # Safety
///
/// `player` must be a valid, non-null handle. `callback` may be `NULL` to
/// disable callbacks. `userdata` is passed unchanged to every callback and may
/// be `NULL`.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cheetah_player_set_event_callback(
    player: *mut CheetahPlayer,
    callback: CheetahEventCallback,
    userdata: *mut c_void,
) -> c_int {
    let Some(player) = (unsafe { player.as_mut() }) else {
        return CheetahResult::NullPtr.code();
    };
    if player.engine.state() == PlayerState::Destroyed {
        return CheetahResult::InvalidState.code();
    }
    player.callback = callback;
    player.userdata = userdata;
    CheetahResult::Ok.code()
}

/// Apply a JSON configuration string to the player.
///
/// # Safety
///
/// `player` must be a valid, non-null handle. `config` must be a valid
/// null-terminated UTF-8 string. The contents are stored and will be consumed
/// by later backend ports.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cheetah_player_configure(
    player: *mut CheetahPlayer,
    config: *const c_char,
) -> c_int {
    let Some(player) = (unsafe { player.as_mut() }) else {
        return CheetahResult::NullPtr.code();
    };
    if player.engine.state() == PlayerState::Destroyed {
        return CheetahResult::InvalidState.code();
    }
    if config.is_null() {
        return CheetahResult::NullPtr.code();
    }
    // SAFETY: `config` is claimed to be a valid, null-terminated C string.
    let cstr = unsafe { CStr::from_ptr(config) };
    let s = match cstr.to_str() {
        Ok(s) => s,
        Err(_) => return CheetahResult::InvalidData.code(),
    };
    player.config = Some(s.to_string());
    CheetahResult::Ok.code()
}

/// Load a media URL and prepare the engine for playback.
///
/// # Safety
///
/// `player` and `url` must be valid, non-null pointers. `url` must be a
/// null-terminated UTF-8 string containing a scheme (e.g. `http://`).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cheetah_player_load(
    player: *mut CheetahPlayer,
    url: *const c_char,
    is_live: bool,
) -> c_int {
    if player.is_null() {
        return CheetahResult::NullPtr.code();
    }
    if url.is_null() {
        return CheetahResult::NullPtr.code();
    }
    // SAFETY: `url` is claimed to be a valid, null-terminated C string.
    let cstr = unsafe { CStr::from_ptr(url) };
    let url = match cstr.to_str() {
        Ok(s) if !s.is_empty() && s.contains("://") => s,
        _ => return CheetahResult::InvalidData.code(),
    };

    let req = LoadRequest {
        url: url.to_string(),
        is_live,
    };

    // The exclusive borrow of *player ends when this block returns, so the
    // host callback can re-enter other C ABI functions on the same raw handle.
    let (callback, userdata, events) = {
        // SAFETY: `player` is valid and not null; engine state is checked below.
        let p = unsafe { &mut *player };
        if p.engine.state() == PlayerState::Destroyed {
            return CheetahResult::InvalidState.code();
        }
        let callback = p.callback;
        let userdata = p.userdata;
        let output = match p.engine.apply(EngineCommand::Load(req)) {
            Ok(out) => out,
            Err(_) => return CheetahResult::InvalidState.code(),
        };
        (callback, userdata, output.events)
    };

    let result = summarize_events(&events);
    // SAFETY: no mutable borrow of *player is live; only the raw pointer is passed
    // to the callback so re-entrant calls cannot alias an exclusive reference.
    unsafe { dispatch_events(player, events, callback, userdata) };
    result.code()
}

/// Begin playback.
///
/// # Safety
///
/// `player` must be a valid, non-null handle.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cheetah_player_play(player: *mut CheetahPlayer) -> c_int {
    // SAFETY: The caller must pass a valid player handle.
    unsafe { control_command(player, EngineCommand::Play) }
}

/// Pause playback.
///
/// # Safety
///
/// `player` must be a valid, non-null handle.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cheetah_player_pause(player: *mut CheetahPlayer) -> c_int {
    // SAFETY: The caller must pass a valid player handle.
    unsafe { control_command(player, EngineCommand::Pause) }
}

/// Stop and release the current session.
///
/// # Safety
///
/// `player` must be a valid, non-null handle.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cheetah_player_stop(player: *mut CheetahPlayer) -> c_int {
    // SAFETY: The caller must pass a valid player handle.
    unsafe { control_command(player, EngineCommand::Stop) }
}

/// Apply an engine command and dispatch the resulting events to the registered
/// callback, if any.
///
/// # Safety
///
/// `player` must be a valid, non-null handle.
#[allow(unsafe_code)]
unsafe fn control_command(player: *mut CheetahPlayer, command: EngineCommand) -> c_int {
    if player.is_null() {
        return CheetahResult::NullPtr.code();
    }

    let (callback, userdata, events) = {
        // SAFETY: `player` is valid and not null; engine state is checked below.
        let p = unsafe { &mut *player };
        if p.engine.state() == PlayerState::Destroyed {
            return CheetahResult::InvalidState.code();
        }
        let callback = p.callback;
        let userdata = p.userdata;
        let output = match p.engine.apply(command) {
            Ok(out) => out,
            Err(_) => return CheetahResult::InvalidState.code(),
        };
        (callback, userdata, output.events)
    };

    let result = summarize_events(&events);
    // SAFETY: no mutable borrow of *player is live; only the raw pointer is passed
    // to the callback so re-entrant calls cannot alias an exclusive reference.
    unsafe { dispatch_events(player, events, callback, userdata) };
    result.code()
}

/// Translate a sequence of engine events to a single C result code.
fn summarize_events(events: &[EngineEvent]) -> CheetahResult {
    for ev in events {
        if let EngineEvent::Error(
            EngineError::InvalidState { .. } | EngineError::InvalidCommand { .. },
        ) = ev
        {
            return CheetahResult::InvalidState;
        }
    }
    CheetahResult::Ok
}

/// Dispatch engine events through a C callback.
///
/// # Safety
///
/// `player` must be a valid pointer. `callback` must be a valid function
/// pointer. String fields inside each constructed `CheetahEvent` are valid
/// only for the duration of the callback call.
#[allow(unsafe_code)]
unsafe fn dispatch_events(
    player: *mut CheetahPlayer,
    events: Vec<EngineEvent>,
    callback: CheetahEventCallback,
    userdata: *mut c_void,
) {
    let Some(callback) = callback else {
        return;
    };
    for ev in events {
        let (event, _track, _message) = build_event(&ev);
        // SAFETY: `callback` is a valid function pointer supplied by the host.
        // `event` and its backing `CString`s live until the end of this scope.
        unsafe {
            callback(player as *const CheetahPlayer, &event, userdata);
        }
    }
}

/// Build a `CheetahEvent` from an `EngineEvent`, returning any owned `CString`s
/// that back the event's pointer fields. The caller must keep those `CString`s
/// alive while the `CheetahEvent` is in use.
fn build_event(ev: &EngineEvent) -> (CheetahEvent, Option<CString>, Option<CString>) {
    match ev {
        EngineEvent::StateChanged { from, to } => {
            let msg = CString::new(format!("{}, {}", state_name(*from), state_name(*to))).unwrap();
            let event = CheetahEvent {
                event_type: c"state_changed".as_ptr(),
                track_id: ptr::null(),
                message: msg.as_ptr(),
                error_code: 0,
            };
            (event, None, Some(msg))
        }
        EngineEvent::TrackAdded(info) => {
            let track = CString::new(info.id.get().to_string()).unwrap();
            let msg = CString::new(format!("{:?}:{:?}", info.kind, info.codec)).unwrap();
            let event = CheetahEvent {
                event_type: c"track_added".as_ptr(),
                track_id: track.as_ptr(),
                message: msg.as_ptr(),
                error_code: 0,
            };
            (event, Some(track), Some(msg))
        }
        EngineEvent::TrackConfigChanged {
            track_id,
            generation,
        } => {
            let track = CString::new(track_id.get().to_string()).unwrap();
            let msg = CString::new(format!("generation={}", generation)).unwrap();
            let event = CheetahEvent {
                event_type: c"track_config_changed".as_ptr(),
                track_id: track.as_ptr(),
                message: msg.as_ptr(),
                error_code: 0,
            };
            (event, Some(track), Some(msg))
        }
        EngineEvent::Discontinuity { epoch } => {
            let msg = CString::new(format!("epoch={}", epoch.get())).unwrap();
            let event = CheetahEvent {
                event_type: c"discontinuity".as_ptr(),
                track_id: ptr::null(),
                message: msg.as_ptr(),
                error_code: 0,
            };
            (event, None, Some(msg))
        }
        EngineEvent::Eof => (
            CheetahEvent {
                event_type: c"eof".as_ptr(),
                track_id: ptr::null(),
                message: ptr::null(),
                error_code: 0,
            },
            None,
            None,
        ),
        EngineEvent::Error(err) => {
            let msg = CString::new(error_message(err)).unwrap();
            let event = CheetahEvent {
                event_type: c"error".as_ptr(),
                track_id: ptr::null(),
                message: msg.as_ptr(),
                error_code: err.code(),
            };
            (event, None, Some(msg))
        }
        EngineEvent::RecoveryScheduled {
            action,
            code,
            delay_ms,
            attempts_left,
            ..
        } => {
            let msg = CString::new(format!(
                "{:?}:code={}:delay={}:left={}",
                action, code, delay_ms, attempts_left
            ))
            .unwrap();
            let event = CheetahEvent {
                event_type: c"recovery_scheduled".as_ptr(),
                track_id: ptr::null(),
                message: msg.as_ptr(),
                error_code: 0,
            };
            (event, None, Some(msg))
        }
        EngineEvent::ResourceWarning { kinds, total } => {
            let msg = CString::new(format!("{:?}:total={}", kinds, total)).unwrap();
            let event = CheetahEvent {
                event_type: c"resource_warning".as_ptr(),
                track_id: ptr::null(),
                message: msg.as_ptr(),
                error_code: 0,
            };
            (event, None, Some(msg))
        }
        EngineEvent::Stopped => (
            CheetahEvent {
                event_type: c"stopped".as_ptr(),
                track_id: ptr::null(),
                message: ptr::null(),
                error_code: 0,
            },
            None,
            None,
        ),
        EngineEvent::Destroyed => (
            CheetahEvent {
                event_type: c"destroyed".as_ptr(),
                track_id: ptr::null(),
                message: ptr::null(),
                error_code: 0,
            },
            None,
            None,
        ),
        EngineEvent::Metrics(_) => (
            CheetahEvent {
                event_type: c"metrics".as_ptr(),
                track_id: ptr::null(),
                message: ptr::null(),
                error_code: 0,
            },
            None,
            None,
        ),
    }
}

fn error_message(err: &EngineError) -> String {
    match err {
        EngineError::InvalidState { state, command } => {
            format!("InvalidState:{}:{}", state_name(*state), command)
        }
        EngineError::InvalidCommand { command } => format!("InvalidCommand:{}", command),
        EngineError::Backend { stage, code } => format!("Backend:{}:{}", stage, code),
        EngineError::ResourceLimit {
            name,
            current,
            limit,
        } => format!("ResourceLimit:{}:{}/{}", name, current, limit),
        EngineError::Destroyed => "Destroyed".to_string(),
    }
}

fn state_name(state: PlayerState) -> &'static str {
    match state {
        PlayerState::Idle => "idle",
        PlayerState::Loading => "loading",
        PlayerState::Preroll => "preroll",
        PlayerState::Playing => "playing",
        PlayerState::Paused => "paused",
        PlayerState::Rebuffering => "rebuffering",
        PlayerState::Stopping => "stopping",
        PlayerState::Failed => "failed",
        PlayerState::Destroyed => "destroyed",
    }
}

fn state_cstr(state: PlayerState) -> *const c_char {
    // SAFETY: All returned string literals are static and null-terminated.
    match state {
        PlayerState::Idle => c"idle".as_ptr(),
        PlayerState::Loading => c"loading".as_ptr(),
        PlayerState::Preroll => c"preroll".as_ptr(),
        PlayerState::Playing => c"playing".as_ptr(),
        PlayerState::Paused => c"paused".as_ptr(),
        PlayerState::Rebuffering => c"rebuffering".as_ptr(),
        PlayerState::Stopping => c"stopping".as_ptr(),
        PlayerState::Failed => c"failed".as_ptr(),
        PlayerState::Destroyed => c"destroyed".as_ptr(),
    }
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

    #[test]
    fn configure_null_is_null_ptr() {
        let mut player: *mut CheetahPlayer = ptr::null_mut();
        // SAFETY: `player` is a valid pointer to a null handle.
        unsafe { cheetah_player_create(&mut player) };
        // SAFETY: Passing null config is an explicit invalid input test.
        let res = unsafe { cheetah_player_configure(player, ptr::null()) };
        assert_eq!(res, CheetahResult::NullPtr.code());
        // SAFETY: `player` is valid.
        unsafe { cheetah_player_destroy(&mut player) };
    }

    #[test]
    fn configure_and_query_state() {
        let mut player: *mut CheetahPlayer = ptr::null_mut();
        // SAFETY: `player` is a valid pointer to a null handle.
        unsafe { cheetah_player_create(&mut player) };

        let config = c"{}";
        // SAFETY: `config` is a valid null-terminated string.
        let res = unsafe { cheetah_player_configure(player, config.as_ptr()) };
        assert_eq!(res, CheetahResult::Ok.code());

        // SAFETY: `player` is valid.
        unsafe { cheetah_player_destroy(&mut player) };
    }

    #[test]
    fn load_invalid_url_returns_invalid_data() {
        let mut player: *mut CheetahPlayer = ptr::null_mut();
        // SAFETY: `player` is a valid pointer to a null handle.
        unsafe { cheetah_player_create(&mut player) };

        let url = c"not-a-url";
        // SAFETY: `url` is a valid null-terminated string.
        let res = unsafe { cheetah_player_load(player, url.as_ptr(), false) };
        assert_eq!(res, CheetahResult::InvalidData.code());

        // SAFETY: `player` is valid.
        unsafe { cheetah_player_destroy(&mut player) };
    }

    #[test]
    fn play_before_load_returns_invalid_state() {
        let mut player: *mut CheetahPlayer = ptr::null_mut();
        // SAFETY: `player` is a valid pointer to a null handle.
        unsafe { cheetah_player_create(&mut player) };

        // SAFETY: `player` is valid.
        let res = unsafe { cheetah_player_play(player) };
        assert_eq!(res, CheetahResult::InvalidState.code());

        // SAFETY: `player` is valid.
        unsafe { cheetah_player_destroy(&mut player) };
    }

    #[test]
    fn events_are_delivered_through_callback() {
        use cheetah_media_engine::BackendEvent;
        use std::sync::atomic::{AtomicUsize, Ordering};

        static COUNT: AtomicUsize = AtomicUsize::new(0);

        extern "C" fn on_event(
            _player: *const CheetahPlayer,
            event: *const CheetahEvent,
            _userdata: *mut c_void,
        ) {
            // SAFETY: The test driver guarantees a valid event pointer.
            let ev = unsafe { &*event };
            // SAFETY: `event_type` is a static null-terminated string.
            let ty = unsafe { CStr::from_ptr(ev.event_type) };
            let ty = ty.to_str().unwrap();
            if ty == "state_changed" {
                COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }

        let mut player: *mut CheetahPlayer = ptr::null_mut();
        // SAFETY: `player` is a valid pointer to a null handle.
        unsafe { cheetah_player_create(&mut player) };

        // SAFETY: `player` is valid; `on_event` is a valid extern "C" function.
        let res =
            unsafe { cheetah_player_set_event_callback(player, Some(on_event), ptr::null_mut()) };
        assert_eq!(res, CheetahResult::Ok.code());

        let url = c"http://example.com/test.flv";
        // SAFETY: `url` is a valid null-terminated string.
        let res = unsafe { cheetah_player_load(player, url.as_ptr(), false) };
        assert_eq!(res, CheetahResult::Ok.code());

        // Drive the engine to preroll by injecting a synthetic track and config.
        {
            // SAFETY: `player` is valid and not destroyed.
            let p = unsafe { &mut *player };
            use cheetah_media_types::{CodecId, TimeBase, TrackId, TrackInfo, TrackKind};
            let _ = p.engine.apply(EngineCommand::Backend(BackendEvent::Track {
                epoch: p.engine.epoch(),
                info: TrackInfo::new(
                    TrackId::new(7).unwrap(),
                    TrackKind::Video,
                    CodecId::H264,
                    TimeBase::DEFAULT,
                ),
            }));
            let _ = p
                .engine
                .apply(EngineCommand::Backend(BackendEvent::ConfigChanged {
                    epoch: p.engine.epoch(),
                    track_id: TrackId::new(7).unwrap(),
                    generation: 1,
                }));
            assert_eq!(p.engine.state(), PlayerState::Preroll);
        }

        // SAFETY: `player` is valid.
        let res = unsafe { cheetah_player_play(player) };
        assert_eq!(res, CheetahResult::Ok.code());

        // load produced a state_changed; play produced another.
        let count = COUNT.load(Ordering::SeqCst);
        assert_eq!(count, 2, "expected 2 state_changed events, got {count}");

        // SAFETY: `player` is valid.
        let state = unsafe { cheetah_player_state(player) };
        // SAFETY: `cheetah_player_state` returns a valid null-terminated string.
        let state = unsafe { CStr::from_ptr(state) }.to_str().unwrap();
        assert_eq!(state, "playing");

        // SAFETY: `player` is valid.
        unsafe { cheetah_player_destroy(&mut player) };
    }
}
