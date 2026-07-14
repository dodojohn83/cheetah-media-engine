//! Compressed media packets.

use crate::{
    BufferLifecycle, BufferRef, MediaDuration, MediaError, MediaLimits, MediaTime, SequenceNumber,
    StreamEpoch, TrackId,
};

/// Flags describing a compressed media packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct PacketFlags {
    pub is_keyframe: bool,
    pub is_corrupt: bool,
    pub is_discontinuity: bool,
}

/// A compressed media packet.
///
/// The payload uses a `BufferRef` so that transport data can be shared between
/// parser, demuxer, and decoder without copying. Borrowed payloads keep the
/// original lifetime; shared payloads use reference-counted storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPacket<'a> {
    pub payload: BufferRef<'a>,
    pub track_id: TrackId,
    pub stream_epoch: StreamEpoch,
    pub sequence: SequenceNumber,
    pub time: MediaTime,
    pub flags: PacketFlags,
}

impl<'a> MediaPacket<'a> {
    /// Create a new packet with the given payload and metadata.
    pub fn new(
        payload: impl Into<BufferRef<'a>>,
        track_id: TrackId,
        stream_epoch: StreamEpoch,
        sequence: SequenceNumber,
        time: MediaTime,
    ) -> Self {
        Self {
            payload: payload.into(),
            track_id,
            stream_epoch,
            sequence,
            time,
            flags: PacketFlags::default(),
        }
    }

    /// Validate the packet against configured limits.
    ///
    /// Rejects oversized payloads. No payload content is logged.
    pub fn check_limits(&self, limits: &MediaLimits) -> Result<(), MediaError> {
        limits.check_chunk_size(self.payload.len() as u64)
    }

    /// Return the duration as a `MediaDuration`, if known.
    pub fn duration(&self) -> Option<MediaDuration> {
        self.time.duration.map(|t| MediaDuration::new(t.ticks()))
    }

    /// Mark this packet as a keyframe.
    pub fn with_keyframe(mut self) -> Self {
        self.flags.is_keyframe = true;
        self
    }

    /// Slice the payload without copying.
    pub fn slice_payload(&self, range: core::ops::Range<usize>) -> BufferRef<'a> {
        self.payload.slice(range)
    }

    /// Lifetime classification of the payload.
    pub fn lifecycle(&self) -> BufferLifecycle {
        self.payload.lifecycle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TimeBase, Timestamp};

    #[test]
    fn packet_payload_is_borrowed_by_default() {
        let data = [0u8, 1, 2, 3];
        let time = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let packet = MediaPacket::new(
            &data[..],
            TrackId::new(1).unwrap(),
            StreamEpoch::new(0),
            SequenceNumber::new(0),
            time,
        );
        assert_eq!(packet.payload.as_ref(), &data);
        assert!(packet.payload.is_borrowed());
    }

    #[test]
    fn packet_payload_can_be_owned_and_shared() {
        let data = alloc::vec![0u8, 1, 2, 3];
        let time = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let packet = MediaPacket::new(
            data,
            TrackId::new(1).unwrap(),
            StreamEpoch::new(0),
            SequenceNumber::new(0),
            time,
        );
        assert_eq!(packet.payload.as_ref(), &[0, 1, 2, 3]);
        assert!(packet.payload.is_shared());
    }

    #[test]
    fn packet_limits_reject_oversized_payload() {
        let data = alloc::vec![0u8; 17 * 1024 * 1024];
        let time = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let packet = MediaPacket::new(
            data,
            TrackId::new(1).unwrap(),
            StreamEpoch::new(0),
            SequenceNumber::new(0),
            time,
        );
        assert!(packet.check_limits(&MediaLimits::default()).is_err());
    }
}
