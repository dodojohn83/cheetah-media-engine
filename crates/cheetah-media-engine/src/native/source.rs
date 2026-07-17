//! In-memory byte source for native player tests and headless benchmarks.

use alloc::vec::Vec;

use cheetah_media_backend_api::{ByteSource, ByteSourceError, ByteSourceEvent, SourceStats};

/// A `ByteSource` backed by an owned byte buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryByteSource {
    data: Vec<u8>,
    position: usize,
    chunk_size: usize,
    finished: bool,
    stats: SourceStats,
}

impl MemoryByteSource {
    /// Create a source from `data`, returning up to `chunk_size` bytes per
    /// read.
    pub fn new(data: Vec<u8>, chunk_size: usize) -> Self {
        Self {
            data,
            position: 0,
            chunk_size: chunk_size.max(1),
            finished: false,
            stats: SourceStats::default(),
        }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }
}

impl ByteSource for MemoryByteSource {
    fn start(&mut self, _url: &str) -> Result<(), ByteSourceError> {
        self.position = 0;
        self.finished = false;
        self.stats = SourceStats::default();
        Ok(())
    }

    fn read_or_push<'a>(&'a mut self, _buf: &mut [u8]) -> ByteSourceEvent<'a> {
        if self.finished {
            return ByteSourceEvent::Eof;
        }
        if self.position >= self.data.len() {
            self.finished = true;
            return ByteSourceEvent::Eof;
        }
        let end = self.position + self.chunk_size.min(self.remaining());
        let chunk = &self.data[self.position..end];
        self.position = end;
        self.stats.bytes_received += chunk.len() as u64;
        ByteSourceEvent::Data(chunk)
    }

    fn cancel(&mut self) -> Result<(), ByteSourceError> {
        self.position = 0;
        self.finished = true;
        Ok(())
    }

    fn stats(&self) -> SourceStats {
        self.stats
    }
}
