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

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

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
