//! Buffer ownership, pooling, copy-budget, and WASM linear-memory descriptors.
//!
//! `BufferRef` is the native shared-buffer primitive: it is either a borrowed
//! slice or a reference-counted `Bytes` region. Slicing never copies the
//! underlying data. Pooled buffers are returned to their pool on drop, making
//! the total in-flight memory bounded.

pub mod budget;
pub mod pool;
pub mod r#ref;
pub mod wasm;

pub use budget::{CopyBudget, CopyCounter, CopyReason, DropPolicy, StageBudget};
pub use pool::{BufferPool, BufferPoolConfig, PoolStats, SimpleBufferPool};
pub use r#ref::{BufferLifecycle, BufferRef};
pub use wasm::LinearMemoryRef;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_are_consistent() {
        let _ = BufferRef::empty();
        let _ = CopyBudget::new(None);
        let _ = LinearMemoryRef::default();
    }
}
