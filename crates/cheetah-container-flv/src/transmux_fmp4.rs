//! FLV → fragmented MP4 transmuxer for MSE playback.
//!
//! Incremental: feed FLV bytes via [`FlvToFmp4Transmuxer::push`], then drain
//! [`SegmentOutput`] values with [`FlvToFmp4Transmuxer::poll`]. On end-of-stream
//! call [`FlvToFmp4Transmuxer::finish`] to flush trailing samples.

use alloc::collections::{BTreeMap, VecDeque};

use cheetah_container_isobmff::{FragmentedMp4Muxer, SegmentOutput, TrackConfig, boxes::types};
use cheetah_media_types::{CodecConfig, CodecId, MediaPacket, TrackInfo, TrackKind};

use crate::{FlvDemuxer, FlvError, FlvEvent, FlvMode};

/// Errors produced while transmuxing FLV into fMP4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransmuxError {
    /// Underlying FLV demux failed.
    Flv(FlvError),
    /// Muxer rejected a packet or track config.
    Mux(u32),
    /// Unsupported codec for MSE fMP4 (only H.264/H.265 + AAC/MP3/G.711).
    UnsupportedCodec,
}

impl core::fmt::Display for TransmuxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Flv(e) => write!(f, "flv demux error: {e}"),
            Self::Mux(code) => write!(f, "fmp4 mux error code={code}"),
            Self::UnsupportedCodec => write!(f, "unsupported codec for flv→fmp4 transmux"),
        }
    }
}

/// Incremental FLV → fMP4 transmuxer.
#[derive(Debug)]
pub struct FlvToFmp4Transmuxer {
    demuxer: FlvDemuxer,
    muxer: FragmentedMp4Muxer,
    configured: BTreeMap<u32, TrackConfig>,
    pending: VecDeque<SegmentOutput>,
    /// Samples since last successful keyframe-aligned flush.
    samples_since_flush: usize,
    /// Force a media flush after this many samples even without a new keyframe.
    max_samples_per_segment: usize,
    finished: bool,
    error: Option<TransmuxError>,
}

impl Default for FlvToFmp4Transmuxer {
    fn default() -> Self {
        Self::new()
    }
}

impl FlvToFmp4Transmuxer {
    /// Create a transmuxer in FLV auto mode (file or stream PreviousTagSize).
    pub fn new() -> Self {
        Self::with_mode(FlvMode::Auto)
    }

    /// Create a transmuxer with an explicit FLV parse mode.
    pub fn with_mode(mode: FlvMode) -> Self {
        Self {
            demuxer: FlvDemuxer::new(mode),
            muxer: FragmentedMp4Muxer::new(),
            configured: BTreeMap::new(),
            pending: VecDeque::new(),
            samples_since_flush: 0,
            max_samples_per_segment: 60,
            finished: false,
            error: None,
        }
    }

    /// Override the soft sample budget used to flush media segments.
    pub fn set_max_samples_per_segment(&mut self, n: usize) {
        self.max_samples_per_segment = n.max(1);
    }

    /// Push additional FLV bytes.
    pub fn push(&mut self, data: &[u8]) -> Result<(), TransmuxError> {
        if let Some(err) = self.error {
            return Err(err);
        }
        if self.finished {
            return Ok(());
        }
        self.demuxer.push(data);
        self.drain_demux()?;
        Ok(())
    }

    /// Signal end of FLV input and flush remaining samples into segments.
    pub fn finish(&mut self) -> Result<(), TransmuxError> {
        if let Some(err) = self.error {
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

    /// Pop the next ready fMP4 segment (init and/or media).
    pub fn poll(&mut self) -> Option<SegmentOutput> {
        self.pending.pop_front()
    }

    /// True when a fatal error has been recorded.
    pub fn failed(&self) -> bool {
        self.error.is_some()
    }

    fn drain_demux(&mut self) -> Result<(), TransmuxError> {
        loop {
            match self.demuxer.next_event() {
                Ok(Some(FlvEvent::Header(_))) => continue,
                Ok(Some(FlvEvent::Script(_))) => continue,
                Ok(Some(FlvEvent::Track(info))) => {
                    self.configure_track(&info)?;
                }
                Ok(Some(FlvEvent::Packet(packet))) => {
                    self.push_packet(packet)?;
                }
                Ok(None) => return Ok(()),
                Err(FlvError::NeedMoreData) => return Ok(()),
                Err(e) => {
                    let err = TransmuxError::Flv(e);
                    self.error = Some(err);
                    return Err(err);
                }
            }
        }
    }

    fn configure_track(&mut self, info: &TrackInfo) -> Result<(), TransmuxError> {
        let cfg = track_info_to_config(info)?;
        let id = cfg.track_id;
        let changed = self
            .configured
            .get(&id)
            .map(|prev| prev.codec_config != cfg.codec_config || prev.width != cfg.width || prev.height != cfg.height)
            .unwrap_or(true);
        if changed {
            self.muxer.configure(cfg.clone());
            self.configured.insert(id, cfg);
        }
        Ok(())
    }

    fn push_packet(&mut self, packet: MediaPacket<'static>) -> Result<(), TransmuxError> {
        let track_id = packet.track_id.get();
        if !self.configured.contains_key(&track_id) {
            // Packet before track config — skip rather than fail (common for
            // incomplete live start).
            return Ok(());
        }
        let is_key = packet.flags.is_keyframe;
        self.muxer
            .push_packet(packet)
            .map_err(|e| {
                let err = TransmuxError::Mux(mp4_error_code(&e));
                self.error = Some(err);
                err
            })?;
        self.samples_since_flush = self.samples_since_flush.saturating_add(1);

        // Flush on keyframe boundaries once we have a full GOP, or when the
        // soft sample budget is hit (audio-only / no keyframes).
        if (is_key && self.samples_since_flush > 1)
            || self.samples_since_flush >= self.max_samples_per_segment
        {
            self.flush_mux(false)?;
        }
        Ok(())
    }

    fn flush_mux(&mut self, force: bool) -> Result<(), TransmuxError> {
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
                let err = TransmuxError::Mux(mp4_error_code(&e));
                self.error = Some(err);
                Err(err)
            }
        }
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

fn track_info_to_config(info: &TrackInfo) -> Result<TrackConfig, TransmuxError> {
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
                _ => return Err(TransmuxError::UnsupportedCodec),
            };
            let codec_config = match &info.codec_config {
                CodecConfig::AvcC(_) | CodecConfig::HevcC(_) => info.codec_config.clone(),
                _ => return Err(TransmuxError::UnsupportedCodec),
            };
            // FLV timestamps are milliseconds; use 1000 Hz timescale.
            Ok(TrackConfig {
                track_id,
                kind: TrackKind::Video,
                codec: info.codec,
                codec_config,
                timescale: 1000,
                sample_entry_type,
                width: if width == 0 { 640 } else { width },
                height: if height == 0 { 360 } else { height },
                sample_rate: 0,
                channel_count: 0,
                default_sample_duration: 33,
            })
        }
        TrackKind::Audio => {
            let (sample_rate, channels) = info
                .audio_format
                .map(|af| (af.sample_rate, af.channel_layout.channels() as u16))
                .unwrap_or((44_100, 2));
            let (codec, codec_config, sample_entry_type) = match info.codec {
                CodecId::Aac => match &info.codec_config {
                    CodecConfig::AacAudioSpecificConfig(bytes) => (
                        CodecId::Aac,
                        CodecConfig::AacAudioSpecificConfig(bytes.clone()),
                        types::MP4A,
                    ),
                    _ => return Err(TransmuxError::UnsupportedCodec),
                },
                // G.711 / MP3 in FLV may lack a full sample entry path in the
                // current muxer; reject until supported.
                _ => return Err(TransmuxError::UnsupportedCodec),
            };
            Ok(TrackConfig {
                track_id,
                kind: TrackKind::Audio,
                codec,
                codec_config,
                timescale: sample_rate.max(1),
                sample_entry_type,
                width: 0,
                height: 0,
                sample_rate: sample_rate.max(1),
                channel_count: channels.max(1),
                default_sample_duration: 1024,
            })
        }
        TrackKind::Data => Err(TransmuxError::UnsupportedCodec),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_container_isobmff::{IsobmffDemuxer, Mp4Event};

    #[test]
    fn transmux_fixture_flv_produces_init_and_media() {
        let bytes = include_bytes!("../../../testing/fixtures/media/h264-flv/clip.flv");
        let mut tm = FlvToFmp4Transmuxer::new();
        tm.set_max_samples_per_segment(8);
        tm.push(bytes).expect("push flv");
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
        assert!(init.len() > 32, "init too small");
        // ftyp
        assert_eq!(&init[4..8], b"ftyp");
        assert!(media_count >= 1, "expected at least one media segment");

        // Demux first media segment after init to ensure structure is valid.
        let mut demux = IsobmffDemuxer::new();
        demux.push(&init);
        // Collect media and push
        let mut tm2 = FlvToFmp4Transmuxer::new();
        tm2.set_max_samples_per_segment(8);
        tm2.push(bytes).unwrap();
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
        assert!(packets >= 1, "expected demuxed packets from transmuxed fMP4");
    }

    #[test]
    fn empty_push_is_ok() {
        let mut tm = FlvToFmp4Transmuxer::new();
        tm.push(&[]).unwrap();
        tm.finish().unwrap();
        assert!(tm.poll().is_none());
    }
}
