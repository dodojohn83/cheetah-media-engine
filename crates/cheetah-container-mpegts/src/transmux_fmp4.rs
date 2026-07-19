//! MPEG-TS → fragmented MP4 transmuxer for MSE / HLS-TS playback.
//!
//! H.264 access units from the demuxer are Annex-B; they are converted to
//! length-prefixed AVCC samples before muxing. AAC ADTS frames are stripped to
//! raw AAC for `mp4a` samples.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use cheetah_container_isobmff::{FragmentedMp4Muxer, SegmentOutput, TrackConfig, boxes::types};
use cheetah_media_bitstream::aac::AdtsHeader;
use cheetah_media_bitstream::h264;
use cheetah_media_bitstream::h265;
use cheetah_media_types::{BufferRef, CodecConfig, CodecId, MediaPacket, TrackInfo, TrackKind};

use crate::{TsDemuxer, TsError, TsEvent};

/// Errors produced while transmuxing MPEG-TS into fMP4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TsTransmuxError {
    /// Underlying TS demux failed.
    Ts(TsError),
    /// Muxer rejected a packet or track config.
    Mux(u32),
    /// Unsupported codec for MSE fMP4.
    UnsupportedCodec,
    /// Bitstream conversion failed (Annex-B / ADTS).
    Bitstream,
}

impl core::fmt::Display for TsTransmuxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ts(e) => write!(f, "ts demux error: {e}"),
            Self::Mux(code) => write!(f, "fmp4 mux error code={code}"),
            Self::UnsupportedCodec => write!(f, "unsupported codec for ts→fmp4 transmux"),
            Self::Bitstream => write!(f, "bitstream conversion failed"),
        }
    }
}

/// Incremental MPEG-TS → fMP4 transmuxer.
#[derive(Debug)]
pub struct TsToFmp4Transmuxer {
    demuxer: TsDemuxer,
    muxer: FragmentedMp4Muxer,
    configured: BTreeMap<u32, TrackConfig>,
    track_codecs: BTreeMap<u32, CodecId>,
    pending: VecDeque<SegmentOutput>,
    samples_since_flush: usize,
    max_samples_per_segment: usize,
    finished: bool,
    error: Option<TsTransmuxError>,
}

impl Default for TsToFmp4Transmuxer {
    fn default() -> Self {
        Self::new()
    }
}

impl TsToFmp4Transmuxer {
    /// Create a new transmuxer.
    pub fn new() -> Self {
        Self {
            demuxer: TsDemuxer::new(),
            muxer: FragmentedMp4Muxer::new(),
            configured: BTreeMap::new(),
            track_codecs: BTreeMap::new(),
            pending: VecDeque::new(),
            samples_since_flush: 0,
            max_samples_per_segment: 60,
            finished: false,
            error: None,
        }
    }

    /// Soft sample budget before forcing a media flush.
    pub fn set_max_samples_per_segment(&mut self, n: usize) {
        self.max_samples_per_segment = n.max(1);
    }

    /// Push additional MPEG-TS bytes.
    pub fn push(&mut self, data: &[u8]) -> Result<(), TsTransmuxError> {
        if let Some(err) = self.error.clone() {
            return Err(err);
        }
        if self.finished {
            return Ok(());
        }
        self.demuxer.push(data);
        self.drain_demux()?;
        Ok(())
    }

    /// Signal end of input and flush remaining samples.
    pub fn finish(&mut self) -> Result<(), TsTransmuxError> {
        if let Some(err) = self.error.clone() {
            return Err(err);
        }
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        self.drain_demux()?;
        self.flush_mux(true)?;
        Ok(())
    }

    /// Pop the next ready fMP4 segment.
    pub fn poll(&mut self) -> Option<SegmentOutput> {
        self.pending.pop_front()
    }

    fn drain_demux(&mut self) -> Result<(), TsTransmuxError> {
        loop {
            match self.demuxer.next_event() {
                Ok(Some(TsEvent::Track(info))) => {
                    self.configure_track(&info)?;
                }
                Ok(Some(TsEvent::Packet(packet))) => {
                    self.push_packet(packet)?;
                }
                Ok(Some(TsEvent::Metadata(_))) | Ok(Some(TsEvent::Clock(_))) => {}
                Ok(None) => return Ok(()),
                Err(TsError::NeedMoreData) => return Ok(()),
                Err(e) => {
                    let err = TsTransmuxError::Ts(e);
                    self.error = Some(err.clone());
                    return Err(err);
                }
            }
        }
    }

    fn configure_track(&mut self, info: &TrackInfo) -> Result<(), TsTransmuxError> {
        // Tracks may be announced before parameter sets arrive; wait for a
        // full decoder config rather than failing the stream.
        let Ok(cfg) = track_info_to_config(info) else {
            return Ok(());
        };
        let id = cfg.track_id;
        let changed = self
            .configured
            .get(&id)
            .map(|prev| {
                prev.codec_config != cfg.codec_config
                    || prev.width != cfg.width
                    || prev.height != cfg.height
                    || prev.sample_rate != cfg.sample_rate
            })
            .unwrap_or(true);
        if changed {
            self.muxer.configure(cfg.clone());
            self.configured.insert(id, cfg);
            self.track_codecs.insert(id, info.codec);
        }
        Ok(())
    }

    fn push_packet(&mut self, packet: MediaPacket<'static>) -> Result<(), TsTransmuxError> {
        let track_id = packet.track_id.get();
        if !self.configured.contains_key(&track_id) {
            return Ok(());
        }
        let codec = self
            .track_codecs
            .get(&track_id)
            .copied()
            .unwrap_or(CodecId::Unknown(0));
        let converted = convert_packet_for_fmp4(packet, codec)?;
        let is_key = converted.flags.is_keyframe;
        self.muxer.push_packet(converted).map_err(|e| {
            let err = TsTransmuxError::Mux(mp4_error_code(&e));
            self.error = Some(err.clone());
            err
        })?;
        self.samples_since_flush = self.samples_since_flush.saturating_add(1);
        if (is_key && self.samples_since_flush > 1)
            || self.samples_since_flush >= self.max_samples_per_segment
        {
            self.flush_mux(false)?;
        }
        Ok(())
    }

    fn flush_mux(&mut self, force: bool) -> Result<(), TsTransmuxError> {
        if self.samples_since_flush == 0 && !force {
            return Ok(());
        }
        match self.muxer.flush_segment() {
            Ok(Some(seg)) => {
                self.samples_since_flush = 0;
                self.pending.push_back(seg);
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(e) => {
                let err = TsTransmuxError::Mux(mp4_error_code(&e));
                self.error = Some(err.clone());
                Err(err)
            }
        }
    }
}

fn convert_packet_for_fmp4(
    packet: MediaPacket<'static>,
    codec: CodecId,
) -> Result<MediaPacket<'static>, TsTransmuxError> {
    let raw = packet.payload.as_ref();
    let payload: Vec<u8> = match codec {
        CodecId::H264 => {
            if looks_like_annexb(raw) {
                h264::annexb_to_avcc(raw, 4).map_err(|_| TsTransmuxError::Bitstream)?
            } else {
                raw.to_vec()
            }
        }
        CodecId::H265 => {
            if looks_like_annexb(raw) {
                h265::annexb_to_hvcc(raw, 4).map_err(|_| TsTransmuxError::Bitstream)?
            } else {
                raw.to_vec()
            }
        }
        CodecId::Aac => strip_adts_if_present(raw),
        _ => return Err(TsTransmuxError::UnsupportedCodec),
    };

    let mut out = MediaPacket::new(
        BufferRef::from_owned(payload),
        packet.track_id,
        packet.stream_epoch,
        packet.sequence,
        packet.time,
    );
    out.flags = packet.flags;
    Ok(out)
}

fn looks_like_annexb(data: &[u8]) -> bool {
    data.len() >= 4
        && ((data[0] == 0 && data[1] == 0 && data[2] == 0 && data[3] == 1)
            || (data[0] == 0 && data[1] == 0 && data[2] == 1))
}

fn strip_adts_if_present(data: &[u8]) -> Vec<u8> {
    match AdtsHeader::parse(data) {
        Ok(h) if h.header_size() < data.len() => data[h.header_size()..].to_vec(),
        _ => data.to_vec(),
    }
}

fn mp4_error_code(e: &cheetah_container_isobmff::Mp4Error) -> u32 {
    match e {
        cheetah_container_isobmff::Mp4Error::NeedMoreData => 3500,
        cheetah_container_isobmff::Mp4Error::InvalidInput { code, .. } => *code,
        cheetah_container_isobmff::Mp4Error::LimitExceeded { .. } => 3599,
        cheetah_container_isobmff::Mp4Error::Unsupported { code, .. } => *code,
    }
}

fn track_info_to_config(info: &TrackInfo) -> Result<TrackConfig, TsTransmuxError> {
    let track_id = info.id.get();
    match info.kind {
        TrackKind::Video => {
            let (width, height) = info
                .video_format
                .map(|vf| {
                    (
                        u16::try_from(vf.visible_width).unwrap_or(0),
                        u16::try_from(vf.visible_height).unwrap_or(0),
                    )
                })
                .unwrap_or((0, 0));
            let sample_entry_type = match info.codec {
                CodecId::H264 => types::AVC1,
                CodecId::H265 => types::HVC1,
                _ => return Err(TsTransmuxError::UnsupportedCodec),
            };
            let codec_config = match &info.codec_config {
                CodecConfig::AvcC(_) | CodecConfig::HevcC(_) => info.codec_config.clone(),
                _ => return Err(TsTransmuxError::UnsupportedCodec),
            };
            // MPEG-TS timestamps are 90 kHz.
            Ok(TrackConfig {
                track_id,
                kind: TrackKind::Video,
                codec: info.codec,
                codec_config,
                timescale: 90_000,
                sample_entry_type,
                width: if width == 0 { 640 } else { width },
                height: if height == 0 { 360 } else { height },
                sample_rate: 0,
                channel_count: 0,
                default_sample_duration: 3000,
            })
        }
        TrackKind::Audio => {
            let (sample_rate, channels) = info
                .audio_format
                .map(|af| (af.sample_rate, af.channel_layout.channels() as u16))
                .unwrap_or((44_100, 2));
            match info.codec {
                CodecId::Aac => match &info.codec_config {
                    CodecConfig::AacAudioSpecificConfig(bytes) => Ok(TrackConfig {
                        track_id,
                        kind: TrackKind::Audio,
                        codec: CodecId::Aac,
                        codec_config: CodecConfig::AacAudioSpecificConfig(bytes.clone()),
                        timescale: sample_rate.max(1),
                        sample_entry_type: types::MP4A,
                        width: 0,
                        height: 0,
                        sample_rate: sample_rate.max(1),
                        channel_count: channels.max(1),
                        default_sample_duration: 1024,
                    }),
                    _ => Err(TsTransmuxError::UnsupportedCodec),
                },
                _ => Err(TsTransmuxError::UnsupportedCodec),
            }
        }
        TrackKind::Data => Err(TsTransmuxError::UnsupportedCodec),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_container_isobmff::{IsobmffDemuxer, Mp4Event};

    fn ts_packet(pid: u16, payload_unit_start: bool, payload: &[u8], cc: u8) -> Vec<u8> {
        let mut pkt = vec![0xff; 188];
        pkt[0] = 0x47;
        let pid_hi = (((pid >> 8) & 0x1f) as u8) | if payload_unit_start { 0x40 } else { 0 };
        pkt[1] = pid_hi;
        pkt[2] = (pid & 0xff) as u8;
        pkt[3] = (1 << 4) | (cc & 0x0f);
        let len = payload.len().min(184);
        pkt[4..4 + len].copy_from_slice(&payload[..len]);
        pkt
    }

    fn pat_section(programs: &[(u16, u16)]) -> Vec<u8> {
        let n = programs.len();
        let section_length = 9 + 4 * n;
        let mut s = vec![0u8; section_length + 3];
        s[0] = 0x00;
        s[1] = 0xb0 | ((section_length >> 8) & 0x0f) as u8;
        s[2] = (section_length & 0xff) as u8;
        s[3..5].copy_from_slice(&[0x00, 0x01]);
        s[5] = 0xc1;
        s[6] = 0x00;
        s[7] = 0x00;
        for (i, (pn, pmt_pid)) in programs.iter().enumerate() {
            let off = 8 + i * 4;
            s[off..off + 2].copy_from_slice(&pn.to_be_bytes());
            s[off + 2..off + 4].copy_from_slice(&(0xe000u16 | pmt_pid).to_be_bytes());
        }
        s
    }

    fn pmt_section(program_number: u16, pcr_pid: u16, streams: &[(u8, u16)]) -> Vec<u8> {
        let stream_bytes = 5 * streams.len();
        let section_length = 13 + stream_bytes;
        let mut s = vec![0u8; section_length + 3];
        s[0] = 0x02;
        s[1] = 0xb0 | ((section_length >> 8) & 0x0f) as u8;
        s[2] = (section_length & 0xff) as u8;
        s[3..5].copy_from_slice(&program_number.to_be_bytes());
        s[5] = 0xc1;
        s[6] = 0x00;
        s[7] = 0x00;
        s[8..10].copy_from_slice(&(0xe000u16 | pcr_pid).to_be_bytes());
        s[10..12].copy_from_slice(&0xf000u16.to_be_bytes());
        let mut off = 12;
        for (st, pid) in streams {
            s[off] = *st;
            s[off + 1..off + 3].copy_from_slice(&(0xe000u16 | pid).to_be_bytes());
            s[off + 3..off + 5].copy_from_slice(&0xf000u16.to_be_bytes());
            off += 5;
        }
        s
    }

    fn timestamp_bytes(ts: u64, nibble: u8) -> [u8; 5] {
        let b0 = ((nibble & 0x0f) << 4) | ((((ts >> 30) & 0x07) as u8) << 1) | 1;
        let b1 = ((ts >> 22) & 0xff) as u8;
        let b2 = ((((ts >> 15) & 0x7f) as u8) << 1) | 1;
        let b3 = ((ts >> 7) & 0xff) as u8;
        let b4 = (((ts & 0x7f) as u8) << 1) | 1;
        [b0, b1, b2, b3, b4]
    }

    fn pes_packet(stream_id: u8, payload: &[u8], pts: Option<u64>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[0x00, 0x00, 0x01, stream_id]);
        let mut optional = Vec::new();
        let mut flags2 = 0u8;
        if let Some(p) = pts {
            optional.extend_from_slice(&timestamp_bytes(p, 0x02));
            flags2 = 0x80;
        }
        let header_data_length = optional.len() as u16;
        let packet_length = (3 + header_data_length as usize + payload.len()) as u16;
        out.extend_from_slice(&packet_length.to_be_bytes());
        out.push(0x80);
        out.push(flags2);
        out.push(header_data_length as u8);
        out.extend_from_slice(&optional);
        out.extend_from_slice(payload);
        out
    }

    fn build_h264_es() -> Vec<u8> {
        let sps = [
            0x67u8, 0x42, 0x00, 0x1e, 0xe9, 0x42, 0x10, 0x89, 0xf3, 0x22, 0xcb, 0x80,
        ];
        let pps = [0x68u8, 0xce, 0x3c, 0x80];
        let mut es = Vec::new();
        es.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        es.extend_from_slice(&sps);
        es.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        es.extend_from_slice(&pps);
        es.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        es.push(0x65);
        es.extend_from_slice(&[0x88; 32]);
        es
    }

    fn build_test_stream() -> Vec<u8> {
        let mut stream = Vec::new();
        let mut pat_payload = vec![0x00];
        pat_payload.extend_from_slice(&pat_section(&[(1, 0x100)]));
        stream.extend_from_slice(&ts_packet(0x000, true, &pat_payload, 0));
        let mut pmt_payload = vec![0x00];
        pmt_payload.extend_from_slice(&pmt_section(1, 0x102, &[(0x1b, 0x101)]));
        stream.extend_from_slice(&ts_packet(0x100, true, &pmt_payload, 0));
        let es = build_h264_es();
        let pes = pes_packet(0xe0, &es, Some(90_000));
        // May span multiple packets
        let mut cc = 0u8;
        let mut offset = 0;
        let mut first = true;
        while offset < pes.len() {
            let take = (pes.len() - offset).min(184);
            stream.extend_from_slice(&ts_packet(0x101, first, &pes[offset..offset + take], cc));
            first = false;
            cc = (cc + 1) & 0x0f;
            offset += take;
        }
        // Second keyframe-ish AU so flush has a cut point
        let pes2 = pes_packet(0xe0, &es, Some(180_000));
        offset = 0;
        first = true;
        while offset < pes2.len() {
            let take = (pes2.len() - offset).min(184);
            stream.extend_from_slice(&ts_packet(0x101, first, &pes2[offset..offset + take], cc));
            first = false;
            cc = (cc + 1) & 0x0f;
            offset += take;
        }
        stream
    }

    #[test]
    fn transmux_synthetic_ts_produces_init_and_media() {
        let stream = build_test_stream();
        let mut tm = TsToFmp4Transmuxer::new();
        tm.set_max_samples_per_segment(2);
        tm.push(&stream).expect("push");
        tm.finish().expect("finish");

        let mut init: Option<Vec<u8>> = None;
        let mut media_count = 0usize;
        while let Some(seg) = tm.poll() {
            if let Some(i) = seg.init_segment {
                init = Some(i);
            }
            if seg.media_segment.is_some() {
                media_count += 1;
            }
        }
        let init = init.expect("init segment");
        assert_eq!(&init[4..8], b"ftyp");
        assert!(
            media_count >= 1,
            "expected media segments, got {media_count}"
        );

        let mut demux = IsobmffDemuxer::new();
        demux.push(&init);
        let mut tm2 = TsToFmp4Transmuxer::new();
        tm2.set_max_samples_per_segment(2);
        tm2.push(&stream).unwrap();
        tm2.finish().unwrap();
        while let Some(seg) = tm2.poll() {
            if let Some(m) = seg.media_segment {
                demux.push(&m);
            }
        }
        let mut packets = 0usize;
        loop {
            match demux.next_event() {
                Ok(Some(Mp4Event::Packet(_))) => packets += 1,
                Ok(Some(_)) => continue,
                Ok(None) => break,
                Err(_) => break,
            }
        }
        assert!(
            packets >= 1,
            "expected demuxed packets from transmuxed fMP4"
        );
    }

    #[test]
    fn empty_push_is_ok() {
        let mut tm = TsToFmp4Transmuxer::new();
        tm.push(&[]).unwrap();
        tm.finish().unwrap();
        assert!(tm.poll().is_none());
    }
}
