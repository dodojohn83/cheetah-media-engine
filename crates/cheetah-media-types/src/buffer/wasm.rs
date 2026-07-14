//! WASM linear-memory region descriptor.

/// A WASM linear-memory region descriptor.
///
/// JS must not hold a permanent `TypedArray` over this region because the
/// WebAssembly memory may grow and invalidate the view. Instead the descriptor
/// carries a `generation` that the runtime checks before mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LinearMemoryRef {
    /// Byte offset into the WASM memory.
    pub offset: usize,
    /// Byte length of the region.
    pub length: usize,
    /// Generation of the WASM memory at the time of creation.
    pub generation: u64,
    /// Identifier for the memory instance (default is 0).
    pub memory_id: u64,
}

impl LinearMemoryRef {
    /// True if `offset + length` does not overflow.
    pub const fn is_valid(self) -> bool {
        self.offset.checked_add(self.length).is_some()
    }

    /// True if this descriptor is empty.
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_memory_ref_validates_overflow() {
        let mut desc = LinearMemoryRef {
            offset: usize::MAX - 10,
            length: 20,
            generation: 1,
            memory_id: 0,
        };
        assert!(!desc.is_valid());
        desc.length = 5;
        assert!(desc.is_valid());
    }
}
