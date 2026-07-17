//! JNI lifecycle hooks for the Android runtime.
//!
//! These are safe Rust helpers used by the JVM load/unload path. They do not
//! touch raw JNI pointers; the actual `JNI_OnLoad` / `JNI_OnUnload` exported
//! functions and `MediaCodec` JNI calls will be added once the Android NDK is
//! linked (WP-64). The counters let tests verify that create/destroy pairs are
//! balanced even on the host.

use cheetah_media_abi::AbiError;
use core::sync::atomic::{AtomicUsize, Ordering};

static INSTANCE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Increment the engine instance counter.
///
/// Returns `Ok(())` unless the instance limit is exceeded.
pub fn create() -> Result<(), AbiError> {
    let prev = INSTANCE_COUNT.fetch_add(1, Ordering::Relaxed);
    // A sanity limit; real Android code will use the JVM global reference
    // count rather than this counter.
    if prev >= usize::MAX - 1 {
        INSTANCE_COUNT.fetch_sub(1, Ordering::Relaxed);
        return Err(AbiError::OutOfBounds);
    }
    Ok(())
}

/// Decrement the engine instance counter.
pub fn destroy() {
    // Do not underflow if destroy is called without create.
    let _ = INSTANCE_COUNT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
        if n == 0 { Some(0) } else { Some(n - 1) }
    });
}

/// Return the number of active engine instances.
pub fn instance_count() -> usize {
    INSTANCE_COUNT.load(Ordering::Relaxed)
}

/// Called from the JVM when the shared library is loaded.
pub fn jni_on_load() -> bool {
    // On the host this is a no-op; on Android it will cache JNI class/method
    // IDs and return whether the required classes are available.
    true
}

/// Called from the JVM when the shared library is unloaded.
pub fn jni_on_unload() {
    // Release cached JNI global references.
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    // The instance counter is a process-wide static; serialize tests that touch
    // it so parallel test threads do not interfere with each other's expected
    // counts.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset_counter() {
        while instance_count() > 0 {
            destroy();
        }
    }

    #[test]
    fn create_and_destroy_are_balanced() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_counter();
        create().unwrap();
        create().unwrap();
        assert_eq!(instance_count(), 2);
        destroy();
        destroy();
        assert_eq!(instance_count(), 0);
    }

    #[test]
    fn destroy_without_create_does_not_underflow() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_counter();
        destroy();
        assert_eq!(instance_count(), 0);
    }

    #[test]
    fn jni_on_load_returns_true() {
        assert!(jni_on_load());
    }
}
