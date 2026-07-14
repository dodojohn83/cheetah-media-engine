//! Bounded `BufferPool` reference implementation.

use alloc::sync::Arc;
use alloc::vec::Vec;
use bytes::Bytes;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;

use crate::MediaError;

use super::BufferRef;

/// Configuration for a bounded `BufferPool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferPoolConfig {
    /// Maximum total in-flight bytes.
    pub max_total_bytes: usize,
    /// Maximum number of live buffers.
    pub max_count: usize,
    /// Maximum size of a single buffer.
    pub max_object_size: usize,
    /// Maximum wait time in milliseconds before failing acquisition.
    pub max_wait_ms: u64,
    /// Maximum number of buffers kept in the free list. `None` means unbounded.
    pub max_free_count: Option<usize>,
}

impl Default for BufferPoolConfig {
    fn default() -> Self {
        Self {
            max_total_bytes: 64 * 1024 * 1024, // 64 MiB
            max_count: 256,
            max_object_size: 16 * 1024 * 1024, // 16 MiB
            max_wait_ms: 100,
            max_free_count: Some(64),
        }
    }
}

/// Snapshot of pool statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PoolStats {
    pub in_use_bytes: usize,
    pub in_use_count: usize,
    pub free_count: usize,
    pub total_acquired: usize,
    pub total_released: usize,
}

/// A pool of reusable `BufferRef` buffers.
pub trait BufferPool: Send + Sync {
    /// Acquire a buffer of at least `size` bytes.
    ///
    /// Returns `MediaError::ResourceLimit` if the pool is exhausted or the
    /// requested size exceeds the per-object limit.
    fn acquire(&self, size: usize) -> Result<BufferRef<'static>, MediaError>;

    /// Current pool statistics.
    fn stats(&self) -> PoolStats;
}

struct PoolInner {
    config: BufferPoolConfig,
    in_use_bytes: AtomicUsize,
    in_use_count: AtomicUsize,
    total_acquired: AtomicUsize,
    total_released: AtomicUsize,
    free: Mutex<Vec<Vec<u8>>>,
}

impl PoolInner {
    fn load_stats(&self) -> PoolStats {
        let free_count = self.free.lock().len();
        PoolStats {
            in_use_bytes: self.in_use_bytes.load(Ordering::Relaxed),
            in_use_count: self.in_use_count.load(Ordering::Relaxed),
            free_count,
            total_acquired: self.total_acquired.load(Ordering::Relaxed),
            total_released: self.total_released.load(Ordering::Relaxed),
        }
    }
}

struct PoolToken {
    data: Vec<u8>,
    size: usize,
    pool: Arc<PoolInner>,
}

impl AsRef<[u8]> for PoolToken {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl Drop for PoolToken {
    fn drop(&mut self) {
        self.pool.total_released.fetch_add(1, Ordering::Relaxed);
        self.pool.in_use_count.fetch_sub(1, Ordering::Relaxed);
        self.pool
            .in_use_bytes
            .fetch_sub(self.size, Ordering::Relaxed);

        let mut data = core::mem::take(&mut self.data);
        let mut free = self.pool.free.lock();
        let should_retain = self
            .pool
            .config
            .max_free_count
            .is_none_or(|max| free.len() < max);
        if should_retain {
            data.clear();
            free.push(data);
        }
    }
}

/// A simple bounded buffer pool backed by a free list of `Vec<u8>` chunks.
#[derive(Clone)]
pub struct SimpleBufferPool {
    inner: Arc<PoolInner>,
}

impl SimpleBufferPool {
    /// Create a pool with the given configuration.
    pub fn new(config: BufferPoolConfig) -> Self {
        Self {
            inner: Arc::new(PoolInner {
                config,
                in_use_bytes: AtomicUsize::new(0),
                in_use_count: AtomicUsize::new(0),
                total_acquired: AtomicUsize::new(0),
                total_released: AtomicUsize::new(0),
                free: Mutex::new(Vec::new()),
            }),
        }
    }
}

impl Default for SimpleBufferPool {
    fn default() -> Self {
        Self::new(BufferPoolConfig::default())
    }
}

impl BufferPool for SimpleBufferPool {
    fn acquire(&self, size: usize) -> Result<BufferRef<'static>, MediaError> {
        let cfg = &self.inner.config;

        if size > cfg.max_object_size {
            return Err(MediaError::ResourceLimit {
                name: "buffer_pool_object_size",
                current: size as u64,
                limit: cfg.max_object_size as u64,
            });
        }

        // Reserve the slot atomically to avoid TOCTOU races under concurrent
        // acquire calls.
        let new_count = self.inner.in_use_count.fetch_add(1, Ordering::Relaxed) + 1;
        if new_count > cfg.max_count {
            self.inner.in_use_count.fetch_sub(1, Ordering::Relaxed);
            return Err(MediaError::ResourceLimit {
                name: "buffer_pool_count",
                current: new_count as u64,
                limit: cfg.max_count as u64,
            });
        }

        let prev_bytes = self.inner.in_use_bytes.fetch_add(size, Ordering::Relaxed);
        let new_bytes = prev_bytes + size;
        if new_bytes > cfg.max_total_bytes || new_bytes < prev_bytes {
            // Overflow or over total-byte limit: release the reservation.
            self.inner.in_use_count.fetch_sub(1, Ordering::Relaxed);
            self.inner.in_use_bytes.fetch_sub(size, Ordering::Relaxed);
            return Err(MediaError::ResourceLimit {
                name: "buffer_pool_total_bytes",
                current: new_bytes as u64,
                limit: cfg.max_total_bytes as u64,
            });
        }

        let mut data = {
            let mut free = self.inner.free.lock();
            if let Some(pos) = free.iter().position(|v| v.capacity() >= size) {
                let mut chunk = free.swap_remove(pos);
                chunk.resize(size, 0);
                chunk
            } else {
                drop(free);
                vec![0u8; size]
            }
        };

        // Ensure exact length even if a larger chunk was reused.
        if data.len() != size {
            data.resize(size, 0);
        }

        let token = PoolToken {
            data,
            size,
            pool: self.inner.clone(),
        };
        let bytes = Bytes::from_owner(token);

        self.inner.total_acquired.fetch_add(1, Ordering::Relaxed);

        Ok(BufferRef::Shared(bytes))
    }

    fn stats(&self) -> PoolStats {
        self.inner.load_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_pool_recycles_and_tracks_stats() {
        let pool = SimpleBufferPool::new(BufferPoolConfig {
            max_total_bytes: 1024,
            max_count: 4,
            max_object_size: 512,
            max_wait_ms: 0,
            max_free_count: Some(2),
        });

        let b1 = pool.acquire(64).unwrap();
        assert_eq!(b1.len(), 64);
        let stats = pool.stats();
        assert_eq!(stats.in_use_count, 1);
        assert_eq!(stats.in_use_bytes, 64);

        drop(b1);
        let stats = pool.stats();
        assert_eq!(stats.in_use_count, 0);
        assert_eq!(stats.in_use_bytes, 0);
        assert_eq!(stats.total_acquired, 1);
        assert_eq!(stats.total_released, 1);
        assert_eq!(stats.free_count, 1);

        let b2 = pool.acquire(64).unwrap();
        assert_eq!(b2.len(), 64);
        assert_eq!(pool.stats().free_count, 0);
    }

    #[test]
    fn buffer_pool_enforces_limits() {
        let pool = SimpleBufferPool::new(BufferPoolConfig {
            max_total_bytes: 128,
            max_count: 2,
            max_object_size: 64,
            max_wait_ms: 0,
            max_free_count: None,
        });

        assert!(pool.acquire(128).is_err()); // exceeds max_object_size
        let _b1 = pool.acquire(64).unwrap();
        let _b2 = pool.acquire(64).unwrap();
        assert!(pool.acquire(1).is_err()); // exceeds max_count
    }
}
