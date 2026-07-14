//! fMP4 / MSE segmenter and muxer.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use cheetah_media_types::{CodecConfig, CodecId, MediaPacket, TrackKind};

use crate::Mp4Error;
use crate::boxes::{
    types, write_box, write_fullbox, write_fullbox_box, write_u16, write_u24, write_u32, write_u64,
};

/// Track configuration used by the muxer.
#[derive(Debug, Clone)]
pub struct TrackConfig {
    pub track_id: u32,
    pub kind: TrackKind,
    pub codec: CodecId,
    pub codec_config: CodecConfig,
    pub timescale: u32,
    pub sample_entry_type: u32,
    pub width: u16,
    pub height: u16,
    pub sample_rate: u32,
    pub channel_count: u16,
    pub default_sample_duration: u32,
}

/// Output from a segment flush.
#[derive(Debug, Clone)]
pub struct SegmentOutput {
    /// The init segment (`ftyp` + `moov`) if it was generated/updated.
    pub init_segment: Option<Vec<u8>>,
    /// The media segment (`moof` + `mdat`).
    pub media_segment: Option<Vec<u8>>,
    /// Segment sequence number.
    pub sequence: u32,
}

/// Fragmented MP4 muxer that builds MSE-compatible segments.
#[derive(Debug)]
pub struct FragmentedMp4Muxer {
    configs: BTreeMap<u32, TrackConfig>,
    buffers: BTreeMap<u32, Vec<MediaPacket<'static>>>,
    sequence: u32,
    init_segment: Option<Vec<u8>>,
    last_init_hash: u64,
}

impl FragmentedMp4Muxer {
    pub fn new() -> Self {
        Self {
            configs: BTreeMap::new(),
            buffers: BTreeMap::new(),
            sequence: 1,
            init_segment: None,
            last_init_hash: 0,
        }
    }

    /// Add or update a track configuration.
    pub fn configure(&mut self, config: TrackConfig) {
        self.configs.insert(config.track_id, config);
    }

    /// Push a packet into the muxer.
    pub fn push_packet(&mut self, packet: MediaPacket<'static>) {
        self.buffers
            .entry(packet.track_id.get())
            .or_default()
            .push(packet);
    }

    /// Flush a media segment. Returns `None` if there is nothing to flush.
    ///
    /// Each track is cut at its last keyframe if one is present, retaining the
    /// trailing samples for the next segment.
    pub fn flush_segment(&mut self) -> Result<Option<SegmentOutput>, Mp4Error> {
        // First determine the cut point for each track.
        let mut cut_indices: Vec<(u32, usize)> = Vec::new();
        for (track_id, buf) in &self.buffers {
            if buf.is_empty() {
                continue;
            }
            let mut last_key = None;
            for (i, pkt) in buf.iter().enumerate() {
                if pkt.flags.is_keyframe {
                    last_key = Some(i);
                }
            }
            // If no keyframe, flush all (common for audio or test data).
            let cut = last_key.map(|i| i + 1).unwrap_or(buf.len());
            cut_indices.push((*track_id, cut.min(buf.len())));
        }
        if cut_indices.is_empty() {
            return Ok(None);
        }

        // Extract the samples for this segment.
        let mut track_packets: BTreeMap<u32, Vec<MediaPacket<'static>>> = BTreeMap::new();
        for (track_id, cut) in cut_indices {
            if let Some(buf) = self.buffers.get_mut(&track_id) {
                let tail = buf.split_off(cut);
                let taken = core::mem::replace(buf, tail);
                track_packets.insert(track_id, taken);
            }
        }

        // Build or rebuild init segment when configurations changed.
        let init_hash = self
            .configs
            .iter()
            .fold(0u64, |acc, (k, c)| hash_config(acc, *k, c));
        let init_segment = if self.init_segment.is_none() || init_hash != self.last_init_hash {
            let init = self.write_init_segment()?;
            self.last_init_hash = init_hash;
            self.init_segment = Some(init.clone());
            Some(init)
        } else {
            None
        };

        let media_segment = self.write_media_segment(&track_packets)?;
        let sequence = self.sequence;
        self.sequence += 1;

        Ok(Some(SegmentOutput {
            init_segment,
            media_segment: Some(media_segment),
            sequence,
        }))
    }

    fn write_init_segment(&self) -> Result<Vec<u8>, Mp4Error> {
        if self.configs.is_empty() {
            return Err(Mp4Error::invalid_input(3501, Some("no tracks configured")));
        }
        let mut out = write_ftyp();
        out.extend(write_moov(&self.configs)?);
        Ok(out)
    }

    fn write_media_segment(
        &self,
        track_packets: &BTreeMap<u32, Vec<MediaPacket<'static>>>,
    ) -> Result<Vec<u8>, Mp4Error> {
        let mut out = Vec::new();
        out.extend(write_moof(track_packets, self.sequence)?);
        out.extend(write_mdat(track_packets));
        Ok(out)
    }
}

impl Default for FragmentedMp4Muxer {
    fn default() -> Self {
        Self::new()
    }
}

fn hash_config(acc: u64, track_id: u32, c: &TrackConfig) -> u64 {
    let mut h = acc;
    h = h.wrapping_mul(31).wrapping_add(u64::from(track_id));

    let codec_val: u64 = match c.codec {
        CodecId::H264 => 1,
        CodecId::H265 => 2,
        CodecId::Aac => 3,
        CodecId::G711A => 4,
        CodecId::G711U => 5,
        CodecId::Mp3 => 6,
        CodecId::Opus => 7,
        CodecId::PcmU8 => 8,
        CodecId::PcmS16 => 9,
        CodecId::Unknown(v) => 10 + u64::from(v),
    };
    h = h.wrapping_mul(31).wrapping_add(codec_val);

    if let Some(bytes) = c.codec_config.bytes() {
        for b in bytes {
            h = h.wrapping_mul(31).wrapping_add(u64::from(*b));
        }
    }
    h = h.wrapping_mul(31).wrapping_add(c.timescale as u64);
    h = h.wrapping_mul(31).wrapping_add(c.width as u64);
    h = h.wrapping_mul(31).wrapping_add(c.height as u64);
    h = h.wrapping_mul(31).wrapping_add(c.sample_rate as u64);
    h = h.wrapping_mul(31).wrapping_add(c.channel_count as u64);
    h
}

fn write_ftyp() -> Vec<u8> {
    let mut body = Vec::with_capacity(16);
    body.extend_from_slice(b"isom");
    write_u32(&mut body, 0x200);
    body.extend_from_slice(b"isom");
    body.extend_from_slice(b"mp41");
    write_box(types::FTYP, &body)
}

fn write_moov(configs: &BTreeMap<u32, TrackConfig>) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::new();
    body.extend(write_mvhd(configs));
    for cfg in configs.values() {
        body.extend(write_trak(cfg)?);
    }
    body.extend(write_mvex(configs));
    Ok(write_box(types::MOOV, &body))
}

fn write_mvhd(configs: &BTreeMap<u32, TrackConfig>) -> Vec<u8> {
    let mut body = Vec::with_capacity(100);
    body.extend_from_slice(&write_fullbox(0, 0)); // version/flags
    write_u32(&mut body, 0); // creation_time
    write_u32(&mut body, 0); // modification_time
    let timescale = configs.values().next().map(|c| c.timescale).unwrap_or(1000);
    write_u32(&mut body, timescale);
    write_u32(&mut body, 0); // duration
    write_u32(&mut body, 0x00010000); // rate
    write_u16(&mut body, 0x0100); // volume
    body.extend_from_slice(&[0u8; 10]); // reserved
    body.extend_from_slice(&[0u8; 36]); // matrix
    body.extend_from_slice(&[0u8; 24]); // pre_defined
    let next_track_id = configs.keys().next_back().copied().unwrap_or(0) + 1;
    write_u32(&mut body, next_track_id);
    write_box(types::MVHD, &body)
}

fn write_trak(cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::new();
    body.extend(write_tkhd(cfg));
    body.extend(write_mdia(cfg)?);
    Ok(write_box(types::TRAK, &body))
}

fn write_tkhd(cfg: &TrackConfig) -> Vec<u8> {
    let mut body = Vec::with_capacity(84);
    body.extend_from_slice(&write_fullbox(0, 0x0000_0003)); // enabled + in_movie
    write_u32(&mut body, 0); // creation_time
    write_u32(&mut body, 0); // modification_time
    write_u32(&mut body, cfg.track_id);
    write_u32(&mut body, 0); // reserved
    write_u32(&mut body, 0); // duration
    body.extend_from_slice(&[0u8; 8]); // reserved
    write_u16(&mut body, 0); // layer
    write_u16(&mut body, 0); // alternate_group
    write_u16(&mut body, 0); // volume
    write_u16(&mut body, 0); // reserved
    body.extend_from_slice(&[0u8; 36]); // matrix
    let (w, h) = if cfg.kind == TrackKind::Video {
        (u32::from(cfg.width) << 16, u32::from(cfg.height) << 16)
    } else {
        (0u32, 0u32)
    };
    write_u32(&mut body, w);
    write_u32(&mut body, h);
    write_box(types::TKHD, &body)
}

fn write_mdia(cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::new();
    body.extend(write_mdhd(cfg));
    body.extend(write_hdlr(cfg));
    body.extend(write_minf(cfg)?);
    Ok(write_box(types::MDIA, &body))
}

fn write_mdhd(cfg: &TrackConfig) -> Vec<u8> {
    let mut body = Vec::with_capacity(24);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, 0); // creation_time
    write_u32(&mut body, 0); // modification_time
    write_u32(&mut body, cfg.timescale);
    write_u32(&mut body, 0); // duration
    write_u16(&mut body, 0x55c4); // language (und)
    write_u16(&mut body, 0); // pre_defined
    write_box(types::MDHD, &body)
}

fn write_hdlr(cfg: &TrackConfig) -> Vec<u8> {
    let mut body = Vec::with_capacity(33);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, 0); // pre_defined
    let handler: &[u8; 4] = if cfg.kind == TrackKind::Video {
        b"vide"
    } else {
        b"soun"
    };
    body.extend_from_slice(handler);
    body.extend_from_slice(&[0u8; 12]); // reserved
    if cfg.kind == TrackKind::Video {
        body.extend_from_slice(b"VideoHandler\0");
    } else {
        body.extend_from_slice(b"SoundHandler\0");
    }
    write_box(types::HDLR, &body)
}

fn write_minf(cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::new();
    if cfg.kind == TrackKind::Video {
        body.extend(write_vmhd());
    } else {
        body.extend(write_smhd());
    }
    body.extend(write_dinf());
    body.extend(write_stbl(cfg)?);
    Ok(write_box(types::MINF, &body))
}

fn write_vmhd() -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&write_fullbox(0, 0x0000_0001));
    write_u16(&mut body, 0); // graphicsmode
    body.extend_from_slice(&[0u8; 6]); // opcolor
    write_box(types::VMHD, &body)
}

fn write_smhd() -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u16(&mut body, 0); // balance
    write_u16(&mut body, 0); // reserved
    write_box(types::SMHD, &body)
}

fn write_dinf() -> Vec<u8> {
    let mut dref_body = Vec::new();
    dref_body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut dref_body, 1); // entry_count
    let url_body = write_fullbox(0, 0x0000_0001); // self-contained
    dref_body.extend(write_box(types::URL, &url_body));
    write_box(types::DREF, &dref_body)
}

fn write_stbl(cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::new();
    body.extend(write_stsd(cfg)?);
    body.extend(write_stts());
    body.extend(write_stsc());
    body.extend(write_stsz());
    body.extend(write_stco());
    Ok(write_box(types::STBL, &body))
}

fn write_stsd(cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::new();
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, 1); // entry_count
    body.extend(write_sample_entry(cfg)?);
    Ok(write_box(types::STSD, &body))
}

fn write_sample_entry(cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
    match cfg.kind {
        TrackKind::Video => write_visual_sample_entry(cfg),
        TrackKind::Audio => write_audio_sample_entry(cfg),
        TrackKind::Data => Err(Mp4Error::unsupported(
            3502,
            Some("data tracks not supported"),
        )),
    }
}

fn write_visual_sample_entry(cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::with_capacity(78);
    body.extend_from_slice(&[0u8; 6]); // reserved
    write_u16(&mut body, 1); // data_reference_index
    write_u16(&mut body, 0); // pre_defined
    write_u16(&mut body, 0); // reserved
    body.extend_from_slice(&[0u8; 12]); // pre_defined[3]
    write_u16(&mut body, cfg.width);
    write_u16(&mut body, cfg.height);
    write_u32(&mut body, 0x00480000); // horizresolution
    write_u32(&mut body, 0x00480000); // vertresolution
    write_u32(&mut body, 0); // reserved
    write_u16(&mut body, 1); // frame_count
    body.extend_from_slice(&[0u8; 32]); // compressorname
    write_u16(&mut body, 0x0018); // depth
    write_u16(&mut body, 0xffff); // pre_defined

    body.extend(codec_config_box(cfg)?);

    let box_type = if cfg.codec == CodecId::H265 {
        types::HVC1
    } else {
        types::AVC1
    };
    Ok(write_box(box_type, &body))
}

fn write_audio_sample_entry(cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::with_capacity(28);
    body.extend_from_slice(&[0u8; 6]); // reserved
    write_u16(&mut body, 1); // data_reference_index
    body.extend_from_slice(&[0u8; 8]); // reserved
    write_u16(&mut body, cfg.channel_count);
    write_u16(&mut body, 16); // samplesize
    write_u16(&mut body, 0); // pre_defined
    write_u16(&mut body, 0); // reserved
    write_u32(&mut body, cfg.sample_rate << 16);

    body.extend(write_esds(cfg)?);
    Ok(write_box(types::MP4A, &body))
}

fn codec_config_box(cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
    match &cfg.codec_config {
        CodecConfig::AvcC(bytes) => Ok(write_box(types::AVCC, bytes)),
        CodecConfig::HevcC(bytes) => Ok(write_box(types::HVCC, bytes)),
        _ => Err(Mp4Error::unsupported(
            3503,
            Some("codec config not supported for muxing"),
        )),
    }
}

fn write_esds(cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
    let asc = match &cfg.codec_config {
        CodecConfig::AacAudioSpecificConfig(bytes) => bytes.as_slice(),
        _ => {
            return Err(Mp4Error::unsupported(
                3504,
                Some("esds requires AAC AudioSpecificConfig"),
            ));
        }
    };

    // DecoderSpecificInfo (tag 0x05)
    let dsi = write_descriptor(0x05, asc);

    // DecoderConfigDescriptor (tag 0x04)
    let mut dcd_body = Vec::with_capacity(13 + dsi.len());
    dcd_body.push(0x40); // objectTypeIndication (MPEG-4 Audio)
    dcd_body.push((0x05 << 2) | 0x01); // streamType audio (5), upstream 0, reserved 1
    write_u24(&mut dcd_body, 0); // bufferSizeDB
    write_u32(&mut dcd_body, 0); // maxBitrate
    write_u32(&mut dcd_body, 0); // avgBitrate
    dcd_body.extend(dsi);
    let dcd = write_descriptor(0x04, &dcd_body);

    // SLConfigDescriptor (tag 0x06)
    let slc = write_descriptor(0x06, &[0x02]);

    // ESDescriptor (tag 0x03)
    let mut es_body = Vec::with_capacity(3 + dcd.len() + slc.len());
    write_u16(&mut es_body, cfg.track_id as u16); // ES_ID
    es_body.push(0x00); // flags
    es_body.extend(dcd);
    es_body.extend(slc);
    let esd = write_descriptor(0x03, &es_body);

    Ok(write_fullbox_box(types::ESDS, 0, 0, &esd))
}

fn write_descriptor(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + body.len());
    out.push(tag);
    let mut len = body.len();
    let mut bytes = Vec::new();
    loop {
        bytes.push((len & 0x7f) as u8);
        len >>= 7;
        if len == 0 {
            break;
        }
    }
    bytes.reverse();
    let last = bytes.len().saturating_sub(1);
    for (i, b) in bytes.iter_mut().enumerate() {
        if i != last {
            *b |= 0x80;
        }
    }
    out.extend(bytes);
    out.extend_from_slice(body);
    out
}

fn write_stts() -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, 0); // entry_count
    write_box(types::STTS, &body)
}

fn write_stsc() -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, 0); // entry_count
    write_box(types::STSC, &body)
}

fn write_stsz() -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, 0); // sample_size
    write_u32(&mut body, 0); // sample_count
    write_box(types::STSZ, &body)
}

fn write_stco() -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, 0); // entry_count
    write_box(types::STCO, &body)
}

fn write_mvex(configs: &BTreeMap<u32, TrackConfig>) -> Vec<u8> {
    let mut body = Vec::new();
    for cfg in configs.values() {
        body.extend(write_trex(cfg));
    }
    write_box(types::MVEX, &body)
}

fn write_trex(cfg: &TrackConfig) -> Vec<u8> {
    let mut body = Vec::with_capacity(24);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, cfg.track_id);
    write_u32(&mut body, 1); // default_sample_description_index
    write_u32(&mut body, cfg.default_sample_duration);
    write_u32(&mut body, 0); // default_sample_size
    write_u32(&mut body, 0); // default_sample_flags
    write_box(types::TREX, &body)
}

fn write_moof(
    track_packets: &BTreeMap<u32, Vec<MediaPacket<'static>>>,
    sequence: u32,
) -> Result<Vec<u8>, Mp4Error> {
    let mut moof_body = Vec::new();
    moof_body.extend(write_mfhd(sequence));

    // Build each traf and remember where to patch the data_offset.
    let mut patches: Vec<usize> = Vec::new();
    let mut track_sizes: Vec<u64> = Vec::new();

    for (track_id, packets) in track_packets {
        let (traf_box, patch_pos) = write_traf(*track_id, packets)?;
        patches.push(moof_body.len() + patch_pos);
        track_sizes.push(
            packets
                .iter()
                .map(|p| p.payload.as_ref().len() as u64)
                .sum(),
        );
        moof_body.extend(traf_box);
    }

    // Patch data_offset values: offset from start of moof to first sample in mdat.
    let moof_size = 8 + moof_body.len() as u64;
    let mdat_payload_offset = moof_size + 8;
    let mut cumulative = 0u64;
    for (patch_pos, total_size) in patches.iter().zip(track_sizes.iter()) {
        let data_offset = (mdat_payload_offset + cumulative) as i32;
        let pos = *patch_pos;
        moof_body[pos..pos + 4].copy_from_slice(&data_offset.to_be_bytes());
        cumulative += total_size;
    }

    Ok(write_box(types::MOOF, &moof_body))
}

fn write_mfhd(sequence: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, sequence);
    write_box(types::MFHD, &body)
}

fn write_traf(
    track_id: u32,
    packets: &[MediaPacket<'static>],
) -> Result<(Vec<u8>, usize), Mp4Error> {
    let mut inner = Vec::new();
    let tfhd = write_tfhd(track_id);
    inner.extend_from_slice(&tfhd);
    let tfdt = write_tfdt(packets)?;
    inner.extend_from_slice(&tfdt);
    let (trun_box, patch_pos_in_trun) = write_trun(packets);
    inner.extend_from_slice(&trun_box);

    // patch position relative to the start of the full `traf` box (size/type included).
    let patch_pos = 8 + tfhd.len() + tfdt.len() + patch_pos_in_trun;
    Ok((write_box(types::TRAF, &inner), patch_pos))
}

fn write_tfhd(track_id: u32) -> Vec<u8> {
    // default-base-is-moof
    let flags = 0x020000;
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&write_fullbox(0, flags));
    write_u32(&mut body, track_id);
    write_box(types::TFHD, &body)
}

fn write_tfdt(packets: &[MediaPacket<'static>]) -> Result<Vec<u8>, Mp4Error> {
    let base = packets
        .first()
        .and_then(|p| p.time.dts.map(|t| t.ticks() as u64))
        .unwrap_or(0);
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&write_fullbox(1, 0));
    write_u64(&mut body, base);
    Ok(write_box(types::TFDT, &body))
}

fn write_trun(packets: &[MediaPacket<'static>]) -> (Vec<u8>, usize) {
    let mut body = Vec::new();
    let flags = 0x0000_0f01u32; // data_offset, duration, size, flags, composition offset
    body.extend_from_slice(&write_fullbox(1, flags));
    write_u32(&mut body, packets.len() as u32); // sample_count

    // Data offset placeholder; record its position relative to the start of the trun box.
    let data_offset_pos = body.len();
    write_u32(&mut body, 0);

    for pkt in packets {
        let duration = pkt.time.duration.map(|d| d.ticks() as u32).unwrap_or(0);
        let size = pkt.payload.as_ref().len() as u32;
        let composition_offset = pkt
            .time
            .pts
            .zip(pkt.time.dts)
            .map(|(p, d)| (p.ticks() - d.ticks()) as i32)
            .unwrap_or(0);
        let flags = if pkt.flags.is_keyframe {
            0x02000000
        } else {
            0x01010000
        };
        write_u32(&mut body, duration);
        write_u32(&mut body, size);
        write_u32(&mut body, flags);
        write_u32(&mut body, composition_offset as u32);
    }

    (write_box(types::TRUN, &body), data_offset_pos + 8)
}

fn write_mdat(track_packets: &BTreeMap<u32, Vec<MediaPacket<'static>>>) -> Vec<u8> {
    let mut body = Vec::new();
    for packets in track_packets.values() {
        for pkt in packets {
            body.extend_from_slice(pkt.payload.as_ref());
        }
    }
    write_box(types::MDAT, &body)
}
