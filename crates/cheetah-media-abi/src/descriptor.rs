//! Fixed-layout ABI descriptors for memory, packets and frames.

use crate::AbiVersion;

/// A writable or read-only region inside a WASM linear memory or native buffer.
///
/// The descriptor is intentionally plain-old-data so that JS can read fields
/// directly without calling back into Rust. `offset` is the byte offset from
/// the start of the memory region identified by `region`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryDescriptor {
    /// Region identifier (0 for the main WASM linear memory).
    pub region: u32,
    /// Byte offset from the start of `region`.
    pub offset: u64,
    /// Number of valid bytes in the region (may be smaller than `capacity`).
    pub length: u32,
    /// Total allocated bytes starting at `offset`.
    pub capacity: u32,
    /// Generation of the slot this descriptor belongs to; JS must pass it
    /// back unchanged so the engine can detect stale handles.
    pub generation: u64,
    /// Bit flags (e.g. read-only, keyframe, end-of-stream).
    pub flags: u32,
}

impl MemoryDescriptor {
    /// An empty descriptor with no region.
    pub const fn empty() -> Self {
        Self {
            region: 0,
            offset: 0,
            length: 0,
            capacity: 0,
            generation: 0,
            flags: 0,
        }
    }

    /// True if the descriptor references a non-empty region.
    pub const fn is_valid(self) -> bool {
        self.length > 0 && self.capacity > 0 && self.length <= self.capacity
    }

    /// True if no bytes are described.
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }

    /// Byte range described by this descriptor.
    pub const fn range(self) -> core::ops::Range<usize> {
        self.offset as usize..(self.offset as usize + self.length as usize)
    }
}

/// Compressed sample descriptor.
///
/// `payload` points to the encoded data. `side_data` may be empty.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketDescriptor {
    pub track_index: u32,
    pub payload: MemoryDescriptor,
    pub side_data: MemoryDescriptor,
    pub pts_ms: i64,
    pub dts_ms: i64,
    pub duration_ms: i64,
    pub flags: u32,
    pub epoch: u64,
}

// Static layout assertions for the C-facing descriptors. These are checked at
// compile time and also drive the TypeScript constant generator.
const _: () = assert!(core::mem::size_of::<MemoryDescriptor>() == 40);
const _: () = assert!(core::mem::align_of::<MemoryDescriptor>() == 8);
const _: () = assert!(core::mem::offset_of!(MemoryDescriptor, region) == 0);
const _: () = assert!(core::mem::offset_of!(MemoryDescriptor, offset) == 8);
const _: () = assert!(core::mem::offset_of!(MemoryDescriptor, length) == 16);
const _: () = assert!(core::mem::offset_of!(MemoryDescriptor, capacity) == 20);
const _: () = assert!(core::mem::offset_of!(MemoryDescriptor, generation) == 24);
const _: () = assert!(core::mem::offset_of!(MemoryDescriptor, flags) == 32);

const _: () = assert!(core::mem::size_of::<AbiVersion>() == 4);
const _: () = assert!(core::mem::align_of::<AbiVersion>() == 2);
const _: () = assert!(core::mem::offset_of!(AbiVersion, major) == 0);
const _: () = assert!(core::mem::offset_of!(AbiVersion, minor) == 2);

const _: () = assert!(core::mem::size_of::<PacketDescriptor>() == 128);
const _: () = assert!(core::mem::align_of::<PacketDescriptor>() == 8);
const _: () = assert!(core::mem::offset_of!(PacketDescriptor, track_index) == 0);
const _: () = assert!(core::mem::offset_of!(PacketDescriptor, payload) == 8);
const _: () = assert!(core::mem::offset_of!(PacketDescriptor, side_data) == 48);
const _: () = assert!(core::mem::offset_of!(PacketDescriptor, pts_ms) == 88);
const _: () = assert!(core::mem::offset_of!(PacketDescriptor, dts_ms) == 96);
const _: () = assert!(core::mem::offset_of!(PacketDescriptor, duration_ms) == 104);
const _: () = assert!(core::mem::offset_of!(PacketDescriptor, flags) == 112);
const _: () = assert!(core::mem::offset_of!(PacketDescriptor, epoch) == 120);

const _: () = assert!(core::mem::size_of::<FrameDescriptor>() == 288);
const _: () = assert!(core::mem::align_of::<FrameDescriptor>() == 8);
const _: () = assert!(core::mem::offset_of!(FrameDescriptor, track_index) == 0);
const _: () = assert!(core::mem::offset_of!(FrameDescriptor, payload) == 8);
const _: () = assert!(core::mem::offset_of!(FrameDescriptor, planes) == 48);
const _: () = assert!(core::mem::offset_of!(FrameDescriptor, side_data) == 208);
const _: () = assert!(core::mem::offset_of!(FrameDescriptor, width) == 248);
const _: () = assert!(core::mem::offset_of!(FrameDescriptor, height) == 252);
const _: () = assert!(core::mem::offset_of!(FrameDescriptor, pts_ms) == 256);
const _: () = assert!(core::mem::offset_of!(FrameDescriptor, duration_ms) == 264);
const _: () = assert!(core::mem::offset_of!(FrameDescriptor, flags) == 272);
const _: () = assert!(core::mem::offset_of!(FrameDescriptor, epoch) == 280);

/// Decoded frame descriptor.
///
/// Planes are ordered Y/U/V/A for video or interleaved for audio.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameDescriptor {
    pub track_index: u32,
    pub payload: MemoryDescriptor,
    pub planes: [MemoryDescriptor; 4],
    pub side_data: MemoryDescriptor,
    pub width: u32,
    pub height: u32,
    pub pts_ms: i64,
    pub duration_ms: i64,
    pub flags: u32,
    pub epoch: u64,
}
