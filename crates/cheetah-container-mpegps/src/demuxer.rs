//! MPEG-2 Program Stream demuxer.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use cheetah_container_annexb::{AnnexBConfig, AnnexBDemuxer, AnnexbError, AnnexbEvent};
use cheetah_media_bitstream::aac::{AdtsHeader, AudioSpecificConfig};
use cheetah_media_types::{
    AudioFormat, ChannelLayout, CodecConfig, CodecId, MediaPacket, MediaTime, PacketFlags,
    SampleFormat, SequenceNumber, StreamEpoch, TimeBase, Timestamp, TrackId, TrackInfo, TrackKind,
};

use crate::error::MpegPsError;
use crate::pack::{is_pack_start_code, is_system_start_code, parse_pack_header};
use crate::pes::{is_audio_stream, is_video_stream, parse_pes_header};
use crate::scan::{find_ps_boundary, find_start_code};

/// Default maximum input buffer size in bytes.
const DEFAULT_MAX_BUFFER_SIZE: usize = 32 * 1024 * 1024;

/// Default maximum NAL size passed to the Annex-B demuxer.
const DEFAULT_MAX_NAL_SIZE: usize = 4 * 1024 * 1024;

/// Video track identifier used for all emitted video packets.
const VIDEO_TRACK_ID: u32 = 1;

/// Audio track identifier used for all emitted audio packets.
const AUDIO_TRACK_ID: u32 = 2;

/// Configuration for the MPEG-PS demuxer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MpegPsConfig {
    /// Expected video codec: `H264` or `H265`.
    pub video_codec: CodecId,
    /// Maximum accepted PES packet size in bytes.
    pub max_packet_size_bytes: usize,
    /// Maximum internal buffer size in bytes.
    pub max_buffer_bytes: usize,
    /// Maximum single NAL size passed to the Annex-B demuxer.
    pub max_nal_size_bytes: usize,
}

impl MpegPsConfig {
    /// Create a new config for H.264 video.
    pub fn h264() -> Self {
        Self {
            video_codec: CodecId::H264,
            max_packet_size_bytes: crate::DEFAULT_MAX_PES_SIZE,
            max_buffer_bytes: DEFAULT_MAX_BUFFER_SIZE,
            max_nal_size_bytes: DEFAULT_MAX_NAL_SIZE,
        }
    }

    /// Create a new config for H.265 video.
    pub fn h265() -> Self {
        Self {
            video_codec: CodecId::H265,
            max_packet_size_bytes: crate::DEFAULT_MAX_PES_SIZE,
            max_buffer_bytes: DEFAULT_MAX_BUFFER_SIZE,
            max_nal_size_bytes: DEFAULT_MAX_NAL_SIZE,
        }
    }
}

impl Default for MpegPsConfig {
    fn default() -> Self {
        Self::h264()
    }
}

/// Event emitted by `MpegPsDemuxer`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MpegPsEvent {
    /// A media track was discovered or its configuration changed.
    Track(TrackInfo),
    /// A compressed media packet.
    Packet(MediaPacket<'static>),
    /// End of stream.
    Eof,
}

/// Incremental MPEG-2 Program Stream demuxer.
#[derive(Debug)]
pub struct MpegPsDemuxer {
    config: MpegPsConfig,
    buffer: Vec<u8>,
    pending_events: VecDeque<MpegPsEvent>,
    video_demuxer: Option<AnnexBDemuxer>,
    audio_track: Option<TrackInfo>,
    audio_sequence: u64,
    ended: bool,
    eof_emitted: bool,
    video_track_seen: bool,
}

impl MpegPsDemuxer {
    /// Create a new demuxer with the given configuration.
    pub fn new(config: MpegPsConfig) -> Self {
        Self {
            config,
            buffer: Vec::new(),
            pending_events: VecDeque::new(),
            video_demuxer: None,
            audio_track: None,
            audio_sequence: 0,
            ended: false,
            eof_emitted: false,
            video_track_seen: false,
        }
    }

    /// Push more MPEG-PS bytes into the demuxer.
    pub fn push(&mut self, data: &[u8]) {
        if !data.is_empty() {
            self.buffer.extend_from_slice(data);
        }
    }

    /// Return the next parsed event, or `None` if more data is needed.
    pub fn next_event(&mut self) -> Result<Option<MpegPsEvent>, MpegPsError> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(Some(event));
        }

        loop {
            let progress = self.process_one()?;
            if let Some(event) = self.pending_events.pop_front() {
                return Ok(Some(event));
            }
            if !progress {
                break;
            }
        }

        if self.ended && !self.eof_emitted {
            self.flush_video()?;
            if let Some(event) = self.pending_events.pop_front() {
                return Ok(Some(event));
            }
            self.eof_emitted = true;
            return Ok(Some(MpegPsEvent::Eof));
        }

        Ok(None)
    }

    /// Signal that no more bytes will arrive.
    pub fn end(&mut self) -> Result<(), MpegPsError> {
        if self.ended {
            return Ok(());
        }
        self.ended = true;
        Ok(())
    }

    /// Reset the demuxer state, discarding any buffered data and pending events.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.pending_events.clear();
        self.video_demuxer = None;
        self.audio_track = None;
        self.audio_sequence = 0;
        self.ended = false;
        self.eof_emitted = false;
        self.video_track_seen = false;
    }

    fn process_one(&mut self) -> Result<bool, MpegPsError> {
        if self.buffer.is_empty() {
            return Ok(false);
        }

        if let Some(pos) = find_start_code(&self.buffer, 0) {
            if pos > 0 {
                self.buffer.drain(0..pos);
                return Ok(true);
            }
        } else {
            // Keep the last two bytes in case they are the start of a start code.
            let keep = self.buffer.len().saturating_sub(2);
            if keep > 0 {
                self.buffer.drain(0..keep);
            }
            if self.buffer.len() > self.config.max_buffer_bytes {
                return Err(MpegPsError::BufferExceeded {
                    max: self.config.max_buffer_bytes,
                });
            }
            return Ok(false);
        }

        // pos == 0; we have a start code at the front of the buffer.
        if self.buffer.len() < 4 {
            return Ok(false);
        }

        let code = self.buffer[3];

        if code == 0xB9 {
            // MPEG_program_end_code.
            self.buffer.drain(0..4);
            self.ended = true;
            return Ok(true);
        }

        if is_pack_start_code(code) {
            let header = parse_pack_header(&self.buffer)?;
            self.buffer.drain(0..header.end_offset);
            return Ok(true);
        }

        if is_system_start_code(code) {
            return self.skip_system_packet();
        }

        if is_video_stream(code) || is_audio_stream(code) || code == 0xBD {
            return self.process_pes(code);
        }

        // Unrecognized start code; skip it and continue.
        self.buffer.drain(0..4);
        Ok(true)
    }

    fn skip_system_packet(&mut self) -> Result<bool, MpegPsError> {
        if self.buffer.len() < 6 {
            return Ok(false);
        }
        let length = u16::from_be_bytes([self.buffer[4], self.buffer[5]]) as usize;
        let total = 6_usize.saturating_add(length);
        if total > self.config.max_packet_size_bytes {
            return Err(MpegPsError::PacketTooLarge {
                size: total,
                max: self.config.max_packet_size_bytes,
            });
        }
        if self.buffer.len() < total {
            return Ok(false);
        }
        self.buffer.drain(0..total);
        Ok(true)
    }

    fn process_pes(&mut self, code: u8) -> Result<bool, MpegPsError> {
        if self.buffer.len() < 6 {
            return Ok(false);
        }
        let packet_length = u16::from_be_bytes([self.buffer[4], self.buffer[5]]) as usize;

        let pes_end = if packet_length == 0 {
            // Unbounded PES: scan for the next PS boundary. This is safe for video
            // because NAL start codes are followed by bytes < 0x80.
            match find_ps_boundary(&self.buffer, 6) {
                Some(end) => end,
                None => {
                    if self.buffer.len() > self.config.max_buffer_bytes {
                        return Err(MpegPsError::BufferExceeded {
                            max: self.config.max_buffer_bytes,
                        });
                    }
                    return Ok(false);
                }
            }
        } else {
            let total = 6_usize.saturating_add(packet_length);
            if total > self.config.max_packet_size_bytes {
                return Err(MpegPsError::PacketTooLarge {
                    size: total,
                    max: self.config.max_packet_size_bytes,
                });
            }
            if self.buffer.len() < total {
                return Ok(false);
            }
            total
        };

        if pes_end < 9 {
            self.buffer.drain(0..pes_end);
            return Ok(true);
        }

        let header = match parse_pes_header(&self.buffer[..pes_end]) {
            Ok(h) => h,
            Err(MpegPsError::NeedMoreData) => return Ok(false),
            Err(e) => return Err(e),
        };

        if header.header_size > pes_end {
            return Ok(false);
        }

        let payload = self.buffer[header.header_size..pes_end].to_vec();
        let media_time = MediaTime::new(header.pts, header.dts, None, TimeBase::TS_90K);

        if is_video_stream(code) {
            if !self.video_track_seen {
                self.init_video_demuxer()?;
                self.video_track_seen = true;
            }
            self.process_video_payload(&payload, media_time)?;
        } else if is_audio_stream(code) || code == 0xBD {
            self.process_audio_payload(&payload, media_time)?;
        }

        self.buffer.drain(0..pes_end);
        Ok(true)
    }

    fn init_video_demuxer(&mut self) -> Result<(), MpegPsError> {
        if !matches!(self.config.video_codec, CodecId::H264 | CodecId::H265) {
            return Err(MpegPsError::UnsupportedVideoCodec);
        }

        let track_id = TrackId::new(VIDEO_TRACK_ID).expect("video track id 1 is valid");
        let config = AnnexBConfig {
            track_id,
            codec: self.config.video_codec,
            timebase: TimeBase::TS_90K,
            stream_epoch: StreamEpoch::new(0),
            max_nal_size_bytes: self.config.max_nal_size_bytes,
            max_buffer_bytes: self.config.max_buffer_bytes,
        };
        self.video_demuxer = Some(AnnexBDemuxer::new(config));
        Ok(())
    }

    fn process_video_payload(
        &mut self,
        payload: &[u8],
        media_time: MediaTime,
    ) -> Result<(), MpegPsError> {
        if let Some(demuxer) = self.video_demuxer.as_mut() {
            demuxer.push(payload);
            loop {
                match demuxer.next_event() {
                    Ok(Some(AnnexbEvent::Track(track))) => {
                        self.pending_events.push_back(MpegPsEvent::Track(track));
                    }
                    Ok(Some(AnnexbEvent::Packet(mut packet))) => {
                        packet.time = media_time;
                        self.pending_events.push_back(MpegPsEvent::Packet(packet));
                    }
                    Ok(Some(AnnexbEvent::Eof)) | Ok(None) => break,
                    Err(e) => return Err(map_annexb_error(e)),
                }
            }
        }
        Ok(())
    }

    fn flush_video(&mut self) -> Result<(), MpegPsError> {
        if let Some(demuxer) = self.video_demuxer.as_mut() {
            demuxer.end().map_err(map_annexb_error)?;
            loop {
                match demuxer.next_event() {
                    Ok(Some(AnnexbEvent::Track(track))) => {
                        self.pending_events.push_back(MpegPsEvent::Track(track));
                    }
                    Ok(Some(AnnexbEvent::Packet(packet))) => {
                        self.pending_events.push_back(MpegPsEvent::Packet(packet));
                    }
                    Ok(Some(AnnexbEvent::Eof)) | Ok(None) => break,
                    Err(e) => return Err(map_annexb_error(e)),
                }
            }
        }
        Ok(())
    }

    fn process_audio_payload(
        &mut self,
        payload: &[u8],
        media_time: MediaTime,
    ) -> Result<(), MpegPsError> {
        let track_id = TrackId::new(AUDIO_TRACK_ID).expect("audio track id 2 is valid");

        let mut offset = 0;
        while offset < payload.len() {
            let header = match AdtsHeader::parse(&payload[offset..]) {
                Ok(h) => h,
                Err(_) => break,
            };
            let frame_len = header.frame_length as usize;
            if frame_len == 0 || payload.len() - offset < frame_len {
                break;
            }

            if self.audio_track.is_none() {
                let track = self.build_audio_track(&header, track_id)?;
                self.audio_track = Some(track.clone());
                self.pending_events.push_back(MpegPsEvent::Track(track));
            }

            let frame = &payload[offset..offset + frame_len];
            let flags = PacketFlags {
                is_keyframe: true,
                is_corrupt: false,
                is_discontinuity: false,
            };
            let duration_ticks = (u64::from(header.samples_per_frame) * 90_000
                / u64::from(header.sampling_frequency)) as i64;
            let packet_time = MediaTime::new(
                media_time.pts,
                media_time.dts,
                Some(Timestamp::new(duration_ticks)),
                TimeBase::TS_90K,
            );
            let mut packet = MediaPacket::new(
                frame.to_vec(),
                track_id,
                StreamEpoch::new(0),
                SequenceNumber::new(self.audio_sequence),
                packet_time,
            );
            packet.flags = flags;
            self.audio_sequence += 1;
            self.pending_events.push_back(MpegPsEvent::Packet(packet));

            offset += frame_len;
        }
        Ok(())
    }

    fn build_audio_track(
        &self,
        header: &AdtsHeader,
        track_id: TrackId,
    ) -> Result<TrackInfo, MpegPsError> {
        let audio_object_type = header.profile + 1;
        let asc = AudioSpecificConfig {
            audio_object_type,
            sampling_frequency_index: header.sampling_frequency_index,
            sampling_frequency: header.sampling_frequency,
            channel_configuration: header.channel_configuration,
            channel_count: header.channel_count,
        };
        let config_bytes = asc.build();

        let channel_layout = match header.channel_count {
            1 => ChannelLayout::Mono,
            2 => ChannelLayout::Stereo,
            n => ChannelLayout::Unknown(u64::from(n)),
        };
        let audio_format = AudioFormat {
            sample_format: SampleFormat::Unknown(0),
            sample_rate: header.sampling_frequency,
            channel_layout,
            sample_count: u32::from(header.samples_per_frame),
        };

        let mut track = TrackInfo::new(track_id, TrackKind::Audio, CodecId::Aac, TimeBase::TS_90K);
        track.codec_config = CodecConfig::AacAudioSpecificConfig(config_bytes);
        track.audio_format = Some(audio_format);
        Ok(track)
    }
}

fn map_annexb_error(err: AnnexbError) -> MpegPsError {
    match err {
        AnnexbError::BufferExceeded { max } => MpegPsError::BufferExceeded { max },
        AnnexbError::NalTooLarge { size, max } => MpegPsError::PacketTooLarge { size, max },
        AnnexbError::UnsupportedCodec => MpegPsError::UnsupportedVideoCodec,
        AnnexbError::InvalidInput => MpegPsError::InvalidInput,
    }
}
