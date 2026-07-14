//! Slot + generation handle used by the memory arena and control surface.

/// Opaque handle to a slot inside a `MemoryArena`.
///
/// Handles are cheap to copy but must not be shared across engine instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle {
    /// Engine instance that created this handle.
    pub instance_id: u64,
    /// Slot index inside the arena.
    pub slot: u32,
    /// Generation of the slot at creation time.
    pub generation: u64,
}
