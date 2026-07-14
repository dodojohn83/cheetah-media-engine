//! Incremental FLV demuxer.

use alloc::vec::Vec;

use cheetah_media_types::{
    BufferRef, CodecId, MediaPacket, MediaTime, SequenceNumber, StreamEpoch, TimeBase, Timestamp,
    TrackId, TrackInfo, TrackKind,
};

use crate::{
    FlvError,
    amf::{AmfLimits, FlvScriptData, parse_script_data},
    audio::{AudioTagHeader, parse_aac_config},
    header::{FlvHeader, FlvTagHeader, parse_file_header, parse_tag_header, tag_total_size},
    video::{VideoCodecId, VideoTagHeader, is_keyframe, parse_video_config},
};

/// Stream parsing mode. Auto detects whether previous-tag-size fields are present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlvMode {
    #[default]
    Auto,
    /// File mode: expect a 4-byte PreviousTagSize field after each tag.
    File,
    /// Stream mode: do not expect PreviousTagSize fields.
    Stream,
}

/// Output event from the FLV demuxer.
#[derive(Debug, Clone, PartialEq)]
pub enum FlvEvent {
    /// FLV file header parsed.
    Header(FlvHeader),
    /// A media track was discovered or its configuration changed.
    Track(TrackInfo),
    /// A compressed media packet.
    Packet(MediaPacket<'static>),
    /// Script / onMetaData tag.
    Script(FlvScriptData),
}

/// Track container for the FLV demuxer.
#[derive(Debug, Clone, Default)]
struct TrackState {
    info: Option<TrackInfo>,
}

impl TrackState {
    fn id(&self) -> Option<TrackId> {
        self.info.as_ref().map(|i| i.id)
    }

    fn info_mut(&mut self) -> Option<&mut TrackInfo> {
        self.info.as_mut()
    }
}

/// Incremental FLV demuxer.
#[derive(Debug)]
pub struct FlvDemuxer {
    mode: FlvMode,
    buffer: Vec<u8>,
    read_pos: usize,
    header: Option<FlvHeader>,
    audio: TrackState,
    video: TrackState,
    sequence: u64,
    stream_epoch: StreamEpoch,
    last_raw_timestamp: i64,
    last_unwrapped_ms: i64,
    previous_tag_total_size: u32,
    pending_previous_tag_size: bool,
    next_audio_id: u32,
    next_video_id: u32,
    amf_limits: AmfLimits,
    epoch_jumps: u64,
}

impl Default for FlvDemuxer {
    fn default() -> Self {
        Self::new(FlvMode::Auto)
    }
}

impl FlvDemuxer {
    /// Create a new demuxer with the given mode.
    pub fn new(mode: FlvMode) -> Self {
        Self {
            mode,
            buffer: Vec::new(),
            read_pos: 0,
            header: None,
            audio: TrackState::default(),
            video: TrackState::default(),
            sequence: 0,
            stream_epoch: StreamEpoch::new(0),
            last_raw_timestamp: -1,
            last_unwrapped_ms: 0,
            previous_tag_total_size: 0,
            pending_previous_tag_size: false,
            next_audio_id: 1,
            next_video_id: 2,
            amf_limits: AmfLimits::default(),
            epoch_jumps: 0,
        }
    }

    /// Push additional bytes into the demuxer buffer.
    pub fn push(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Set AMF parsing limits.
    pub fn set_amf_limits(&mut self, limits: AmfLimits) {
        self.amf_limits = limits;
    }

    /// Return the next parsed event, or `None` if more data is required.
    ///
    /// `NeedMoreData` is returned when a primitive cannot be completed. It is
    /// not fatal; call `push` with more bytes and retry.
    pub fn next_event(&mut self) -> Result<Option<FlvEvent>, FlvError> {
        loop {
            if self.header.is_none() {
                return self.parse_header();
            }

            // Retry an unfinished trailing PreviousTagSize from the last call.
            if self.pending_previous_tag_size {
                if self.mode == FlvMode::Stream {
                    self.pending_previous_tag_size = false;
                } else if self.available() >= 4 {
                    if let Some(total) = self.try_consume_previous_tag_size()? {
                        self.previous_tag_total_size = total;
                    }
                    self.pending_previous_tag_size = false;
                    continue;
                } else if self.mode == FlvMode::File {
                    return Err(FlvError::NeedMoreData);
                } else {
                    return Ok(None);
                }
            }

            // First tag boundary: optional PreviousTagSize0 (must be 0).
            if self.previous_tag_total_size == 0 {
                let consumed = self.consume_optional_previous_tag_size(0)?;
                if consumed {
                    continue;
                }
            }

            let tag = match self.parse_tag_header()? {
                Some(h) => h,
                None => return Ok(None),
            };

            let start_pos = self.read_pos;
            let event = self.process_tag(tag)?;

            // If read_pos didn't advance, the tag body is not fully buffered.
            if event.is_none() && self.read_pos == start_pos {
                return Ok(None);
            }

            // After a tag, optionally consume the 4-byte previous tag size.
            if let Some(total) = self.try_consume_previous_tag_size()? {
                self.previous_tag_total_size = total;
            }

            if let Some(event) = event {
                return Ok(Some(event));
            }

            // The tag was consumed but produced no event; keep parsing.
        }
    }

    fn available(&self) -> usize {
        self.buffer.len().saturating_sub(self.read_pos)
    }

    fn parse_header(&mut self) -> Result<Option<FlvEvent>, FlvError> {
        if self.available() < 9 {
            return Ok(None);
        }
        let header = parse_file_header(&self.buffer[self.read_pos..self.read_pos + 9])?;
        let skip = header.header_size as usize;
        if self.available() < skip {
            return Ok(None);
        }
        self.read_pos += skip;
        self.header = Some(header);
        self.shrink();
        Ok(Some(FlvEvent::Header(header)))
    }

    /// If the next 4 bytes equal `expected_total_size`, consume them and return true.
    fn consume_optional_previous_tag_size(&mut self, expected: u32) -> Result<bool, FlvError> {
        if self.mode == FlvMode::Stream {
            return Ok(false);
        }
        if self.available() < 4 {
            if self.mode == FlvMode::File {
                return Err(FlvError::NeedMoreData);
            }
            return Ok(false);
        }
        let bytes = [
            self.buffer[self.read_pos],
            self.buffer[self.read_pos + 1],
            self.buffer[self.read_pos + 2],
            self.buffer[self.read_pos + 3],
        ];
        let value = u32::from_be_bytes(bytes);
        if value == expected {
            self.read_pos += 4;
            return Ok(true);
        }
        if self.mode == FlvMode::File {
            return Err(FlvError::MalformedTag);
        }
        Ok(false)
    }

    /// Try to consume a previous tag size after a tag without enforcing it.
    fn try_consume_previous_tag_size(&mut self) -> Result<Option<u32>, FlvError> {
        if self.mode == FlvMode::Stream {
            return Ok(None);
        }
        if self.available() < 4 {
            if self.mode == FlvMode::File {
                return Err(FlvError::NeedMoreData);
            }
            self.pending_previous_tag_size = true;
            return Ok(None);
        }
        let bytes = [
            self.buffer[self.read_pos],
            self.buffer[self.read_pos + 1],
            self.buffer[self.read_pos + 2],
            self.buffer[self.read_pos + 3],
        ];
        let value = u32::from_be_bytes(bytes);
        // Only consume if it looks like a previous tag size: the next byte after
        // the 4 bytes is either missing or a valid tag type byte (8, 9, 18).
        let next_type = self.buffer.get(self.read_pos + 4).copied();
        let looks_valid =
            next_type.is_none() || matches!(next_type.unwrap_or(0) & 0x1f, 8 | 9 | 18);
        if looks_valid && value == self.previous_tag_total_size {
            self.read_pos += 4;
            return Ok(Some(value));
        }
        Ok(None)
    }

    fn parse_tag_header(&mut self) -> Result<Option<FlvTagHeader>, FlvError> {
        if self.available() < 11 {
            return Ok(None);
        }
        let header = parse_tag_header(&self.buffer[self.read_pos..self.read_pos + 11])?;
        if header.filter {
            return Err(FlvError::MalformedTag);
        }
        Ok(Some(header))
    }

    fn process_tag(&mut self, header: FlvTagHeader) -> Result<Option<FlvEvent>, FlvError> {
        let total_size = tag_total_size(header.data_size);
        if self.available() < total_size as usize {
            return Ok(None);
        }

        let start = self.read_pos;
        let data_start = start + 11;
        let data_end = data_start + header.data_size as usize;
        let data = self.buffer[data_start..data_end].to_vec();
        self.read_pos = data_end;
        self.previous_tag_total_size = total_size;
        self.shrink();

        match header.tag_type {
            crate::header::TagType::Audio => self.process_audio_tag(header, &data),
            crate::header::TagType::Video => self.process_video_tag(header, &data),
            crate::header::TagType::Script => self.process_script_tag(&data),
        }
    }

    fn process_audio_tag(
        &mut self,
        header: FlvTagHeader,
        data: &[u8],
    ) -> Result<Option<FlvEvent>, FlvError> {
        let ah = AudioTagHeader::parse(data)?;

        let track_id = if let Some(id) = self.audio.id() {
            id
        } else {
            let id = TrackId::new(self.next_audio_id).ok_or(FlvError::LimitExceeded)?;
            self.next_audio_id += 2; // keep audio/video ids distinct
            let info = TrackInfo::new(id, TrackKind::Audio, CodecId::Aac, TimeBase::DEFAULT);
            self.audio.info = Some(info);
            id
        };

        if ah.is_aac_config() {
            let mut track = self.audio.info_mut().ok_or(FlvError::MalformedTag)?.clone();
            parse_aac_config(&mut track, &data[ah.header_size..])?;
            self.audio.info = Some(track.clone());
            return Ok(Some(FlvEvent::Track(track)));
        }

        let codec = ah
            .sound_format
            .to_codec_id()
            .ok_or(FlvError::UnsupportedCodec)?;

        // Update track codec if it changed (e.g. first raw frame before config).
        if let Some(info) = self.audio.info_mut()
            && info.codec != codec
        {
            info.codec = codec;
        }

        let payload = &data[ah.header_size..];
        if payload.is_empty() {
            return Ok(None);
        }

        let dts_ms = self.unwrapped_timestamp_ms(header.timestamp_ms);
        let time = MediaTime::from_ticks(Some(dts_ms), Some(dts_ms), None, TimeBase::DEFAULT);
        let packet = self.new_packet(track_id, payload, time, false);
        Ok(Some(FlvEvent::Packet(packet)))
    }

    fn process_video_tag(
        &mut self,
        header: FlvTagHeader,
        data: &[u8],
    ) -> Result<Option<FlvEvent>, FlvError> {
        let vh = VideoTagHeader::parse(data)?;

        let track_id = if let Some(id) = self.video.id() {
            id
        } else {
            let id = TrackId::new(self.next_video_id).ok_or(FlvError::LimitExceeded)?;
            self.next_video_id += 2;
            let info = TrackInfo::new(id, TrackKind::Video, CodecId::H264, TimeBase::DEFAULT);
            self.video.info = Some(info);
            id
        };

        match vh.codec_id {
            VideoCodecId::H264 | VideoCodecId::H265 => {
                if vh.packet_type == 0 {
                    let mut track = self.video.info_mut().ok_or(FlvError::MalformedTag)?.clone();
                    parse_video_config(&mut track, &data[vh.header_size..], vh.codec_id)?;
                    self.video.info = Some(track.clone());
                    return Ok(Some(FlvEvent::Track(track)));
                }
                if vh.packet_type == 2 {
                    // End of sequence; no media packet.
                    return Ok(None);
                }
                let payload = &data[vh.header_size..];
                if payload.is_empty() {
                    return Ok(None);
                }
                let key = is_keyframe(payload, &vh);
                let dts_ms = self.unwrapped_timestamp_ms(header.timestamp_ms);
                let pts_ms = dts_ms + i64::from(vh.cts_ms);
                let time =
                    MediaTime::from_ticks(Some(pts_ms), Some(dts_ms), None, TimeBase::DEFAULT);
                let packet = self.new_packet(track_id, payload, time, key);
                Ok(Some(FlvEvent::Packet(packet)))
            }
            _ => Err(FlvError::UnsupportedCodec),
        }
    }

    fn process_script_tag(&mut self, data: &[u8]) -> Result<Option<FlvEvent>, FlvError> {
        let meta = parse_script_data(data, self.amf_limits)?;
        Ok(Some(FlvEvent::Script(meta)))
    }

    fn unwrapped_timestamp_ms(&mut self, raw_timestamp_ms: u32) -> i64 {
        let raw = i64::from(raw_timestamp_ms);
        if self.last_raw_timestamp < 0 {
            self.last_raw_timestamp = raw;
            self.last_unwrapped_ms = raw;
            return raw;
        }

        let previous = Timestamp::new(self.last_unwrapped_ms);
        let current = Timestamp::new(raw);
        let unwrapped = current.unwrapped_around(previous, 32);
        let mut unwrapped_ms = unwrapped.ticks();

        // Detect a non-wrap backward jump (reset/discontinuity).
        const RESET_THRESHOLD: i64 = 10_000; // 10 seconds
        if unwrapped_ms < self.last_unwrapped_ms - RESET_THRESHOLD {
            self.epoch_jumps += 1;
            self.stream_epoch = StreamEpoch::new(self.epoch_jumps);
            unwrapped_ms = raw;
        }

        self.last_raw_timestamp = raw;
        self.last_unwrapped_ms = unwrapped_ms;
        unwrapped_ms
    }

    fn new_packet(
        &mut self,
        track_id: TrackId,
        payload: &[u8],
        time: MediaTime,
        keyframe: bool,
    ) -> MediaPacket<'static> {
        let seq = SequenceNumber::new(self.sequence);
        self.sequence += 1;
        let mut packet = MediaPacket::new(
            BufferRef::from_owned(payload.to_vec()),
            track_id,
            self.stream_epoch,
            seq,
            time,
        );
        packet.flags.is_keyframe = keyframe;
        packet
    }

    fn shrink(&mut self) {
        // Periodically discard consumed prefix to keep the buffer bounded.
        if self.read_pos > 4096 && self.read_pos * 2 > self.buffer.len() {
            let new_len = self.buffer.len() - self.read_pos;
            self.buffer.copy_within(self.read_pos.., 0);
            self.buffer.truncate(new_len);
            self.read_pos = 0;
        }
    }
}
