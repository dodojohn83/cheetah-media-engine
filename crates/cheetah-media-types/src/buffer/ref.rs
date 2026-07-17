//! `BufferRef` — a cheaply cloneable, sliceable media buffer.

use alloc::borrow::Cow;
use alloc::vec::Vec;
use bytes::Bytes;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::Range;

/// Lifetime classification for a media buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BufferLifecycle {
    /// A transient borrow with a concrete Rust lifetime.
    Borrowed,
    /// An immutable, reference-counted buffer that outlives borrows.
    Shared,
    /// An external GPU/decoder frame resource identified by a handle.
    External,
}

/// A cheaply cloneable, sliceable media buffer.
///
/// * `Borrowed` holds a slice reference and carries no allocation cost.
/// * `Shared` wraps a `Bytes` region. Clones are O(1) and slicing is O(1).
#[derive(Clone, Default)]
pub enum BufferRef<'a> {
    /// Empty buffer.
    #[default]
    Empty,
    /// Borrowed slice with lifetime `'a`.
    Borrowed(&'a [u8]),
    /// Reference-counted, `'static` byte region.
    Shared(Bytes),
}

impl<'a> BufferRef<'a> {
    /// Create an empty `BufferRef`.
    pub const fn empty() -> Self {
        Self::Empty
    }

    /// Create a `BufferRef` from a borrowed slice.
    pub const fn from_borrowed(data: &'a [u8]) -> Self {
        if data.is_empty() {
            Self::Empty
        } else {
            Self::Borrowed(data)
        }
    }

    /// Create a `BufferRef` from an owned `Vec<u8>`.
    pub fn from_owned(data: Vec<u8>) -> Self {
        if data.is_empty() {
            Self::Empty
        } else {
            Self::Shared(Bytes::from(data))
        }
    }

    /// Create a `BufferRef` from a `Bytes` region.
    pub fn from_bytes(data: Bytes) -> Self {
        if data.is_empty() {
            Self::Empty
        } else {
            Self::Shared(data)
        }
    }

    /// Return the buffer length.
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }

    /// True if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Slice the buffer without copying.
    ///
    /// Panics if `range` is out of bounds.
    pub fn slice(&self, range: Range<usize>) -> Self {
        let data = self.as_ref();
        assert!(range.end <= data.len(), "BufferRef slice out of bounds");
        if range.is_empty() {
            return Self::Empty;
        }
        match self {
            Self::Empty => Self::Empty,
            Self::Borrowed(b) => Self::Borrowed(&b[range]),
            Self::Shared(b) => Self::Shared(b.slice(range)),
        }
    }

    /// Convert into an owned `Vec<u8>`. This is the only method that copies.
    pub fn to_vec(&self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    /// Lifetime classification of this buffer.
    pub const fn lifecycle(&self) -> BufferLifecycle {
        match self {
            Self::Empty | Self::Shared(_) => BufferLifecycle::Shared,
            Self::Borrowed(_) => BufferLifecycle::Borrowed,
        }
    }

    /// True if this buffer is reference-counted and cheap to clone.
    pub const fn is_shared(&self) -> bool {
        matches!(self, Self::Shared(_))
    }

    /// True if this buffer borrows memory with a Rust lifetime.
    pub const fn is_borrowed(&self) -> bool {
        matches!(self, Self::Borrowed(_))
    }

    /// Promote a borrowed buffer to a `'static` shared buffer by copying.
    ///
    /// If the buffer is already shared, this is a no-op clone.
    pub fn to_static(&self) -> BufferRef<'static> {
        match self {
            Self::Borrowed(b) => BufferRef::from_owned(b.to_vec()),
            Self::Shared(b) => BufferRef::Shared(b.clone()),
            Self::Empty => BufferRef::Empty,
        }
    }
}

impl<'a> AsRef<[u8]> for BufferRef<'a> {
    fn as_ref(&self) -> &[u8] {
        match self {
            BufferRef::Empty => &[],
            BufferRef::Borrowed(b) => b,
            BufferRef::Shared(b) => b.as_ref(),
        }
    }
}

impl<'a> PartialEq for BufferRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl<'a> Eq for BufferRef<'a> {}

impl<'a> Hash for BufferRef<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl<'a> fmt::Debug for BufferRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufferRef")
            .field("len", &self.len())
            .field("lifecycle", &self.lifecycle())
            .finish()
    }
}

impl<'a> From<&'a [u8]> for BufferRef<'a> {
    fn from(data: &'a [u8]) -> Self {
        Self::from_borrowed(data)
    }
}

impl From<Vec<u8>> for BufferRef<'static> {
    fn from(data: Vec<u8>) -> Self {
        Self::from_owned(data)
    }
}

impl From<Bytes> for BufferRef<'static> {
    fn from(data: Bytes) -> Self {
        Self::from_bytes(data)
    }
}

impl<'a> From<Cow<'a, [u8]>> for BufferRef<'a> {
    fn from(cow: Cow<'a, [u8]>) -> Self {
        match cow {
            Cow::Borrowed(b) => Self::from_borrowed(b),
            Cow::Owned(o) => Self::from_owned(o),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_buffer_ref_slices_without_copy() {
        let data = [0u8, 1, 2, 3, 4];
        let buf = BufferRef::from_borrowed(&data);
        let sliced = buf.slice(1..4);
        assert_eq!(sliced.as_ref(), &[1, 2, 3]);
        assert!(sliced.is_borrowed());
    }

    #[test]
    fn shared_buffer_ref_slices_and_clones_cheaply() {
        let buf = BufferRef::from_owned(vec![0u8, 1, 2, 3, 4]);
        let cloned = buf.clone();
        let sliced = buf.slice(1..4);
        assert_eq!(sliced.as_ref(), &[1, 2, 3]);
        assert!(buf.is_shared());
        assert!(cloned.is_shared());
        assert!(sliced.is_shared());
    }

    #[test]
    fn empty_slice_is_empty() {
        let buf = BufferRef::from_owned(vec![1, 2, 3]);
        let empty = buf.slice(1..1);
        assert!(empty.is_empty());
    }

    #[test]
    fn buffer_ref_from_cow() {
        let owned: Cow<'static, [u8]> = Cow::Owned(vec![1, 2, 3]);
        let borrowed: Cow<'static, [u8]> = Cow::Borrowed(&[1, 2, 3]);
        let b1: BufferRef = owned.into();
        let b2: BufferRef = borrowed.into();
        assert!(b1.is_shared());
        assert!(b2.is_borrowed());
        assert_eq!(b1, b2);
    }
}
