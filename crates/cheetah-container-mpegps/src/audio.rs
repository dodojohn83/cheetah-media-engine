//! Audio elementary stream assembly for MPEG-PS.

use alloc::collections::VecDeque;

use cheetah_media_bitstream::aac::{AacError, AdtsHeader, AudioSpecificConfig};
use cheetah_media_types::{
    AudioFormat, ChannelLayout, CodecConfig, CodecId, MediaPacket, MediaTime, PacketFlags,
    SampleFormat, SequenceNumber, StreamEpoch, TimeBase, Timestamp, TrackId, TrackInfo, TrackKind,
};

use crate::MpegPsError;
use crate::types::{AUDIO_TRACK_ID, MpegPsConfig, MpegPsEvent};

/// Assembler that extracts AAC ADTS frames from audio PES payloads and emits
/// timestamped `MediaPacket`s.
#[derive(Debug)]
pub(crate) struct AudioAssembler {
    config: MpegPsConfig,
    track: Option<TrackInfo>,
    sequence: u64,
    leftover: Vec<u8>,
    /// Timestamp for the first access unit that begins in `leftover`.
    leftover_time: Option<MediaTime>,
}

impl AudioAssembler {
    pub(crate) fn new(config: MpegPsConfig) -> Self {
        Self {
            config,
            track: None,
            sequence: 0,
            leftover: Vec::new(),
            leftover_time: None,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.track = None;
        self.sequence = 0;
        self.leftover.clear();
        self.leftover_time = None;
    }

    pub(crate) fn process_payload(
        &mut self,
        payload: &[u8],
        media_time: MediaTime,
        events: &mut VecDeque<MpegPsEvent>,
    ) -> Result<(), MpegPsError> {
        let track_id = TrackId::new(AUDIO_TRACK_ID).ok_or(MpegPsError::InvalidInput)?;

        // Audio PES payloads can be split across packet boundaries, so keep any
        // trailing partial ADTS frame for the next payload.  On non-AAC or
        // corrupt audio streams we must not retain garbage indefinitely.
        let leftover_len = self.leftover.len();
        let leftover_time = self.leftover_time;
        let mut data = Vec::with_capacity(leftover_len + payload.len());
        data.extend_from_slice(&self.leftover);
        data.extend_from_slice(payload);
        self.leftover.clear();
        self.leftover_time = None;

        let mut offset = 0;
        let mut pts = leftover_time.map_or(media_time.pts, |t| t.pts);
        let mut dts = leftover_time.map_or(media_time.dts, |t| t.dts);
        let mut switched_to_media_time = leftover_time.is_none();
        let mut retain_remaining = false;
        while offset < data.len() {
            if !switched_to_media_time && offset >= leftover_len {
                pts = media_time.pts;
                dts = media_time.dts;
                switched_to_media_time = true;
            }
            let header = match AdtsHeader::parse(&data[offset..]) {
                Ok(h) => h,
                Err(AacError::TooShort) => {
                    retain_remaining = true;
                    break;
                }
                Err(_) => {
                    // Non-ADTS audio or a corrupted syncword; drop the rest
                    // of this payload and do not carry it forward.
                    offset = data.len();
                    break;
                }
            };
            let frame_len = header.frame_length as usize;
            if frame_len == 0 || data.len() - offset < frame_len {
                retain_remaining = true;
                break;
            }

            if self.track.is_none() {
                let track = self.build_track(&header, track_id)?;
                self.track = Some(track.clone());
                events.push_back(MpegPsEvent::Track(track));
            }

            let frame = &data[offset..offset + frame_len];
            let flags = PacketFlags {
                is_keyframe: true,
                is_corrupt: false,
                is_discontinuity: false,
            };
            let duration_ticks =
                u64::from(header.samples_per_frame) * 90_000 / u64::from(header.sampling_frequency);
            let duration_ticks = i64::try_from(duration_ticks).unwrap_or(i64::MAX);
            let packet_time = MediaTime::new(
                pts,
                dts,
                Some(Timestamp::new(duration_ticks)),
                TimeBase::TS_90K,
            );
            let mut packet = MediaPacket::new(
                frame.to_vec(),
                track_id,
                StreamEpoch::new(0),
                SequenceNumber::new(self.sequence),
                packet_time,
            );
            packet.flags = flags;
            self.sequence = self.sequence.wrapping_add(1);
            events.push_back(MpegPsEvent::Packet(packet));

            offset += frame_len;
            pts = pts.map(|p| Timestamp::new(p.ticks().saturating_add(duration_ticks)));
            dts = dts.map(|d| Timestamp::new(d.ticks().saturating_add(duration_ticks)));
        }

        if retain_remaining && offset < data.len() {
            let remaining = data.len() - offset;
            if remaining > self.config.max_buffer_bytes {
                self.leftover.clear();
                self.leftover_time = None;
                return Err(MpegPsError::BufferExceeded {
                    max: self.config.max_buffer_bytes,
                });
            }
            self.leftover.extend_from_slice(&data[offset..]);
            self.leftover_time = Some(MediaTime::new(pts, dts, None, media_time.timebase));
        }
        Ok(())
    }

    fn build_track(
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

        let channel_layout = ChannelLayout::from_channel_count(u32::from(header.channel_count));
        let audio_format = AudioFormat {
            sample_format: SampleFormat::S16,
            sample_rate: header.sampling_frequency,
            channel_layout,
            sample_count: u32::from(header.samples_per_frame),
        };

        let mut track = TrackInfo::new(track_id, TrackKind::Audio, CodecId::Aac, TimeBase::TS_90K);
        track.set_codec_config(CodecConfig::AacAudioSpecificConfig(config_bytes));
        track
            .set_audio_format(audio_format)
            .map_err(|_| MpegPsError::InvalidInput)?;
        Ok(track)
    }
}
