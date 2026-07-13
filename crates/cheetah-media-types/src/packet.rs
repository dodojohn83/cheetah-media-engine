//! Compressed media packets.

use alloc::borrow::Cow;

use crate::{
    MediaDuration, MediaError, MediaLimits, MediaTime, SequenceNumber, StreamEpoch, TrackId,
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
/// The payload is borrowed or owned via `Cow` so core code can avoid copies while
/// still supporting owned data for network/parser adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPacket<'a> {
    pub payload: Cow<'a, [u8]>,
    pub track_id: TrackId,
    pub stream_epoch: StreamEpoch,
    pub sequence: SequenceNumber,
    pub time: MediaTime,
    pub flags: PacketFlags,
}

impl<'a> MediaPacket<'a> {
    /// Create a new packet with the given payload and metadata.
    pub fn new(
        payload: impl Into<Cow<'a, [u8]>>,
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
