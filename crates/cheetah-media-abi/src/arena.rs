//! Bounded slot-based memory arena for descriptor-based data exchange.

use alloc::vec::Vec;

use crate::{AbiError, Handle, MemoryDescriptor};

/// Owned byte region tracked by the arena.
struct MemoryRegion {
    data: Vec<u8>,
    committed: usize,
}

impl MemoryRegion {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
            committed: size,
        }
    }

    fn commit(&mut self, len: usize) {
        if len < self.data.len() {
            self.data.truncate(len);
        }
        self.committed = self.data.len();
    }

    fn as_descriptor(&self, generation: u64) -> MemoryDescriptor {
        MemoryDescriptor {
            region: 0,
            offset: self.data.as_ptr() as usize as u64,
            length: self.committed as u32,
            capacity: self.data.capacity() as u32,
            generation,
            flags: 0,
        }
    }
}

enum Slot {
    Free {
        generation: u64,
    },
    Occupied {
        generation: u64,
        region: MemoryRegion,
    },
}

/// Slot-map arena that lends out writable regions and validates handles by
/// generation and instance id.
///
/// The arena is intentionally synchronous: callers that need async allocation
/// can layer it on top of a budgeted queue later.
pub struct MemoryArena {
    instance_id: u64,
    slots: Vec<Slot>,
    free: Vec<usize>,
    next_generation: u64,
}

impl MemoryArena {
    /// Create an arena belonging to `instance_id`.
    pub fn new(instance_id: u64) -> Self {
        Self {
            instance_id,
            slots: Vec::new(),
            free: Vec::new(),
            next_generation: 1,
        }
    }

    /// The instance id that owns handles created by this arena.
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    /// Allocate a writable region of `size` bytes.
    ///
    /// Returns a handle and a `MemoryDescriptor` that JS can use to build a
    /// typed-array view into WASM linear memory.
    pub fn request(&mut self, size: usize) -> Result<(Handle, MemoryDescriptor), AbiError> {
        if size == 0 {
            return Err(AbiError::InvalidData);
        }
        // Keep the allocation representable in a `MemoryDescriptor` and below the
        // maximum addressable slice length on the target platform. We stay one
        // below `u32::MAX` so that a rounded-up Vec capacity still fits in the
        // descriptor's `capacity: u32` field.
        const MAX_SIZE: usize = if (isize::MAX as usize) < ((u32::MAX - 1) as usize) {
            isize::MAX as usize
        } else {
            (u32::MAX - 1) as usize
        };
        if size > MAX_SIZE {
            return Err(AbiError::OutOfBounds);
        }

        let generation = self.next_generation;
        self.next_generation += 1;

        let region = MemoryRegion::new(size);
        let descriptor = region.as_descriptor(generation);

        let slot = if let Some(index) = self.free.pop() {
            self.slots[index] = Slot::Occupied { generation, region };
            index as u32
        } else {
            let index = self.slots.len();
            if index > u32::MAX as usize {
                return Err(AbiError::OutOfBounds);
            }
            self.slots.push(Slot::Occupied { generation, region });
            index as u32
        };

        let handle = Handle {
            instance_id: self.instance_id,
            slot,
            generation,
        };

        Ok((handle, descriptor))
    }

    /// Write `data` into a previously requested region.
    ///
    /// The region must be large enough to hold `data`; its committed length is
    /// not changed until `commit` is called.
    pub fn write(&mut self, handle: Handle, data: &[u8]) -> Result<(), AbiError> {
        let index = self.validate_occupied(handle)?;
        if let Slot::Occupied { region, .. } = &mut self.slots[index] {
            if data.len() > region.data.len() {
                return Err(AbiError::OutOfBounds);
            }
            region.data[..data.len()].copy_from_slice(data);
            Ok(())
        } else {
            Err(AbiError::StaleHandle)
        }
    }

    /// Commit the first `len` bytes of a previously requested region.
    pub fn commit(&mut self, handle: Handle, len: usize) -> Result<(), AbiError> {
        let index = self.validate_occupied(handle)?;
        if let Slot::Occupied { region, .. } = &mut self.slots[index] {
            if len > region.data.len() {
                return Err(AbiError::OutOfBounds);
            }
            region.commit(len);
            Ok(())
        } else {
            Err(AbiError::StaleHandle)
        }
    }

    /// Return an up-to-date descriptor for a committed region.
    pub fn descriptor(&self, handle: Handle) -> Result<MemoryDescriptor, AbiError> {
        let index = self.validate_occupied(handle)?;
        if let Slot::Occupied { region, .. } = &self.slots[index] {
            Ok(region.as_descriptor(handle.generation))
        } else {
            Err(AbiError::StaleHandle)
        }
    }

    /// Allocate a region, write `data` into it, commit it, and return the
    /// handle and its descriptor.
    pub fn store(&mut self, data: &[u8]) -> Result<(Handle, MemoryDescriptor), AbiError> {
        if data.is_empty() {
            return Err(AbiError::InvalidData);
        }
        let (handle, _desc) = self.request(data.len())?;
        self.write(handle, data)?;
        self.commit(handle, data.len())?;
        let desc = self.descriptor(handle)?;
        Ok((handle, desc))
    }

    /// Read a committed region.
    pub fn read(&self, handle: Handle) -> Result<&[u8], AbiError> {
        let index = self.validate_occupied(handle)?;
        if let Slot::Occupied { region, .. } = &self.slots[index] {
            Ok(&region.data[..region.committed])
        } else {
            Err(AbiError::StaleHandle)
        }
    }

    /// Release a region back to the arena.
    pub fn release(&mut self, handle: Handle) -> Result<(), AbiError> {
        let index = self.validate(handle)?;
        // Preserve the freed handle's generation on the slot so that a second
        // release of the same handle is reported as a double free.
        let freed_generation = handle.generation;
        self.slots[index] = Slot::Free {
            generation: freed_generation,
        };
        self.free.push(index);
        Ok(())
    }

    /// Total number of slots (free + occupied).
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Number of currently occupied slots.
    pub fn occupied_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| matches!(s, Slot::Occupied { .. }))
            .count()
    }

    /// Validate that the handle points to an occupied slot.
    ///
    /// A freed handle is reported as `StaleHandle`, even if its generation
    /// matches a now-free slot, because the caller is trying to use it rather
    /// than release it again.
    fn validate_occupied(&self, handle: Handle) -> Result<usize, AbiError> {
        match self.validate(handle) {
            Err(AbiError::DoubleFree) => Err(AbiError::StaleHandle),
            other => other,
        }
    }

    /// Validate a handle and return its slot index.
    fn validate(&self, handle: Handle) -> Result<usize, AbiError> {
        if handle.instance_id != self.instance_id {
            return Err(AbiError::WrongInstance);
        }
        let index = handle.slot as usize;
        if index >= self.slots.len() {
            return Err(AbiError::OutOfBounds);
        }
        match &self.slots[index] {
            Slot::Free { generation } => {
                if *generation == handle.generation {
                    Err(AbiError::DoubleFree)
                } else {
                    Err(AbiError::StaleHandle)
                }
            }
            Slot::Occupied { generation, .. } => {
                if *generation == handle.generation {
                    Ok(index)
                } else {
                    Err(AbiError::StaleHandle)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_rejects_out_of_bounds_size() {
        let mut arena = MemoryArena::new(1);
        assert_eq!(arena.request(usize::MAX), Err(AbiError::OutOfBounds));
        assert_eq!(arena.request(0), Err(AbiError::InvalidData));
    }
}
