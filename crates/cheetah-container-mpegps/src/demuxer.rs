//! MPEG-2 Program Stream demuxer.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use cheetah_media_types::{MediaTime, MetadataItem, TimeBase};

use crate::error::MpegPsError;
use crate::pack::{is_pack_start_code, is_system_start_code, parse_pack_header};
use crate::pes::{is_audio_stream, is_video_stream, parse_pes_header};
use crate::scan::{find_ps_boundary, find_start_code};
pub use crate::types::{MpegPsConfig, MpegPsEvent};

/// Incremental MPEG-2 Program Stream demuxer.
#[derive(Debug)]
pub struct MpegPsDemuxer {
    config: MpegPsConfig,
    buffer: Vec<u8>,
    pending_events: VecDeque<MpegPsEvent>,
    video: crate::video::VideoEsAssembler,
    audio: crate::audio::AudioAssembler,
    ended: bool,
    eof_emitted: bool,
    error: Option<MpegPsError>,
}

impl MpegPsDemuxer {
    /// Create a new demuxer with the given configuration.
    pub fn new(config: MpegPsConfig) -> Self {
        Self {
            config,
            buffer: Vec::new(),
            pending_events: VecDeque::new(),
            video: crate::video::VideoEsAssembler::new(config),
            audio: crate::audio::AudioAssembler::new(config),
            ended: false,
            eof_emitted: false,
            error: None,
        }
    }

    /// Push more MPEG-PS bytes into the demuxer.
    pub fn push(&mut self, data: &[u8]) {
        if data.is_empty() || self.error.is_some() {
            return;
        }
        if self.buffer.len().saturating_add(data.len()) > self.config.max_buffer_bytes {
            self.error = Some(MpegPsError::BufferExceeded {
                max: self.config.max_buffer_bytes,
            });
            return;
        }
        self.buffer.extend_from_slice(data);
    }

    /// Return the next parsed event, or `None` if more data is needed.
    pub fn next_event(&mut self) -> Result<Option<MpegPsEvent>, MpegPsError> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(Some(event));
        }

        if let Some(err) = self.error {
            return Err(err);
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
            self.video.flush(&mut self.pending_events)?;
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
        self.video.reset();
        self.audio.reset();
        self.ended = false;
        self.eof_emitted = false;
        self.error = None;
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
            match parse_pack_header(&self.buffer) {
                Ok(header) => {
                    self.buffer.drain(0..header.end_offset);
                    Ok(true)
                }
                Err(MpegPsError::NeedMoreData) => Ok(false),
                Err(e) => Err(e),
            }
        } else if is_system_start_code(code) {
            self.skip_system_packet()
        } else if is_video_stream(code) || is_audio_stream(code) || is_private_stream(code) {
            self.process_pes(code)
        } else {
            // Unrecognized start code; skip it and continue.
            self.buffer.drain(0..4);
            Ok(true)
        }
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
                    if self.ended && self.buffer.len() >= 6 {
                        self.buffer.len()
                    } else {
                        return Ok(false);
                    }
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

        // Private/reserved stream ids (0xBF, 0xF0-0xFE) do not carry the standard
        // PES optional header; the payload follows PES_packet_length directly.
        if is_private_stream(code) {
            if pes_end > 6 {
                let payload = self.buffer[6..pes_end].to_vec();
                let item = MetadataItem::pes_private(u32::from(code), payload);
                self.pending_events
                    .push_back(MpegPsEvent::Metadata(alloc::vec![item]));
            }
            self.buffer.drain(0..pes_end);
            return Ok(true);
        }

        if pes_end < 9 {
            self.buffer.drain(0..pes_end);
            return Ok(true);
        }

        let header = match parse_pes_header(&self.buffer[..pes_end]) {
            Ok(h) => h,
            Err(MpegPsError::NeedMoreData) => {
                // A full bounded PES or an unbounded PES whose end is known cannot
                // legitimately need more data for its own header. Skip it.
                self.buffer.drain(0..pes_end);
                return Ok(true);
            }
            Err(e) => return Err(e),
        };

        debug_assert!(header.header_size <= pes_end);

        let payload = self.buffer[header.header_size..pes_end].to_vec();
        let media_time = MediaTime::new(header.pts, header.dts, None, TimeBase::TS_90K);

        if is_video_stream(code) {
            self.video
                .process_payload(&payload, media_time, &mut self.pending_events)?;
        } else if is_audio_stream(code) {
            self.audio
                .process_payload(&payload, media_time, &mut self.pending_events)?;
        }

        self.buffer.drain(0..pes_end);
        Ok(true)
    }
}

/// True if `stream_id` indicates a private PES stream.
///
/// 0xBD (private_stream_1) is excluded here because many existing streams use it
/// for audio (AC3/AAC); those continue to be routed through the audio path.
/// 0xBF (private_stream_2) and the reserved 0xF0-0xFE range are treated as
/// generic private data/metadata.
const fn is_private_stream(stream_id: u8) -> bool {
    stream_id == 0xBF || (stream_id >= 0xF0 && stream_id <= 0xFE)
}

#[cfg(test)]
impl MpegPsDemuxer {
    pub(crate) fn video_es_chunks_len(&self) -> usize {
        self.video.chunks_len()
    }
}
