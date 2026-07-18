//! Progressive/seekable MP4 muxer (non-fragmented).
//!
//! Produces a self-contained `ftyp` + `moov` + `mdat` file suitable for
//! download or storage. All samples are kept in memory until `finish()` is
//! called, so this is intended for short recordings or bounded clips, not
//! indefinite live streaming.

use alloc::vec::Vec;
use cheetah_media_types::{MediaPacket, TrackKind};

use crate::Mp4Error;
use crate::boxes::{iter_boxes, types, write_box, write_fullbox, write_u32};
use crate::muxer::{
    TrackConfig, write_dinf, write_hdlr, write_mdhd, write_mvhd, write_smhd, write_stsd,
    write_tkhd, write_vmhd,
};

/// Sample buffered by [`Mp4Muxer`].
#[derive(Debug)]
struct Sample {
    dts: i64,
    pts: i64,
    duration: u64,
    payload: Vec<u8>,
    keyframe: bool,
}

/// Seekable, progressive MP4 muxer for a single track.
///
/// The resulting file can be played from local storage. Samples are buffered
/// in memory until [`finish`](Self::finish) is called; callers should bound
/// the recording duration to avoid unbounded growth.
#[derive(Debug)]
pub struct Mp4Muxer {
    config: Option<TrackConfig>,
    samples: Vec<Sample>,
}

impl Mp4Muxer {
    /// Create a new muxer.
    pub fn new() -> Self {
        Self {
            config: None,
            samples: Vec::new(),
        }
    }

    /// Configure the single track.
    pub fn configure(&mut self, config: TrackConfig) {
        self.config = Some(config);
    }

    /// Add a packet to the recording.
    ///
    /// Returns an error if no track is configured or the packet's track id
    /// does not match the configured track.
    pub fn push_packet(&mut self, packet: MediaPacket<'_>) -> Result<(), Mp4Error> {
        let cfg = self
            .config
            .as_ref()
            .ok_or(Mp4Error::unsupported(3600, Some("no track configured")))?;
        if packet.track_id.get() != cfg.track_id {
            return Err(Mp4Error::invalid_input(
                3601,
                Some("packet track id does not match configured track"),
            ));
        }

        let dts = packet.time.dts.map(|t| t.ticks()).unwrap_or(0);
        let pts = packet.time.pts.map(|t| t.ticks()).unwrap_or(dts);
        let duration = packet
            .time
            .duration
            .map(|t| t.ticks())
            .and_then(|d| u64::try_from(d).ok())
            .unwrap_or(0);
        self.samples.push(Sample {
            dts,
            pts,
            duration,
            payload: packet.payload.as_ref().to_vec(),
            keyframe: packet.flags.is_keyframe,
        });
        Ok(())
    }

    /// Finalize the recording and return a complete MP4 file.
    ///
    /// The output is `ftyp` + `moov` + `mdat`. The `stco` chunk offset is
    /// patched after the `moov` size is known. If the total payload length
    /// exceeds `u32::MAX`, the size is written using the 64-bit extended-size
    /// form.
    pub fn finish(&self) -> Result<Vec<u8>, Mp4Error> {
        let cfg = self
            .config
            .as_ref()
            .ok_or(Mp4Error::unsupported(3602, Some("no track configured")))?;
        if self.samples.is_empty() {
            return Err(Mp4Error::invalid_input(
                3603,
                Some("cannot finalize an empty recording"),
            ));
        }

        let ftyp = write_ftyp();
        let (moov_body, stco_patch_position) = self.write_moov_body(cfg)?;

        let total_payload: u64 = self.samples.iter().map(|s| s.payload.len() as u64).sum();
        let needs_extended = total_payload.saturating_add(8) > u32::MAX as u64;
        let mdat_header_size: u64 = if needs_extended { 16 } else { 8 };
        let mdat_total_size = total_payload
            .checked_add(mdat_header_size)
            .ok_or_else(|| Mp4Error::limit_exceeded("mdat size overflow"))?;

        let moov_len = moov_body
            .len()
            .checked_add(8)
            .ok_or_else(|| Mp4Error::limit_exceeded("moov box size overflow"))?;
        let chunk_offset = ftyp
            .len()
            .checked_add(moov_len)
            .and_then(|n| n.checked_add(mdat_header_size as usize))
            .ok_or_else(|| Mp4Error::limit_exceeded("chunk offset overflow"))?;
        let mut moov_body = moov_body;
        if let Some(pos) = stco_patch_position {
            let offset = u32::try_from(chunk_offset)
                .map_err(|_| Mp4Error::limit_exceeded("chunk offset exceeds u32"))?;
            moov_body[pos..pos + 4].copy_from_slice(&offset.to_be_bytes());
        }
        let moov = write_box(types::MOOV, &moov_body);

        if mdat_total_size > isize::MAX as u64 {
            return Err(Mp4Error::limit_exceeded(
                "mdat total size exceeds addressable memory",
            ));
        }
        let mdat_capacity = usize::try_from(mdat_total_size)
            .map_err(|_| Mp4Error::limit_exceeded("mdat size exceeds usize"))?;
        let mut mdat = Vec::with_capacity(mdat_capacity);
        if needs_extended {
            mdat.extend_from_slice(&[0, 0, 0, 1]);
            mdat.extend_from_slice(&types::MDAT.to_be_bytes());
            mdat.extend_from_slice(&mdat_total_size.to_be_bytes());
        } else {
            mdat.extend_from_slice(&(mdat_total_size as u32).to_be_bytes());
            mdat.extend_from_slice(&types::MDAT.to_be_bytes());
        }
        for s in &self.samples {
            mdat.extend_from_slice(&s.payload);
        }

        let mut out = Vec::with_capacity(ftyp.len() + moov.len() + mdat.len());
        out.extend_from_slice(&ftyp);
        out.extend_from_slice(&moov);
        out.extend_from_slice(&mdat);
        Ok(out)
    }

    fn write_moov_body(&self, cfg: &TrackConfig) -> Result<(Vec<u8>, Option<usize>), Mp4Error> {
        let mut moov_body = Vec::new();
        moov_body.extend(write_mvhd_single(cfg)?);

        let trak = self.write_trak(cfg)?;
        moov_body.extend_from_slice(&trak);

        // Patch position is relative to the start of `moov_body`.
        // It will be set later when building the stbl.
        let stco_patch_position = self.find_stco_offset_position(&moov_body)?;

        Ok((moov_body, stco_patch_position))
    }

    fn find_stco_offset_position(&self, moov_body: &[u8]) -> Result<Option<usize>, Mp4Error> {
        // Navigate moov -> trak -> mdia -> minf -> stbl -> stco and locate
        // the 4-byte chunk offset entry inside the stco box body.
        let stco_body_offset = find_nested_box_offset(
            moov_body,
            0,
            &[
                types::TRAK,
                types::MDIA,
                types::MINF,
                types::STBL,
                types::STCO,
            ],
        )
        .ok_or(Mp4Error::invalid_input(3604, Some("stco box not found")))?;
        // stco body: version/flags (4) + entry_count (4) + offset entries.
        let pos = usize::try_from(stco_body_offset)
            .map_err(|_| Mp4Error::limit_exceeded("stco patch offset exceeds usize"))?;
        let end = pos
            .checked_add(8)
            .ok_or_else(|| Mp4Error::limit_exceeded("stco patch position overflow"))?;
        if end > moov_body.len() {
            return Err(Mp4Error::invalid_input(
                3605,
                Some("stco patch out of bounds"),
            ));
        }
        Ok(Some(end))
    }

    fn write_trak(&self, cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
        let mut body = Vec::new();
        body.extend(write_tkhd(cfg));
        body.extend(self.write_mdia(cfg)?);
        Ok(write_box(types::TRAK, &body))
    }

    fn write_mdia(&self, cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
        let mut body = Vec::new();
        body.extend(write_mdhd(cfg));
        body.extend(write_hdlr(cfg));
        body.extend(self.write_minf(cfg)?);
        Ok(write_box(types::MDIA, &body))
    }

    fn write_minf(&self, cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
        let mut body = Vec::new();
        if cfg.kind == TrackKind::Video {
            body.extend(write_vmhd());
        } else {
            body.extend(write_smhd());
        }
        body.extend(write_dinf());
        body.extend(self.write_stbl(cfg)?);
        Ok(write_box(types::MINF, &body))
    }

    fn write_stbl(&self, cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
        let mut body = Vec::new();
        body.extend(write_stsd(cfg)?);

        let stts_runs = run_length_encode_u64(self.samples.iter().map(|s| s.duration));
        body.extend(write_stts(&stts_runs)?);

        let has_b_frames = self.samples.iter().any(|s| s.pts != s.dts);
        if has_b_frames {
            let ctts_runs =
                run_length_encode_i64(self.samples.iter().map(|s| s.pts.saturating_sub(s.dts)));
            body.extend(write_ctts(&ctts_runs)?);
        }

        body.extend(write_stsc(
            1,
            u32::try_from(self.samples.len())
                .map_err(|_| Mp4Error::limit_exceeded("stsc samples per chunk"))?,
            1,
        ));
        let sizes: Vec<u32> = self
            .samples
            .iter()
            .map(|s| {
                u32::try_from(s.payload.len())
                    .map_err(|_| Mp4Error::limit_exceeded("sample size exceeds u32"))
            })
            .collect::<Result<_, _>>()?;
        body.extend(write_stsz(&sizes)?);

        // stco with a placeholder offset.
        body.extend(write_stco_placeholder());

        let all_key = self.samples.iter().all(|s| s.keyframe);
        if !all_key {
            let syncs: Vec<u32> = self
                .samples
                .iter()
                .enumerate()
                .filter(|(_, s)| s.keyframe)
                .map(|(i, _)| {
                    u32::try_from(i + 1).map_err(|_| Mp4Error::limit_exceeded("stss entry index"))
                })
                .collect::<Result<_, _>>()?;
            body.extend(write_stss(&syncs)?);
        }

        Ok(write_box(types::STBL, &body))
    }
}

impl Default for Mp4Muxer {
    fn default() -> Self {
        Self::new()
    }
}

fn write_ftyp() -> Vec<u8> {
    let mut body = Vec::with_capacity(24);
    body.extend_from_slice(b"isom");
    body.extend_from_slice(&[0x00, 0x00, 0x02, 0x00]); // minor version
    body.extend_from_slice(b"isom");
    body.extend_from_slice(b"mp41");
    write_box(types::FTYP, &body)
}

fn write_mvhd_single(cfg: &TrackConfig) -> Result<Vec<u8>, Mp4Error> {
    let mut configs = alloc::collections::BTreeMap::new();
    configs.insert(cfg.track_id, cfg.clone());
    write_mvhd(&configs)
}

fn write_stts(runs: &[(u32, u32)]) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::with_capacity(8 + runs.len() * 8);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(
        &mut body,
        u32::try_from(runs.len()).map_err(|_| Mp4Error::limit_exceeded("stts entry count"))?,
    );
    for (count, delta) in runs {
        write_u32(&mut body, *count);
        write_u32(&mut body, *delta);
    }
    Ok(write_box(types::STTS, &body))
}

fn write_ctts(runs: &[(u32, i32)]) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::with_capacity(8 + runs.len() * 8);
    body.extend_from_slice(&write_fullbox(1, 0));
    write_u32(
        &mut body,
        u32::try_from(runs.len()).map_err(|_| Mp4Error::limit_exceeded("ctts entry count"))?,
    );
    for (count, offset) in runs {
        write_u32(&mut body, *count);
        write_u32(&mut body, *offset as u32);
    }
    Ok(write_box(types::CTTS, &body))
}

fn write_stsc(first_chunk: u32, samples_per_chunk: u32, sample_description_index: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, 1); // entry_count
    write_u32(&mut body, first_chunk);
    write_u32(&mut body, samples_per_chunk);
    write_u32(&mut body, sample_description_index);
    write_box(types::STSC, &body)
}

fn write_stsz(sizes: &[u32]) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::with_capacity(12 + sizes.len() * 4);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, 0); // sample_size 0 -> per-sample sizes follow
    write_u32(
        &mut body,
        u32::try_from(sizes.len()).map_err(|_| Mp4Error::limit_exceeded("stsz entry count"))?,
    );
    for s in sizes {
        write_u32(&mut body, *s);
    }
    Ok(write_box(types::STSZ, &body))
}

fn write_stco_placeholder() -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(&mut body, 1); // entry_count
    write_u32(&mut body, 0); // placeholder offset
    write_box(types::STCO, &body)
}

fn write_stss(syncs: &[u32]) -> Result<Vec<u8>, Mp4Error> {
    let mut body = Vec::with_capacity(8 + syncs.len() * 4);
    body.extend_from_slice(&write_fullbox(0, 0));
    write_u32(
        &mut body,
        u32::try_from(syncs.len()).map_err(|_| Mp4Error::limit_exceeded("stss entry count"))?,
    );
    for s in syncs {
        write_u32(&mut body, *s);
    }
    Ok(write_box(types::STSS, &body))
}

fn run_length_encode_u64<I>(values: I) -> Vec<(u32, u32)>
where
    I: Iterator<Item = u64>,
{
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for v in values {
        let v = u32::try_from(v).unwrap_or(u32::MAX);
        if let Some((count, delta)) = runs.last_mut()
            && *delta == v
        {
            *count = count.saturating_add(1);
            continue;
        }
        runs.push((1, v));
    }
    runs
}

fn run_length_encode_i64<I>(values: I) -> Vec<(u32, i32)>
where
    I: Iterator<Item = i64>,
{
    let mut runs: Vec<(u32, i32)> = Vec::new();
    for v in values {
        let v = i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX });
        if let Some((count, offset)) = runs.last_mut()
            && *offset == v
        {
            *count = count.saturating_add(1);
            continue;
        }
        runs.push((1, v));
    }
    runs
}

/// Find the body offset of a nested box by following a path of four-cc types.
///
/// `parent_offset` is the absolute stream offset of `parent_data`. The returned
/// offset is the absolute stream offset of the target box body.
fn find_nested_box_offset(parent_data: &[u8], parent_offset: u64, path: &[u32]) -> Option<u64> {
    let mut data = parent_data;
    let mut offset = parent_offset;
    for (i, target) in path.iter().enumerate() {
        let mut found = None;
        for item in iter_boxes(data, offset, 4).ok()? {
            let (header, body) = item.ok()?;
            if header.box_type == *target {
                found = Some((header.body_offset(), body));
                break;
            }
        }
        let (body_offset, body) = found?;
        if i == path.len() - 1 {
            return Some(body_offset);
        }
        data = body;
        offset = body_offset;
    }
    None
}
