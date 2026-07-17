#ifndef CHEETAH_MEDIA_H
#define CHEETAH_MEDIA_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Result code returned by every C ABI function.
 */
typedef enum {
  /**
   * Operation completed successfully.
   */
  CHEETAH_RESULT_OK = 0,
  /**
   * A required pointer argument was null.
   */
  CHEETAH_RESULT_NULL_PTR = 1,
  /**
   * The player is in an invalid state for this operation.
   */
  CHEETAH_RESULT_INVALID_STATE = 2,
  /**
   * Input data was malformed or out of range.
   */
  CHEETAH_RESULT_INVALID_DATA = 3,
  /**
   * The requested capability is not supported in this build.
   */
  CHEETAH_RESULT_NOT_SUPPORTED = 4,
  /**
   * An internal error occurred; see diagnostics for details.
   */
  CHEETAH_RESULT_INTERNAL_ERROR = 5,
} CheetahResult;

/**
 * Opaque player handle.
 *
 * The internal layout is not exposed to C; callers only receive a pointer.
 */
typedef struct CheetahPlayer CheetahPlayer;

/**
 * Event delivered to a C event callback.
 *
 * All pointer fields are valid only for the duration of the callback and must
 * not be freed or retained by the caller.
 */
typedef struct {
  /**
   * Event type string, e.g. `state_changed`, `error`, `eof`.
   */
  const char *event_type;
  /**
   * Track identifier, if any.
   */
  const char *track_id;
  /**
   * Human-readable payload or error context.
   */
  const char *message;
  /**
   * Stable error code when `event_type` is `error`, otherwise `0`.
   */
  uint32_t error_code;
} CheetahEvent;

/**
 * C callback invoked synchronously when the engine produces events.
 *
 * # Safety
 *
 * Called on the thread that invoked the control function. The `event` pointer
 * and all of its string fields are valid only until the callback returns and
 * must not be retained or freed. The callback must not call back into the same
 * player from a different thread.
 *
 * A `NULL` pointer disables callbacks.
 */
typedef void (*CheetahEventCallback)(const CheetahPlayer *player,
                                     const CheetahEvent *event,
                                     void *userdata);

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Apply a JSON configuration string to the player.
 *
 * # Safety
 *
 * `player` must be a valid, non-null handle. `config` must be a valid
 * null-terminated UTF-8 string. The contents are stored and will be consumed
 * by later backend ports.
 */
int cheetah_player_configure(CheetahPlayer *player, const char *config);

/**
 * Create a new player instance.
 *
 * # Safety
 *
 * `player` must be a valid, non-null pointer to a `*mut CheetahPlayer`. On
 * success the handle is written to `*player`. On failure `*player` is set to
 * `NULL` and a non-zero result is returned.
 */
int cheetah_player_create(CheetahPlayer **player);

/**
 * Destroy a player instance and release all associated resources.
 *
 * # Safety
 *
 * `player` must be a valid, non-null pointer to a handle returned by
 * `cheetah_player_create` or to `NULL`. After this call `*player` is set to
 * `NULL`. Calling twice with the same address is safe.
 */
int cheetah_player_destroy(CheetahPlayer **player);

/**
 * Load a media URL and prepare the engine for playback.
 *
 * # Safety
 *
 * `player` and `url` must be valid, non-null pointers. `url` must be a
 * null-terminated UTF-8 string containing a scheme (e.g. `http://`).
 */
int cheetah_player_load(CheetahPlayer *player, const char *url, bool is_live);

/**
 * Pause playback.
 *
 * # Safety
 *
 * `player` must be a valid, non-null handle.
 */
int cheetah_player_pause(CheetahPlayer *player);

/**
 * Begin playback.
 *
 * # Safety
 *
 * `player` must be a valid, non-null handle.
 */
int cheetah_player_play(CheetahPlayer *player);

/**
 * Register or replace the event callback for a player.
 *
 * # Safety
 *
 * `player` must be a valid, non-null handle. `callback` may be `NULL` to
 * disable callbacks. `userdata` is passed unchanged to every callback and may
 * be `NULL`.
 */
int cheetah_player_set_event_callback(CheetahPlayer *player,
                                      CheetahEventCallback callback,
                                      void *userdata);

/**
 * Return the current player state as a null-terminated UTF-8 string.
 *
 * # Safety
 *
 * `player` must be either `NULL` or a valid handle returned by
 * `cheetah_player_create`. The returned pointer is valid until the next call
 * on the same handle that may mutate state and must not be freed.
 */
const char *cheetah_player_state(const CheetahPlayer *player);

/**
 * Stop and release the current session.
 *
 * # Safety
 *
 * `player` must be a valid, non-null handle.
 */
int cheetah_player_stop(CheetahPlayer *player);

/**
 * Return the engine version as a null-terminated UTF-8 string.
 *
 * # Safety
 *
 * The returned pointer is valid for the lifetime of the loaded library and
 * must not be freed by the caller.
 */
const char *cheetah_player_version(void);

/**
 * Query the `NUL`-terminated length of `cheetah_player_version()` in bytes.
 *
 * # Safety
 *
 * This function is safe to call from any thread and returns a byte count that
 * excludes the terminating `NUL`.
 */
uintptr_t cheetah_player_version_length(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* CHEETAH_MEDIA_H */
