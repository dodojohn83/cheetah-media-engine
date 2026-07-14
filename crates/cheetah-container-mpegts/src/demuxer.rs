//! Incremental MPEG-TS demuxer.

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use cheetah_media_bitstream::aac::{AdtsHeader, AudioSpecificConfig};
use cheetah_media_bitstream::h264::NalUnit as H264NalUnit;
use cheetah_media_bitstream::h265::NalUnit as H265NalUnit;
use cheetah_media_bitstream::{BitCursor, h264, h265};
use cheetah_media_types::{
    BufferRef, ChannelLayout, CodecConfig, CodecId, ColorSpace, MediaDuration, MediaPacket,
    MediaTime, PixelFormat, SampleFormat, SequenceNumber, StreamEpoch, TimeBase, TrackId,
    TrackInfo, TrackKind, VideoFormat,
};

use crate::{
    TsError,
    clock::{ClockState, PcrClock},
    packet::TsPacket,
    pes::{PesAssembler, PesOutput, media_time_from_pes},
    section::{SectionAssembler, parse_pat, parse_pmt},
};

/// Maximum number of elementary PIDs we track.
const MAX_PIDS: usize = 64;
/// Maximum bytes to scan when trying to resync.
const MAX_SYNC_SCAN: usize = 188 * 8;

/// Output event from the MPEG-TS demuxer.
#[derive(Debug, Clone, PartialEq)]
pub enum TsEvent {
    /// A media track was discovered or its configuration changed.
    Track(TrackInfo),
    /// A compressed media packet.
    Packet(MediaPacket<'static>),
    /// PCR clock state update.
    Clock(ClockState),
}

/// Per-elementary-PID state.
#[derive(Debug)]
struct TrackState {
    info: TrackInfo,
    pes: PesAssembler,
    last_continuity: Option<u8>,
    last_time: MediaTime,
    seen_pus: bool,
    h265_vps: Vec<Vec<u8>>,
    h265_sps: Vec<Vec<u8>>,
    h265_pps: Vec<Vec<u8>>,
    h264_sps_nals: Vec<Vec<u8>>,
    h264_pps_nals: Vec<Vec<u8>>,
    h264_sps: Option<h264::Sps>,
}

impl TrackState {
    fn new(info: TrackInfo) -> Self {
        Self {
            info,
            pes: PesAssembler::new(),
            last_continuity: None,
            last_time: MediaTime::new(None, None, None, TimeBase::TS_90K),
            seen_pus: false,
            h265_vps: Vec::new(),
            h265_sps: Vec::new(),
            h265_pps: Vec::new(),
            h264_sps_nals: Vec::new(),
            h264_pps_nals: Vec::new(),
            h264_sps: None,
        }
    }
}

/// Stream mapping from a PMT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PmtEntry {
    stream_type: u8,
    elementary_pid: u16,
}

/// Incremental MPEG-TS demuxer.
#[derive(Debug)]
pub struct TsDemuxer {
    buffer: Vec<u8>,
    read_pos: usize,
    pmt_pids: Vec<u16>,
    pcr_pid: Option<u16>,
    programs: BTreeMap<u16, Vec<PmtEntry>>,
    section_assemblers: BTreeMap<u16, SectionAssembler>,
    tracks: BTreeMap<u16, TrackState>,
    track_id_counter: u32,
    sequence: u64,
    stream_epoch: StreamEpoch,
    pending_events: Vec<TsEvent>,
    clock: PcrClock,
    diagnostics: TsDiagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TsDiagnostics {
    pub packets_processed: u64,
    pub sync_losses: u64,
    pub discontinuities: u64,
}

impl Default for TsDemuxer {
    fn default() -> Self {
        Self::new()
    }
}

impl TsDemuxer {
    /// Create a new demuxer.
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            read_pos: 0,
            pmt_pids: Vec::new(),
            pcr_pid: None,
            programs: BTreeMap::new(),
            section_assemblers: BTreeMap::new(),
            tracks: BTreeMap::new(),
            track_id_counter: 0,
            sequence: 0,
            stream_epoch: StreamEpoch::new(0),
            pending_events: Vec::new(),
            clock: PcrClock::new(),
            diagnostics: TsDiagnostics::default(),
        }
    }

    /// Latest diagnostics counters.
    pub fn diagnostics(&self) -> TsDiagnostics {
        self.diagnostics
    }

    /// Push more TS bytes.
    pub fn push(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Return the next parsed event, or `None` if more data is needed.
    pub fn next_event(&mut self) -> Result<Option<TsEvent>, TsError> {
        if let Some(event) = self.pending_events.pop() {
            return Ok(Some(event));
        }

        loop {
            let packet_start = self.read_pos;
            let packet = match self.parse_packet()? {
                Some(p) => p,
                None => return Ok(None),
            };
            let packet_end = self.read_pos;
            let payload = packet
                .payload(&self.buffer[packet_start..packet_end])
                .to_vec();
            self.process_packet(packet, &payload)?;

            if let Some(event) = self.pending_events.pop() {
                return Ok(Some(event));
            }
        }
    }

    fn parse_packet(&mut self) -> Result<Option<TsPacket>, TsError> {
        loop {
            if self.read_pos + 188 > self.buffer.len() {
                return Ok(None);
            }

            match TsPacket::parse(&self.buffer[self.read_pos..]) {
                Ok(pkt) => {
                    self.read_pos += 188;
                    self.diagnostics.packets_processed += 1;
                    self.shrink();
                    return Ok(Some(pkt));
                }
                Err(TsError::LostSync) | Err(TsError::PacketTooShort) => {
                    if !self.resync()? {
                        return Ok(None);
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn resync(&mut self) -> Result<bool, TsError> {
        let scan_end = (self.read_pos + MAX_SYNC_SCAN).min(self.buffer.len());
        if scan_end < self.read_pos + 188 {
            return Ok(false);
        }

        for i in (self.read_pos + 1)..=(scan_end - 188) {
            if self.buffer[i] == 0x47
                && (i + 188 >= self.buffer.len() || self.buffer[i + 188] == 0x47)
            {
                self.diagnostics.sync_losses += 1;
                self.read_pos = i;
                return Ok(true);
            }
        }

        if self.buffer.len().saturating_sub(self.read_pos) > MAX_SYNC_SCAN {
            let drop = self.buffer.len() - MAX_SYNC_SCAN;
            self.buffer.drain(..drop);
            self.read_pos = 0;
            return Err(TsError::LostSync);
        }
        Ok(false)
    }

    fn shrink(&mut self) {
        if self.read_pos > 4096 {
            self.buffer.drain(..self.read_pos);
            self.read_pos = 0;
        }
    }

    fn process_packet(&mut self, packet: TsPacket, payload: &[u8]) -> Result<(), TsError> {
        // PCR clock update first.
        if packet.has_pcr
            && let Some(pcr_pid) = self.pcr_pid
            && packet.pid == pcr_pid
            && let Some(pcr) = packet.pcr
        {
            let state = self.clock.feed(pcr, None);
            self.pending_events.push(TsEvent::Clock(state));
        }

        if packet.pid == 0x0000 {
            return self.process_pat(payload, packet.payload_unit_start);
        }
        if self.pmt_pids.contains(&packet.pid) {
            return self.process_pmt(packet.pid, payload, packet.payload_unit_start);
        }
        if self.tracks.contains_key(&packet.pid) {
            return self.process_elementary(packet, payload);
        }
        Ok(())
    }

    fn process_pat(&mut self, payload: &[u8], pus: bool) -> Result<(), TsError> {
        let asm = self.section_assemblers.entry(0x0000).or_default();
        if let Some(section) = asm.feed(payload, pus)? {
            let entries = parse_pat(&section)?;
            self.pmt_pids = entries.iter().map(|e| e.pmt_pid).collect();
            for entry in entries {
                self.section_assemblers.entry(entry.pmt_pid).or_default();
            }
        }
        Ok(())
    }

    fn process_pmt(
        &mut self,
        pid: u16,
        payload: &[u8],
        payload_unit_start: bool,
    ) -> Result<(), TsError> {
        let asm = self.section_assemblers.entry(pid).or_default();
        if let Some(section) = asm.feed(payload, payload_unit_start)? {
            let (new_pcr_pid, streams) = parse_pmt(&section)?;
            self.pcr_pid = Some(new_pcr_pid);
            let entries: Vec<PmtEntry> = streams
                .iter()
                .map(|s| PmtEntry {
                    stream_type: s.stream_type,
                    elementary_pid: s.elementary_pid,
                })
                .collect();
            self.programs.insert(pid, entries);
            for stream in streams {
                self.register_stream(stream.stream_type, stream.elementary_pid)?;
            }
        }
        Ok(())
    }

    fn register_stream(&mut self, stream_type: u8, pid: u16) -> Result<(), TsError> {
        if self.tracks.contains_key(&pid) {
            return Ok(());
        }
        if self.tracks.len() >= MAX_PIDS {
            return Err(TsError::LimitExceeded { limit: "max pids" });
        }

        let (kind, codec) = stream_type_to_codec(stream_type)?;
        self.track_id_counter += 1;
        let track_id = TrackId::new(self.track_id_counter)
            .ok_or_else(|| TsError::invalid_input(2201, Some("track id overflow")))?;
        let mut info = TrackInfo::new(track_id, kind, codec, TimeBase::TS_90K);

        if kind == TrackKind::Audio
            && let Some(fmt) = default_audio_format(codec)
        {
            info.set_audio_format(fmt).ok();
        }

        self.tracks.insert(pid, TrackState::new(info.clone()));
        self.pending_events.push(TsEvent::Track(info));
        Ok(())
    }

    fn process_elementary(&mut self, packet: TsPacket, payload: &[u8]) -> Result<(), TsError> {
        let Some(state) = self.tracks.get_mut(&packet.pid) else {
            return Ok(());
        };

        // Transport error, discontinuity indicator, or continuity loss all reset the assembler.
        if packet.transport_error || packet.discontinuity {
            self.diagnostics.discontinuities += 1;
            state.pes = PesAssembler::new();
            state.last_time = MediaTime::new(None, None, None, TimeBase::TS_90K);
            state.seen_pus = false;
            state.last_continuity = Some(packet.continuity_counter);
            return Ok(());
        }

        // Continuity counter check.
        if let Some(last) = state.last_continuity {
            if packet.continuity_counter == last {
                // Duplicate packet; ignore.
                return Ok(());
            }
            let expected = (last + 1) & 0x0F;
            if packet.continuity_counter != expected {
                self.diagnostics.discontinuities += 1;
                state.pes = PesAssembler::new();
                state.last_time = MediaTime::new(None, None, None, TimeBase::TS_90K);
                state.seen_pus = false;
            }
        }
        state.last_continuity = Some(packet.continuity_counter);

        if packet.payload_unit_start {
            state.seen_pus = true;
        } else if !state.seen_pus {
            return Ok(());
        }

        let outputs = state.pes.feed(payload, packet.payload_unit_start)?;
        for output in outputs {
            self.handle_pes_output(packet.pid, output)?;
        }
        Ok(())
    }

    fn handle_pes_output(&mut self, pid: u16, output: PesOutput) -> Result<(), TsError> {
        let mut time = media_time_from_pes(output.header.pts, output.header.dts);
        {
            let state = self
                .tracks
                .get(&pid)
                .ok_or_else(|| TsError::invalid_input(2203, Some("missing track state")))?;
            time = time.unwrapped_33bit(&state.last_time);
        }

        let (codec, kind) = {
            let state = self
                .tracks
                .get_mut(&pid)
                .ok_or_else(|| TsError::invalid_input(2203, Some("missing track state")))?;
            state.last_time = time;
            (state.info.codec, state.info.kind)
        };

        match kind {
            TrackKind::Video => match codec {
                CodecId::H264 => self.emit_h264(pid, &output.payload, time),
                CodecId::H265 => self.emit_h265(pid, &output.payload, time),
                _ => {
                    let track_id = self.track_id_for(pid)?;
                    self.emit_packet(track_id, &output.payload, time, false);
                    Ok(())
                }
            },
            TrackKind::Audio => match codec {
                CodecId::Aac => self.emit_aac(pid, &output.payload, time),
                CodecId::Mp3 => self.emit_mp3(pid, &output.payload, time),
                _ => {
                    let track_id = self.track_id_for(pid)?;
                    self.emit_packet(track_id, &output.payload, time, false);
                    Ok(())
                }
            },
            TrackKind::Data => Ok(()),
        }
    }

    fn track_id_for(&self, pid: u16) -> Result<TrackId, TsError> {
        self.tracks
            .get(&pid)
            .map(|s| s.info.id)
            .ok_or_else(|| TsError::invalid_input(2203, Some("missing track state")))
    }

    fn emit_h264(&mut self, pid: u16, es: &[u8], time: MediaTime) -> Result<(), TsError> {
        let nals = h264::split_annexb(es)
            .map_err(|_| TsError::invalid_input(2202, Some("H.264 Annex-B split failed")))?;
        if nals.is_empty() {
            return Ok(());
        }

        // Accumulate parameter sets and update the codec config.
        {
            let state = self
                .tracks
                .get_mut(&pid)
                .ok_or_else(|| TsError::invalid_input(2203, Some("missing track state")))?;
            let mut parsed = None;
            for nal in &nals {
                if nal.nal_type == 7 && !nal.data.is_empty() {
                    state.h264_sps_nals.push(nal.data.to_vec());
                    let rbsp = h264::unescape_rbsp(nal.payload);
                    if let Ok(sps) = h264::Sps::parse(&rbsp) {
                        parsed = Some(sps);
                    }
                } else if nal.nal_type == 8 && !nal.data.is_empty() {
                    state.h264_pps_nals.push(nal.data.to_vec());
                }
            }
            if let Some(sps) = parsed {
                state.h264_sps = Some(sps);
            }
        }
        self.update_h264_config(pid)?;

        let track_id = self.track_id_for(pid)?;
        let groups = group_h264_nals(&nals)?;
        for group in groups {
            let is_key = group.iter().any(|n| (**n).is_idr());
            let mut au = Vec::new();
            for nal in group {
                au.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
                au.extend_from_slice(nal.data);
            }
            self.emit_packet(track_id, &au, time, is_key);
        }
        Ok(())
    }

    fn update_h264_config(&mut self, pid: u16) -> Result<(), TsError> {
        let state = self
            .tracks
            .get_mut(&pid)
            .ok_or_else(|| TsError::invalid_input(2203, Some("missing track state")))?;
        let Some(sps) = state.h264_sps else {
            return Ok(());
        };
        if state.h264_sps_nals.is_empty() {
            return Ok(());
        }

        let cfg = cheetah_media_bitstream::H264CodecConfig {
            configuration_version: 1,
            avc_profile_indication: sps.profile_idc,
            profile_compatibility: sps.constraint_set_flags,
            avc_level_indication: sps.level_idc,
            length_size_minus_one: 3,
            sps_list: state.h264_sps_nals.clone(),
            pps_list: state.h264_pps_nals.clone(),
            width: sps.width,
            height: sps.height,
            codec_string: sps.codec_string(),
        };
        if cfg.pps_list.is_empty() {
            // Need at least one PPS for a valid decoder config.
            return Ok(());
        }

        let old = state.info.codec_config.clone();
        let new_config = CodecConfig::AvcC(cfg.build());
        if old != new_config {
            state.info.set_codec_config(new_config);
            let format = VideoFormat {
                pixel_format: PixelFormat::Yuv420P,
                coded_width: sps.width,
                coded_height: sps.height,
                visible_width: sps.width,
                visible_height: sps.height,
                stride: sps.width,
                color_space: ColorSpace::Unspecified,
            };
            state.info.set_video_format(format).ok();
            self.pending_events.push(TsEvent::Track(state.info.clone()));
        }
        Ok(())
    }

    fn emit_h265(&mut self, pid: u16, es: &[u8], time: MediaTime) -> Result<(), TsError> {
        let nals = h265::split_annexb(es)
            .map_err(|_| TsError::invalid_input(2204, Some("H.265 Annex-B split failed")))?;
        if nals.is_empty() {
            return Ok(());
        }

        {
            let state = self
                .tracks
                .get_mut(&pid)
                .ok_or_else(|| TsError::invalid_input(2203, Some("missing track state")))?;
            for nal in &nals {
                let t = nal.nal_unit_type;
                if t == 32 && nal.data.len() > 2 {
                    state.h265_vps.push(nal.data.to_vec());
                } else if t == 33 && nal.data.len() > 2 {
                    state.h265_sps.push(nal.data.to_vec());
                } else if t == 34 && nal.data.len() > 2 {
                    state.h265_pps.push(nal.data.to_vec());
                }
            }
        }
        self.update_h265_config(pid)?;

        let track_id = self.track_id_for(pid)?;
        let groups = group_h265_nals(&nals)?;
        for group in groups {
            let is_key = group.iter().any(|n| (**n).is_irap());
            let mut au = Vec::new();
            for nal in group {
                au.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
                au.extend_from_slice(nal.data);
            }
            self.emit_packet(track_id, &au, time, is_key);
        }
        Ok(())
    }

    fn update_h265_config(&mut self, pid: u16) -> Result<(), TsError> {
        let state = self
            .tracks
            .get_mut(&pid)
            .ok_or_else(|| TsError::invalid_input(2203, Some("missing track state")))?;
        if state.h265_sps.is_empty() {
            return Ok(());
        }
        let mut cfg = cheetah_media_bitstream::H265CodecConfig {
            configuration_version: 1,
            length_size_minus_one: 3,
            ..Default::default()
        };
        cfg.vps_list = state.h265_vps.clone();
        cfg.sps_list = state.h265_sps.clone();
        cfg.pps_list = state.h265_pps.clone();

        let new_config = CodecConfig::HevcC(cfg.build());
        if state.info.codec_config != new_config {
            state.info.set_codec_config(new_config);
            self.pending_events.push(TsEvent::Track(state.info.clone()));
        }
        Ok(())
    }

    fn emit_aac(&mut self, pid: u16, es: &[u8], mut time: MediaTime) -> Result<(), TsError> {
        let frames = cheetah_media_bitstream::aac::split_adts(es)
            .map_err(|_| TsError::invalid_input(2206, Some("AAC ADTS split failed")))?;
        if frames.is_empty() {
            return Ok(());
        }

        {
            let state = self
                .tracks
                .get_mut(&pid)
                .ok_or_else(|| TsError::invalid_input(2203, Some("missing track state")))?;
            if let Ok(header) = AdtsHeader::parse(frames[0]) {
                let fmt = cheetah_media_types::AudioFormat {
                    sample_format: SampleFormat::S16,
                    sample_rate: header.sampling_frequency,
                    channel_layout: if header.channel_count == 1 {
                        ChannelLayout::Mono
                    } else {
                        ChannelLayout::Stereo
                    },
                    sample_count: header.samples_per_frame as u32,
                };
                state.info.set_audio_format(fmt).ok();
                let aot = header.profile + 1;
                let asc = AudioSpecificConfig {
                    audio_object_type: aot,
                    sampling_frequency_index: header.sampling_frequency_index,
                    sampling_frequency: header.sampling_frequency,
                    channel_configuration: header.channel_configuration,
                    channel_count: header.channel_count,
                };
                state
                    .info
                    .set_codec_config(CodecConfig::AacAudioSpecificConfig(asc.build()));
                self.pending_events.push(TsEvent::Track(state.info.clone()));
            }
        }

        let track_id = self.track_id_for(pid)?;
        for frame in frames {
            let duration_ms = AdtsHeader::parse(frame)
                .map(|h| h.duration_ms as i64)
                .unwrap_or(0);
            self.emit_packet(track_id, frame, time, false);
            if duration_ms > 0 {
                time = time
                    .checked_add(MediaDuration::new(duration_ms * 90))
                    .unwrap_or(time);
            }
        }
        if let Some(state) = self.tracks.get_mut(&pid) {
            state.last_time = time;
        }
        Ok(())
    }

    fn emit_mp3(&mut self, pid: u16, es: &[u8], time: MediaTime) -> Result<(), TsError> {
        let track_id = self.track_id_for(pid)?;
        if let Some(state) = self.tracks.get_mut(&pid)
            && let Ok(header) = cheetah_media_bitstream::mp3::Mp3Header::parse(es)
        {
            let fmt = cheetah_media_types::AudioFormat {
                sample_format: SampleFormat::S16,
                sample_rate: header.sample_rate,
                channel_layout: if header.channel_count == 1 {
                    ChannelLayout::Mono
                } else {
                    ChannelLayout::Stereo
                },
                sample_count: header.samples_per_frame as u32,
            };
            state.info.set_audio_format(fmt).ok();
        }
        self.emit_packet(track_id, es, time, false);
        Ok(())
    }

    fn emit_packet(&mut self, track_id: TrackId, data: &[u8], time: MediaTime, is_key: bool) {
        let seq = SequenceNumber::new(self.sequence);
        self.sequence += 1;
        let mut packet = MediaPacket::new(
            BufferRef::from_owned(data.to_owned()),
            track_id,
            self.stream_epoch,
            seq,
            time,
        );
        packet.flags.is_keyframe = is_key;
        self.pending_events.push(TsEvent::Packet(packet));
    }
}

fn stream_type_to_codec(stream_type: u8) -> Result<(TrackKind, CodecId), TsError> {
    match stream_type {
        0x1b => Ok((TrackKind::Video, CodecId::H264)),
        0x24 => Ok((TrackKind::Video, CodecId::H265)),
        0x0f => Ok((TrackKind::Audio, CodecId::Aac)),
        0x03 | 0x04 => Ok((TrackKind::Audio, CodecId::Mp3)),
        0x90 => Ok((TrackKind::Audio, CodecId::G711A)),
        0x91 => Ok((TrackKind::Audio, CodecId::G711U)),
        _ => Err(TsError::unsupported(
            2301,
            Some("unsupported MPEG-TS stream type"),
        )),
    }
}

fn default_audio_format(codec: CodecId) -> Option<cheetah_media_types::AudioFormat> {
    Some(match codec {
        CodecId::G711A | CodecId::G711U => cheetah_media_types::AudioFormat {
            sample_format: SampleFormat::U8,
            sample_rate: 8000,
            channel_layout: ChannelLayout::Mono,
            sample_count: 160,
        },
        _ => return None,
    })
}

#[allow(unused_assignments)]
fn group_h264_nals<'a>(
    nals: &'a [H264NalUnit<'a>],
) -> Result<Vec<Vec<&'a H264NalUnit<'a>>>, TsError> {
    let mut groups: Vec<Vec<&H264NalUnit<'_>>> = Vec::new();
    let mut current: Vec<&H264NalUnit<'_>> = Vec::new();
    let mut has_vcl = false;

    for nal in nals {
        if nal.nal_type == 9 {
            if !current.is_empty() {
                groups.push(current);
            }
            current = Vec::new();
            has_vcl = false;
            current.push(nal);
            continue;
        }

        if nal.is_slice() {
            let first_mb = parse_h264_first_mb_in_slice(nal).unwrap_or(0);
            if has_vcl && first_mb == 0 && !current.is_empty() {
                groups.push(current);
                current = Vec::new();
                has_vcl = false;
            }
            has_vcl = true;
        }
        current.push(nal);
    }

    if !current.is_empty() {
        groups.push(current);
    }
    Ok(groups)
}

fn parse_h264_first_mb_in_slice(nal: &H264NalUnit<'_>) -> Option<u64> {
    let rbsp = h264::unescape_rbsp(nal.payload);
    let mut cursor = BitCursor::new(&rbsp);
    cursor.read_ue().ok()
}

#[allow(unused_assignments)]
fn group_h265_nals<'a>(
    nals: &'a [H265NalUnit<'a>],
) -> Result<Vec<Vec<&'a H265NalUnit<'a>>>, TsError> {
    let mut groups: Vec<Vec<&H265NalUnit<'_>>> = Vec::new();
    let mut current: Vec<&H265NalUnit<'_>> = Vec::new();
    let mut has_vcl = false;

    for nal in nals {
        if nal.nal_unit_type == 35 {
            if !current.is_empty() {
                groups.push(current);
            }
            current = Vec::new();
            has_vcl = false;
            current.push(nal);
            continue;
        }

        if is_h265_vcl(nal.nal_unit_type) {
            let first = parse_h265_first_slice_flag(nal).unwrap_or(false);
            if has_vcl && first && !current.is_empty() {
                groups.push(current);
                current = Vec::new();
                has_vcl = false;
            }
            has_vcl = true;
        }
        current.push(nal);
    }

    if !current.is_empty() {
        groups.push(current);
    }
    Ok(groups)
}

fn parse_h265_first_slice_flag(nal: &H265NalUnit<'_>) -> Option<bool> {
    if nal.payload.is_empty() {
        return None;
    }
    let mut cursor = BitCursor::new(nal.payload);
    cursor.read_bool().ok()
}

fn is_h265_vcl(nal_type: u8) -> bool {
    (nal_type <= 9) || (16..=23).contains(&nal_type)
}
