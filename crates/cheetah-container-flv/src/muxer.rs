//! FLV muxer / writer.

use alloc::collections::BinaryHeap;
use alloc::vec::Vec;
use core::cmp::{Ordering, Reverse};

use cheetah_media_types::{CodecId, MediaPacket, TrackInfo, TrackKind};

use crate::{
    FlvError,
    audio::{build_aac_config, build_aac_raw_frame, build_audio_raw_frame},
    header::{TagType, tag_total_size},
    video::{build_video_config, build_video_frame},
};

/// A packet waiting in the muxer's reorder queue.
#[derive(Debug)]
struct QueuedPacket {
    packet: MediaPacket<'static>,
    track: TrackInfo,
}

impl PartialEq for QueuedPacket {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for QueuedPacket {}

impl PartialOrd for QueuedPacket {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedPacket {
    fn cmp(&self, other: &Self) -> Ordering {
        // Order by DTS, falling back to PTS.
        let a = self
            .packet
            .time
            .dts_ms()
            .or_else(|| self.packet.time.pts_ms());
        let b = other
            .packet
            .time
            .dts_ms()
            .or_else(|| other.packet.time.pts_ms());
        a.cmp(&b)
    }
}

/// FLV muxer that writes an FLV byte stream.
#[derive(Debug)]
pub struct FlvMuxer {
    /// True for file-mode output with PreviousTagSize fields; false for stream-mode.
    pub file_mode: bool,
    max_queue_depth: usize,
    output: Vec<u8>,
    audio: Option<TrackInfo>,
    video: Option<TrackInfo>,
    audio_config_emitted: bool,
    video_config_emitted: bool,
    queue: BinaryHeap<Reverse<QueuedPacket>>,
    header_written: bool,
    previous_tag_size: u32,
}

impl Default for FlvMuxer {
    fn default() -> Self {
        Self::new(true, 32)
    }
}

impl FlvMuxer {
    /// Create a muxer.
    pub fn new(file_mode: bool, max_queue_depth: usize) -> Self {
        Self {
            file_mode,
            max_queue_depth,
            output: Vec::new(),
            audio: None,
            video: None,
            audio_config_emitted: false,
            video_config_emitted: false,
            queue: BinaryHeap::new(),
            header_written: false,
            previous_tag_size: 0,
        }
    }

    /// Return a reference to the bytes written so far.
    pub fn output(&self) -> &[u8] {
        &self.output
    }

    /// Take the bytes written so far, leaving the muxer empty.
    pub fn take_output(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.output)
    }

    /// Number of packets still waiting in the reorder queue.
    pub(crate) fn pending_packet_count(&self) -> usize {
        self.queue.len()
    }

    /// Register a track. The header and config tags are emitted on the first
    /// packet or on `finish`, so all `add_track` calls can complete before the
    /// flags byte is written.
    pub fn add_track(&mut self, track: &TrackInfo) -> Result<(), FlvError> {
        self.ensure_track(track);
        Ok(())
    }

    /// Queue a packet for output. The muxer will sort by DTS and flush when the
    /// reorder queue exceeds `max_queue_depth`.
    pub fn push_packet(
        &mut self,
        packet: MediaPacket<'static>,
        track: &TrackInfo,
    ) -> Result<(), FlvError> {
        self.ensure_track(track);
        if !self.header_written {
            self.write_header();
        }
        self.write_config_for_track(track)?;

        self.queue.push(Reverse(QueuedPacket {
            packet,
            track: track.clone(),
        }));

        while self.queue.len() > self.max_queue_depth {
            self.flush_oldest()?;
        }

        Ok(())
    }

    /// Flush the reorder queue and return all remaining bytes.
    pub fn finish(mut self) -> Result<Vec<u8>, FlvError> {
        if !self.header_written {
            self.write_header();
        }
        if let Some(track) = self.audio.clone() {
            self.write_config_for_track(&track)?;
        }
        if let Some(track) = self.video.clone() {
            self.write_config_for_track(&track)?;
        }
        while self.queue.peek().is_some() {
            self.flush_oldest()?;
        }
        // Final previous tag size.
        if self.file_mode && self.previous_tag_size != 0 {
            self.output
                .extend_from_slice(&self.previous_tag_size.to_be_bytes());
        }
        Ok(self.output)
    }

    fn ensure_track(&mut self, track: &TrackInfo) {
        match track.kind {
            TrackKind::Audio if self.audio.is_none() => {
                self.audio = Some(track.clone());
                self.patch_header_flags();
            }
            TrackKind::Video if self.video.is_none() => {
                self.video = Some(track.clone());
                self.patch_header_flags();
            }
            _ => {}
        }
    }

    fn patch_header_flags(&mut self) {
        if !self.header_written {
            return;
        }
        let mut flags = 0u8;
        if self.audio.is_some() {
            flags |= 1 << 2;
        }
        if self.video.is_some() {
            flags |= 1;
        }
        if self.output.len() > 4 {
            self.output[4] = flags;
        }
    }

    fn write_config_for_track(&mut self, track: &TrackInfo) -> Result<(), FlvError> {
        match track.kind {
            TrackKind::Audio => {
                if self.audio_config_emitted {
                    return Ok(());
                }
                if track.codec == CodecId::Aac {
                    let body = build_aac_config(track)?;
                    self.write_tag(TagType::Audio, 0, &body)?;
                }
                self.audio_config_emitted = true;
            }
            TrackKind::Video => {
                if self.video_config_emitted {
                    return Ok(());
                }
                if matches!(track.codec, CodecId::H264 | CodecId::H265) {
                    let body = build_video_config(track)?;
                    self.write_tag(TagType::Video, 0, &body)?;
                }
                self.video_config_emitted = true;
            }
            TrackKind::Data => {}
        }
        Ok(())
    }

    fn write_header(&mut self) {
        if self.header_written {
            return;
        }
        let mut flags = 0u8;
        if self.audio.is_some() {
            flags |= 1 << 2;
        }
        if self.video.is_some() {
            flags |= 1;
        }
        self.output.extend_from_slice(b"FLV");
        self.output.push(1); // version
        self.output.push(flags);
        self.output.extend_from_slice(&9u32.to_be_bytes()); // header size
        if self.file_mode {
            self.output.extend_from_slice(&0u32.to_be_bytes()); // PreviousTagSize0
        }
        self.header_written = true;
    }

    fn flush_oldest(&mut self) -> Result<(), FlvError> {
        let Reverse(qp) = self.queue.pop().ok_or(FlvError::MalformedTag)?;
        self.write_packet(qp.packet, &qp.track)
    }

    fn write_packet(
        &mut self,
        packet: MediaPacket<'static>,
        track: &TrackInfo,
    ) -> Result<(), FlvError> {
        let dts_ms = packet
            .time
            .dts_ms()
            .or_else(|| packet.time.pts_ms())
            .unwrap_or(0);
        let pts_ms = packet.time.pts_ms().or(Some(dts_ms)).unwrap_or(0);

        match track.kind {
            TrackKind::Audio => self.write_audio_packet(&packet, track, dts_ms),
            TrackKind::Video => self.write_video_packet(&packet, track, dts_ms, pts_ms),
            TrackKind::Data => Ok(()),
        }
    }

    fn write_audio_packet(
        &mut self,
        packet: &MediaPacket<'static>,
        track: &TrackInfo,
        dts_ms: i64,
    ) -> Result<(), FlvError> {
        let payload = packet.payload.as_ref();
        let fmt = track.audio_format.ok_or(FlvError::UnsupportedCodec)?;
        let channels = fmt.channel_layout.channels() as u8;
        let body = match track.codec {
            CodecId::Aac => build_aac_raw_frame(track, payload),
            CodecId::Mp3 => build_audio_raw_frame(CodecId::Mp3, fmt.sample_rate, channels, payload),
            CodecId::G711A => build_audio_raw_frame(CodecId::G711A, 8000, channels, payload),
            CodecId::G711U => build_audio_raw_frame(CodecId::G711U, 8000, channels, payload),
            _ => return Err(FlvError::UnsupportedCodec),
        };
        self.write_tag(TagType::Audio, dts_ms as u32, &body)
    }

    fn write_video_packet(
        &mut self,
        packet: &MediaPacket<'static>,
        track: &TrackInfo,
        dts_ms: i64,
        pts_ms: i64,
    ) -> Result<(), FlvError> {
        let payload = packet.payload.as_ref();
        let cts = i32::try_from(pts_ms - dts_ms).map_err(|_| FlvError::InvalidTimestamp)?;
        let timestamp = dts_ms as u32;
        let body = build_video_frame(track.codec, packet.flags.is_keyframe, 1, cts, payload)?;
        self.write_tag(TagType::Video, timestamp, &body)
    }

    fn write_tag(
        &mut self,
        tag_type: TagType,
        timestamp: u32,
        data: &[u8],
    ) -> Result<(), FlvError> {
        let data_size = u32::try_from(data.len()).map_err(|_| FlvError::LimitExceeded)?;
        let total = tag_total_size(data_size);
        self.write_tag_header(tag_type, data_size, timestamp);
        self.output.extend_from_slice(data);
        if self.file_mode {
            self.output.extend_from_slice(&total.to_be_bytes());
        }
        self.previous_tag_size = total;
        Ok(())
    }

    fn write_tag_header(&mut self, tag_type: TagType, data_size: u32, timestamp: u32) {
        let type_byte: u8 = match tag_type {
            TagType::Audio => 8,
            TagType::Video => 9,
            TagType::Script => 18,
        };
        let ts_lower = timestamp & 0x00ff_ffff;
        let ts_extended = (timestamp >> 24) & 0xff;
        let stream_id = 0u32;

        self.output.push(type_byte);
        self.output.extend_from_slice(&data_size.to_be_bytes()[1..]); // 3 bytes
        self.output.extend_from_slice(&ts_lower.to_be_bytes()[1..]); // 3 bytes
        self.output.push(ts_extended as u8);
        self.output.extend_from_slice(&stream_id.to_be_bytes()[1..]); // 3 bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use cheetah_media_bitstream::aac::AudioSpecificConfig;
    use cheetah_media_bitstream::h264::H264CodecConfig;
    use cheetah_media_types::{
        AudioFormat, BufferRef, ChannelLayout, CodecConfig, CodecId, MediaPacket, MediaTime,
        PixelFormat, SampleFormat, SequenceNumber, StreamEpoch, TimeBase, TrackId, TrackInfo,
        TrackKind, VideoFormat,
    };

    use crate::demuxer::{FlvDemuxer, FlvEvent};

    fn make_video_track() -> TrackInfo {
        let sps = vec![0x67, 0x42, 0x00, 0x1e];
        let pps = vec![0x68, 0xce, 0x3c, 0x80];
        let config = H264CodecConfig {
            configuration_version: 1,
            avc_profile_indication: 0x42,
            profile_compatibility: 0x00,
            avc_level_indication: 0x1e,
            length_size_minus_one: 3,
            sps_list: vec![sps],
            pps_list: vec![pps],
            width: 320,
            height: 240,
            codec_string: alloc::string::String::new(),
        };
        let avcc = config.build();
        let mut track = TrackInfo::new(
            TrackId::new(2).unwrap(),
            TrackKind::Video,
            CodecId::H264,
            TimeBase::DEFAULT,
        );
        track.set_codec_config(CodecConfig::AvcC(avcc));
        track
            .set_video_format(VideoFormat {
                pixel_format: PixelFormat::Yuv420P,
                coded_width: 320,
                coded_height: 240,
                visible_width: 320,
                visible_height: 240,
                stride: 320,
                color_space: cheetah_media_types::ColorSpace::Unspecified,
            })
            .unwrap();
        track
    }

    fn make_audio_track() -> TrackInfo {
        let asc = AudioSpecificConfig {
            audio_object_type: 2,
            sampling_frequency_index: 4,
            sampling_frequency: 44100,
            channel_configuration: 2,
            channel_count: 2,
        };
        let mut track = TrackInfo::new(
            TrackId::new(1).unwrap(),
            TrackKind::Audio,
            CodecId::Aac,
            TimeBase::DEFAULT,
        );
        track.set_codec_config(CodecConfig::AacAudioSpecificConfig(asc.build()));
        track
            .set_audio_format(AudioFormat {
                sample_format: SampleFormat::S16,
                sample_rate: 44100,
                channel_layout: ChannelLayout::Stereo,
                sample_count: 1024,
            })
            .unwrap();
        track
    }

    #[test]
    fn demux_mux_demux_round_trip() {
        let video = make_video_track();
        let audio = make_audio_track();

        let mut muxer = FlvMuxer::new(true, 32);
        muxer.add_track(&video).unwrap();
        muxer.add_track(&audio).unwrap();

        // Video keyframe with positive CTS: PTS = DTS + 50.
        let video_payload = BufferRef::from_owned(vec![0x00, 0x00, 0x00, 0x02, 0x65, 0x88]);
        let vpacket = MediaPacket::new(
            video_payload,
            video.id,
            StreamEpoch::new(0),
            SequenceNumber::new(0),
            MediaTime::from_ticks(Some(150), Some(100), None, TimeBase::DEFAULT),
        )
        .with_keyframe();
        muxer.push_packet(vpacket, &video).unwrap();

        // Audio packet at DTS 120.
        let audio_payload = BufferRef::from_owned(vec![0x12, 0x34]);
        let apacket = MediaPacket::new(
            audio_payload,
            audio.id,
            StreamEpoch::new(0),
            SequenceNumber::new(1),
            MediaTime::from_ticks(Some(120), Some(120), None, TimeBase::DEFAULT),
        );
        muxer.push_packet(apacket, &audio).unwrap();

        let bytes = muxer.finish().unwrap();
        assert!(bytes.starts_with(b"FLV"));

        let mut demuxer = FlvDemuxer::default();
        demuxer.push(&bytes);

        let mut tracks = 0;
        let mut packets = 0;
        loop {
            match demuxer.next_event().unwrap() {
                Some(FlvEvent::Header(_)) => {}
                Some(FlvEvent::Track(_)) => tracks += 1,
                Some(FlvEvent::Packet(p)) => {
                    packets += 1;
                    if p.track_id == video.id {
                        assert!(p.flags.is_keyframe);
                        assert_eq!(p.time.pts_ms(), Some(150));
                        assert_eq!(p.time.dts_ms(), Some(100));
                        assert_eq!(p.payload.as_ref(), &[0x00, 0x00, 0x00, 0x02, 0x65, 0x88]);
                    } else if p.track_id == audio.id {
                        assert!(!p.flags.is_keyframe);
                        assert_eq!(p.time.pts_ms(), Some(120));
                        assert_eq!(p.payload.as_ref(), &[0x12, 0x34]);
                    } else {
                        panic!("unexpected track id");
                    }
                }
                Some(FlvEvent::Script(_)) => {}
                None => break,
            }
        }

        assert_eq!(tracks, 2);
        assert_eq!(packets, 2);
    }

    #[test]
    fn timestamp_wrap_is_unwrapped() {
        let video = make_video_track();
        let mut muxer = FlvMuxer::new(false, 32);
        muxer.add_track(&video).unwrap();

        // First packet near the end of the 32-bit timestamp counter.
        let near_max = (1i64 << 32) - 100;
        let p1 = MediaPacket::new(
            BufferRef::from_owned(vec![0x00, 0x00, 0x00, 0x02, 0x65, 0x88]),
            video.id,
            StreamEpoch::new(0),
            SequenceNumber::new(0),
            MediaTime::from_ticks(Some(near_max), Some(near_max), None, TimeBase::DEFAULT),
        )
        .with_keyframe();
        muxer.push_packet(p1, &video).unwrap();

        // Second packet has wrapped to a small raw value; the absolute DTS already
        // reflects the new round so the muxer preserves order and the FLV timestamp
        // is the low 32 bits.
        let p2_abs = (1i64 << 32) + 50;
        let p2 = MediaPacket::new(
            BufferRef::from_owned(vec![0x00, 0x00, 0x00, 0x02, 0x41, 0x88]),
            video.id,
            StreamEpoch::new(0),
            SequenceNumber::new(1),
            MediaTime::from_ticks(Some(p2_abs), Some(p2_abs), None, TimeBase::DEFAULT),
        );
        muxer.push_packet(p2, &video).unwrap();

        let bytes = muxer.finish().unwrap();

        let mut demuxer = FlvDemuxer::default();
        demuxer.push(&bytes);
        let mut p2_dts = None;
        while let Some(ev) = demuxer.next_event().unwrap() {
            if let FlvEvent::Packet(p) = ev
                && p.time.dts_ms() > Some(near_max)
            {
                p2_dts = p.time.dts_ms();
                break;
            }
        }
        assert_eq!(p2_dts, Some((1i64 << 32) + 50));
    }
}
