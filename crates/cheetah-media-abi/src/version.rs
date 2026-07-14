//! Stable ABI versioning.

/// Major/minor ABI version.
///
/// Major changes break compatibility; minor changes are backward-compatible
/// additions. Callers must only use an ABI whose `major` matches and whose
/// `minor` is less than or equal to the provider's current `minor`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbiVersion {
    pub major: u16,
    pub minor: u16,
}

impl AbiVersion {
    /// Current ABI version used by this build.
    pub const CURRENT: Self = Self { major: 0, minor: 1 };

    /// Create a version. Useful for tests and manifest construction.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// True if this provider version can satisfy a caller that speaks `caller`.
    pub const fn supports(self, caller: Self) -> bool {
        self.major == caller.major && caller.minor <= self.minor
    }

    /// Pack into a `u32` for cheap comparison and logging.
    pub const fn to_u32(self) -> u32 {
        ((self.major as u32) << 16) | (self.minor as u32)
    }
}
