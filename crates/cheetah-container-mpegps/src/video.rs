//! Video elementary stream assembly for MPEG-PS.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use cheetah_container_annexb::{
    find_start_code as find_annexb_start_code, param_sets::ParameterSetCache,
};
use cheetah_media_types::{
    CodecId, MediaPacket, MediaTime, PacketFlags, SequenceNumber, StreamEpoch, TimeBase, TrackId,
    TrackInfo, TrackKind,
};

use crate::MpegPsError;
use crate::types::{MpegPsConfig, MpegPsEvent, VIDEO_TRACK_ID};

/// Assembler that builds video NAL units from PES payloads and emits
/// timestamped `MediaPacket`s.
#[derive(Debug)]
pub(crate) struct VideoEsAssembler {
    config: MpegPsConfig,
    track: Option<TrackInfo>,
    param_sets: Option<ParameterSetCache>,
    es_buffer: Vec<u8>,
    es_chunks: Vec<(usize, MediaTime)>,
    es_base_offset: usize,
    sequence: u64,
}

impl VideoEsAssembler {
    pub(crate) fn new(config: MpegPsConfig) -> Self {
        Self {
            config,
            track: None,
            param_sets: None,
            es_buffer: Vec::new(),
            es_chunks: Vec::new(),
            es_base_offset: 0,
            sequence: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.track = None;
        self.param_sets = None;
        self.es_buffer.clear();
        self.es_chunks.clear();
        self.es_base_offset = 0;
        self.sequence = 0;
    }

    pub(crate) fn process_payload(
        &mut self,
        payload: &[u8],
        media_time: MediaTime,
        events: &mut VecDeque<MpegPsEvent>,
    ) -> Result<(), MpegPsError> {
        if self.track.is_none() {
            self.init_track()?;
        }

        if !payload.is_empty() {
            let offset = self.es_base_offset + self.es_buffer.len();
            self.es_buffer.extend_from_slice(payload);
            self.es_chunks.push((offset, media_time));
        }

        self.slice_es(events)
    }

    pub(crate) fn flush(&mut self, events: &mut VecDeque<MpegPsEvent>) -> Result<(), MpegPsError> {
        self.slice_es(events)?;

        if self.es_buffer.len() >= 4
            && let Some((start_pos, code_len)) = find_annexb_start_code(&self.es_buffer, 0)
        {
            let header_pos = start_pos + code_len;
            if header_pos < self.es_buffer.len() {
                let nal = self.es_buffer[header_pos..].to_vec();
                if nal.len() > self.config.max_nal_size_bytes {
                    return Err(MpegPsError::PacketTooLarge {
                        size: nal.len(),
                        max: self.config.max_nal_size_bytes,
                    });
                }
                let nal_start_abs = self.es_base_offset + header_pos;
                let media_time = self
                    .media_time_for_es_offset(nal_start_abs)
                    .unwrap_or(MediaTime::new(None, None, None, TimeBase::TS_90K));
                self.emit_nal(nal, media_time, events)?;
            }
        }

        self.es_buffer.clear();
        self.es_chunks.clear();
        self.es_base_offset = 0;
        Ok(())
    }

    fn init_track(&mut self) -> Result<(), MpegPsError> {
        if !matches!(self.config.video_codec, CodecId::H264 | CodecId::H265) {
            return Err(MpegPsError::UnsupportedVideoCodec);
        }

        let track_id = TrackId::new(VIDEO_TRACK_ID).ok_or(MpegPsError::InvalidInput)?;
        let track = TrackInfo::new(
            track_id,
            TrackKind::Video,
            self.config.video_codec,
            TimeBase::TS_90K,
        );
        self.track = Some(track);
        self.param_sets = Some(ParameterSetCache::new(self.config.video_codec));
        Ok(())
    }

    fn slice_es(&mut self, events: &mut VecDeque<MpegPsEvent>) -> Result<(), MpegPsError> {
        loop {
            if self.es_buffer.len() < 4 {
                break;
            }

            let (start_pos, code_len) = match find_annexb_start_code(&self.es_buffer, 0) {
                Some(v) => v,
                None => {
                    let keep = self.es_buffer.len().saturating_sub(2);
                    if keep > 0 {
                        self.es_base_offset += keep;
                        self.es_buffer.drain(0..keep);
                        self.prune_chunks();
                    }
                    break;
                }
            };

            if start_pos > 0 {
                self.es_base_offset += start_pos;
                self.es_buffer.drain(0..start_pos);
                self.prune_chunks();
                continue;
            }

            let header_pos = code_len;
            if header_pos >= self.es_buffer.len() {
                break;
            }

            let (next_start, _) = match find_annexb_start_code(&self.es_buffer, header_pos) {
                Some(v) => v,
                None => break,
            };

            let nal_size = next_start.saturating_sub(header_pos);
            if nal_size > self.config.max_nal_size_bytes {
                return Err(MpegPsError::PacketTooLarge {
                    size: nal_size,
                    max: self.config.max_nal_size_bytes,
                });
            }

            let nal = self.es_buffer[header_pos..next_start].to_vec();
            let nal_start_abs = self.es_base_offset + header_pos;
            let media_time = self
                .media_time_for_es_offset(nal_start_abs)
                .unwrap_or(MediaTime::new(None, None, None, TimeBase::TS_90K));
            self.emit_nal(nal, media_time, events)?;

            self.es_base_offset += next_start;
            self.es_buffer.drain(0..next_start);
            self.prune_chunks();
        }

        if self.es_buffer.len() > self.config.max_buffer_bytes {
            return Err(MpegPsError::BufferExceeded {
                max: self.config.max_buffer_bytes,
            });
        }
        Ok(())
    }

    fn media_time_for_es_offset(&self, offset: usize) -> Option<MediaTime> {
        self.es_chunks
            .iter()
            .rfind(|(o, _)| *o <= offset)
            .map(|(_, t)| *t)
    }

    fn prune_chunks(&mut self) {
        if let Some(idx) = self
            .es_chunks
            .iter()
            .rposition(|(o, _)| *o <= self.es_base_offset)
        {
            self.es_chunks.drain(0..idx);
        }
    }

    fn emit_nal(
        &mut self,
        nal: Vec<u8>,
        media_time: MediaTime,
        events: &mut VecDeque<MpegPsEvent>,
    ) -> Result<(), MpegPsError> {
        if nal.is_empty() {
            return Ok(());
        }

        if let Some(ref mut cache) = self.param_sets
            && cache.consume(&nal)
        {
            if cache.is_complete()
                && let Some(ref mut track) = self.track
            {
                let old_config = track.codec_config.clone();
                let old_format = track.video_format;
                cache.update_track(track);
                if track.codec_config != old_config || track.video_format != old_format {
                    events.push_back(MpegPsEvent::Track(track.clone()));
                }
            }
            return Ok(());
        }

        let is_keyframe = self.is_keyframe(&nal);
        let flags = PacketFlags {
            is_keyframe,
            is_corrupt: false,
            is_discontinuity: false,
        };
        let track_id = self
            .track
            .as_ref()
            .map(|t| t.id)
            .unwrap_or(TrackId::new(VIDEO_TRACK_ID).ok_or(MpegPsError::InvalidInput)?);

        let mut packet = MediaPacket::new(
            nal,
            track_id,
            StreamEpoch::new(0),
            SequenceNumber::new(self.sequence),
            media_time,
        );
        packet.flags = flags;
        self.sequence += 1;
        events.push_back(MpegPsEvent::Packet(packet));
        Ok(())
    }

    fn is_keyframe(&self, nal: &[u8]) -> bool {
        match self.config.video_codec {
            CodecId::H264 => !nal.is_empty() && (nal[0] & 0x1f) == 5,
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

    #[cfg(test)]
    pub(crate) fn chunks_len(&self) -> usize {
        self.es_chunks.len()
    }
}
