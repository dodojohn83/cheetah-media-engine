//! Incremental Annex-B byte-stream demuxer.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use cheetah_media_bitstream::{parse_sei, unescape_rbsp};
use cheetah_media_types::{
    CodecId, MediaPacket, MediaTime, MetadataItem, PacketFlags, SequenceNumber, StreamEpoch,
    TimeBase, Timestamp, TrackId, TrackInfo, TrackKind,
};

use crate::error::AnnexbError;
use crate::param_sets::ParameterSetCache;

/// Default maximum NAL unit size in bytes.
const DEFAULT_MAX_NAL_SIZE: usize = 16 * 1024 * 1024;
/// Default maximum input buffer size in bytes.
const DEFAULT_MAX_BUFFER_SIZE: usize = 32 * 1024 * 1024;

/// Configuration for an Annex-B demuxer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnexBConfig {
    /// Track identifier for emitted packets.
    pub track_id: TrackId,
    /// Expected codec; this crate supports H.264 and H.265.
    pub codec: CodecId,
    /// Timebase used for generated timestamps.
    pub timebase: TimeBase,
    /// Stream epoch for emitted packets.
    pub stream_epoch: StreamEpoch,
    /// Maximum size of a single NAL unit.
    pub max_nal_size_bytes: usize,
    /// Maximum size of the internal byte buffer.
    pub max_buffer_bytes: usize,
}

impl AnnexBConfig {
    /// Create a new config for H.264.
    pub fn h264(track_id: TrackId, timebase: TimeBase) -> Self {
        Self {
            track_id,
            codec: CodecId::H264,
            timebase,
            stream_epoch: StreamEpoch::new(0),
            max_nal_size_bytes: DEFAULT_MAX_NAL_SIZE,
            max_buffer_bytes: DEFAULT_MAX_BUFFER_SIZE,
        }
    }

    /// Create a new config for H.265.
    pub fn h265(track_id: TrackId, timebase: TimeBase) -> Self {
        Self {
            track_id,
            codec: CodecId::H265,
            timebase,
            stream_epoch: StreamEpoch::new(0),
            max_nal_size_bytes: DEFAULT_MAX_NAL_SIZE,
            max_buffer_bytes: DEFAULT_MAX_BUFFER_SIZE,
        }
    }
}

/// Event emitted by `AnnexBDemuxer`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnexbEvent {
    /// A media track was discovered or its configuration changed.
    Track(TrackInfo),
    /// A compressed media packet.
    Packet(MediaPacket<'static>),
    /// Metadata extracted from SEI or other non-VCL NAL units.
    Metadata(Vec<MetadataItem>),
    /// End of stream.
    Eof,
}

/// Incremental Annex-B H.264/H.265 demuxer.
#[derive(Debug)]
pub struct AnnexBDemuxer {
    config: AnnexBConfig,
    buffer: Vec<u8>,
    pending_events: VecDeque<AnnexbEvent>,
    track: TrackInfo,
    sequence: u64,
    param_sets: ParameterSetCache,
    ended: bool,
    eof_emitted: bool,
    codec_error: Option<AnnexbError>,
}

impl AnnexBDemuxer {
    /// Create a new demuxer with the given configuration.
    pub fn new(config: AnnexBConfig) -> Self {
        let track = TrackInfo::new(
            config.track_id,
            TrackKind::Video,
            config.codec,
            config.timebase,
        );
        Self {
            config,
            buffer: Vec::new(),
            pending_events: VecDeque::new(),
            track,
            sequence: 0,
            param_sets: ParameterSetCache::new(config.codec),
            ended: false,
            eof_emitted: false,
            codec_error: None,
        }
    }

    /// Push more Annex-B bytes into the demuxer.
    pub fn push(&mut self, data: &[u8]) {
        if !data.is_empty() {
            self.buffer.extend_from_slice(data);
        }
    }

    /// Return the next parsed event, or `None` if more data is needed.
    pub fn next_event(&mut self) -> Result<Option<AnnexbEvent>, AnnexbError> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(Some(event));
        }
        if let Some(err) = self.codec_error {
            return Err(err);
        }
        if !matches!(self.config.codec, CodecId::H264 | CodecId::H265) {
            self.codec_error = Some(AnnexbError::UnsupportedCodec);
            return Err(AnnexbError::UnsupportedCodec);
        }

        if self.ended {
            self.process_eof()?;
        } else {
            if self.buffer.len() > self.config.max_buffer_bytes {
                return Err(AnnexbError::BufferExceeded {
                    max: self.config.max_buffer_bytes,
                });
            }
            let mut processed = true;
            while processed && self.pending_events.is_empty() {
                processed = self.process_one()?;
            }
        }

        Ok(self.pending_events.pop_front())
    }

    /// Signal that no more bytes will arrive.
    pub fn end(&mut self) -> Result<(), AnnexbError> {
        if self.ended {
            return Ok(());
        }
        self.ended = true;
        if !matches!(self.config.codec, CodecId::H264 | CodecId::H265) {
            return Err(AnnexbError::UnsupportedCodec);
        }
        Ok(())
    }

    /// Reset the demuxer to a clean state, discarding buffered data and events.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.pending_events.clear();
        self.sequence = 0;
        self.param_sets.reset();
        self.track = TrackInfo::new(
            self.config.track_id,
            TrackKind::Video,
            self.config.codec,
            self.config.timebase,
        );
        self.ended = false;
        self.eof_emitted = false;
        self.codec_error = None;
    }

    /// Current buffered byte count.
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    fn process_one(&mut self) -> Result<bool, AnnexbError> {
        let (start_pos, code_len) = match find_start_code(&self.buffer, 0) {
            Some(v) => v,
            None => return Ok(false),
        };

        let header_pos = start_pos.saturating_add(code_len);
        if header_pos >= self.buffer.len() {
            // Start code is at the end of the buffer; need the NAL header.
            return Ok(false);
        }

        // A NAL unit is complete only when we see the start code of the next NAL.
        let next = match find_start_code(&self.buffer, header_pos) {
            Some(v) => v,
            None => return Ok(false),
        };
        let (next_start, _) = next;
        let nal_end = next_start;

        let nal_size = nal_end.saturating_sub(header_pos);
        if nal_size > self.config.max_nal_size_bytes {
            return Err(AnnexbError::NalTooLarge {
                size: nal_size,
                max: self.config.max_nal_size_bytes,
            });
        }

        // Copy the NAL out before mutating the buffer.
        let nal = self.buffer[header_pos..nal_end].to_vec();

        // Drain consumed bytes up to the next start code, keeping it in the buffer.
        self.buffer.drain(0..next_start);

        self.handle_nal(nal)?;
        Ok(true)
    }

    fn process_eof(&mut self) -> Result<bool, AnnexbError> {
        if self.eof_emitted {
            return Ok(false);
        }

        // Process all complete NALs before flushing the final one.
        while self.process_one()? {}

        // Flush the final NAL whose start code is in the buffer but has no
        // following start code yet.
        if let Some((start_pos, code_len)) = find_start_code(&self.buffer, 0) {
            let header_pos = start_pos.saturating_add(code_len);
            if header_pos < self.buffer.len() {
                let nal = self.buffer[header_pos..].to_vec();
                if nal.len() > self.config.max_nal_size_bytes {
                    return Err(AnnexbError::NalTooLarge {
                        size: nal.len(),
                        max: self.config.max_nal_size_bytes,
                    });
                }
                self.buffer.clear();
                self.eof_emitted = true;
                self.handle_nal(nal)?;
                self.pending_events.push_back(AnnexbEvent::Eof);
                return Ok(true);
            }
        }

        self.buffer.clear();
        self.eof_emitted = true;
        self.pending_events.push_back(AnnexbEvent::Eof);
        Ok(true)
    }

    fn handle_nal(&mut self, nal: Vec<u8>) -> Result<(), AnnexbError> {
        // Reject obviously malformed NAL headers to avoid mis-parsing parameter sets.
        if nal.is_empty() {
            return Ok(());
        }

        if self.is_sei(&nal) {
            if let Some(items) = self.extract_sei_metadata(&nal)
                && !items.is_empty()
            {
                self.pending_events.push_back(AnnexbEvent::Metadata(items));
            }
            // SEI NALs are not forwarded as video packets.
            return Ok(());
        }

        if self.param_sets.consume(&nal) {
            // Only emit a Track once a complete decoder configuration is available.
            if self.param_sets.is_complete() {
                let old_config = self.track.codec_config.clone();
                let old_format = self.track.video_format;
                self.param_sets.update_track(&mut self.track);

                if self.track.codec_config != old_config || self.track.video_format != old_format {
                    self.pending_events
                        .push_back(AnnexbEvent::Track(self.track.clone()));
                }
            }
            return Ok(());
        }

        // For data NALs, emit a packet.
        let is_keyframe = Self::is_keyframe(self.config.codec, &nal);
        let packet = self.new_packet(nal, is_keyframe);
        self.pending_events.push_back(AnnexbEvent::Packet(packet));
        Ok(())
    }

    /// Return true for H.264 SEI (NAL type 6) and H.265 SEI (NAL types 39/40).
    fn is_sei(&self, nal: &[u8]) -> bool {
        match self.config.codec {
            CodecId::H264 => {
                if nal.is_empty() {
                    return false;
                }
                (nal[0] & 0x1f) == 6
            }
            CodecId::H265 => {
                if nal.len() < 2 {
                    return false;
                }
                let nal_type = (nal[0] >> 1) & 0x3f;
                matches!(nal_type, 39 | 40)
            }
            _ => false,
        }
    }

    fn extract_sei_metadata(&self, nal: &[u8]) -> Option<Vec<MetadataItem>> {
        let payload = match self.config.codec {
            CodecId::H264 if nal.len() > 1 => &nal[1..],
            CodecId::H265 if nal.len() > 2 => &nal[2..],
            _ => return None,
        };
        let rbsp = unescape_rbsp(payload);
        let messages = parse_sei(&rbsp).ok()?;
        Some(
            messages
                .into_iter()
                .map(|m| MetadataItem::sei(m.payload_type, m.payload))
                .collect(),
        )
    }

    /// Return true for H.264 IDR slices and H.265 IRAP slices.
    fn is_keyframe(codec: CodecId, nal: &[u8]) -> bool {
        match codec {
            CodecId::H264 => {
                if nal.is_empty() {
                    return false;
                }
                (nal[0] & 0x1f) == 5
            }
            CodecId::H265 => {
                if nal.len() < 2 {
                    return false;
                }
                let nal_type = (nal[0] >> 1) & 0x3f;
                (16..=23).contains(&nal_type)
            }
            _ => false,
        }
    }

    fn new_packet(&mut self, data: Vec<u8>, is_keyframe: bool) -> MediaPacket<'static> {
        let seq = SequenceNumber::new(self.sequence);
        self.sequence += 1;
        let time = MediaTime::from_pts_dts(
            Timestamp::new(seq.get() as i64),
            Timestamp::new(seq.get() as i64),
            self.config.timebase,
        );
        let mut packet = MediaPacket::new(
            data,
            self.config.track_id,
            self.config.stream_epoch,
            seq,
            time,
        );
        packet.flags = PacketFlags {
            is_keyframe,
            ..PacketFlags::default()
        };
        packet
    }
}

/// Find the next Annex-B start code, skipping emulation prevention bytes.
///
/// Returns `(position, code_len)` where `code_len` is 3 or 4.
pub fn find_start_code(data: &[u8], start: usize) -> Option<(usize, usize)> {
    if data.len() < start.saturating_add(3) {
        return None;
    }
    let mut i = start;
    while i.saturating_add(2) < data.len() {
        if data[i] == 0x00 && data[i + 1] == 0x00 {
            if i + 3 < data.len() && data[i + 2] == 0x00 && data[i + 3] == 0x01 {
                return Some((i, 4));
            }
            if data[i + 2] == 0x01 {
                return Some((i, 3));
            }
            if data[i + 2] == 0x03 && i + 3 < data.len() && data[i + 3] <= 0x03 {
                // Emulation prevention: skip the entire 0x00 0x00 0x03 XX sequence
                // and resume scanning from the byte after the protected value.
                i += 4;
                continue;
            }
        }
        i += 1;
    }
    None
}

/// Strip the NAL header byte to obtain the RBSP payload.
pub fn nal_payload(nal: &[u8]) -> &[u8] {
    &nal[1..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_start_code_detects_three_and_four_byte_codes() {
        let data = [0x00, 0x00, 0x00, 0x01, 0x67, 0x00, 0x00, 0x01, 0x68];
        assert_eq!(find_start_code(&data, 0), Some((0, 4)));
        assert_eq!(find_start_code(&data, 4), Some((5, 3)));
    }

    #[test]
    fn find_start_code_skips_emulation_prevention() {
        // Payload contains 00 00 03 01 which must not be treated as a start code.
        let data = [
            0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x03, 0x01, 0x00, 0x00, 0x01, 0x02,
        ];
        assert_eq!(find_start_code(&data, 0), Some((0, 4)));
        assert_eq!(find_start_code(&data, 4), Some((9, 3)));
    }

    #[test]
    fn find_start_code_skips_emulation_prevention_at_nal_boundary() {
        // A NAL ending with the EPB sequence 00 00 03 00 must not have its
        // protected payload byte consumed as part of the next start code.
        let data = [
            0x00, 0x00, 0x00, 0x01, // 4-byte start code
            0x67, // NAL header
            0x00, 0x00, 0x03, 0x00, // EPB inside NAL payload
            0x00, 0x00, 0x01, // 3-byte start code of next NAL
            0x68, // next NAL header
        ];
        assert_eq!(find_start_code(&data, 0), Some((0, 4)));
        assert_eq!(find_start_code(&data, 4), Some((9, 3)));
    }
}
