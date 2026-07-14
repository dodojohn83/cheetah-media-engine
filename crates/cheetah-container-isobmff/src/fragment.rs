//! `moof` / `trun` / `mdat` fragment parsing.

use alloc::vec::Vec;
use cheetah_media_types::{
    BufferRef, MediaPacket, MediaTime, PacketFlags, SequenceNumber, StreamEpoch,
};

use crate::Mp4Error;
use crate::boxes::{Mp4Cursor, iter_boxes, read_fullbox_header, types};
use crate::moov::TrackData;

/// Parsed `trun` entry after defaults and flags have been resolved.
#[derive(Debug, Clone, Copy)]
pub struct FragmentSample {
    pub duration: u64,
    pub size: u64,
    pub flags: u32,
    pub composition_offset: i64,
    pub data_offset: u64,
}

/// A fully parsed movie fragment for one track.
#[derive(Debug, Clone)]
pub struct TrackFragment {
    pub track_id: u32,
    pub base_decode_time: u64,
    pub default_sample_duration: u64,
    pub default_sample_size: u64,
    pub default_sample_flags: u32,
    pub first_sample_flags: Option<u32>,
    pub samples: Vec<FragmentSample>,
    pub data_offset_base: u64,
    pub moof_offset: u64,
}

/// Parse a `moof` box and produce `TrackFragment`s for each `traf`.
///
/// `moof_offset` is the absolute stream offset of the `moof` box start.
pub fn parse_moof(
    moof_body: &[u8],
    moof_offset: u64,
    tracks: &[TrackData],
) -> Result<Vec<TrackFragment>, Mp4Error> {
    let mut fragments = Vec::new();
    for item in iter_boxes(moof_body, moof_offset, 8)? {
        let (header, body) = item?;
        if header.box_type == types::TRAF
            && let Some(tf) = parse_traf(body, header.body_offset(), moof_offset, tracks)?
        {
            fragments.push(tf);
        }
    }
    Ok(fragments)
}

fn parse_traf(
    traf_body: &[u8],
    traf_offset: u64,
    moof_offset: u64,
    tracks: &[TrackData],
) -> Result<Option<TrackFragment>, Mp4Error> {
    let mut tfhd: Option<Tfhd> = None;
    let mut tfdt: Option<Tfdt> = None;
    let mut trun: Option<Trun> = None;

    for item in iter_boxes(traf_body, traf_offset, 8)? {
        let (header, body) = item?;
        match header.box_type {
            types::TFHD => tfhd = Some(parse_tfhd(body)?),
            types::TFDT => tfdt = Some(parse_tfdt(body)?),
            types::TRUN => trun = Some(parse_trun(body)?),
            _ => {}
        }
    }

    let tfhd = match tfhd {
        Some(v) => v,
        None => return Ok(None),
    };

    let track_id = tfhd.track_id;
    let trex = tracks
        .iter()
        .find(|t| t.track.id.get() == track_id)
        .map(|t| t.trex)
        .unwrap_or_default();

    let base_decode_time = match tfdt {
        Some(t) => t.base_media_decode_time,
        None => 0,
    };

    let flags = tfhd.flags;
    let mut default_sample_duration = trex.default_sample_duration;
    let mut default_sample_size = trex.default_sample_size;
    let mut default_sample_flags = trex.default_sample_flags;

    if flags & TFHD_DEFAULT_SAMPLE_DURATION != 0 {
        default_sample_duration = tfhd.default_sample_duration;
    }
    if flags & TFHD_DEFAULT_SAMPLE_SIZE != 0 {
        default_sample_size = tfhd.default_sample_size;
    }
    if flags & TFHD_DEFAULT_SAMPLE_FLAGS != 0 {
        default_sample_flags = tfhd.default_sample_flags;
    }

    let trun = match trun {
        Some(v) => v,
        None => return Ok(None),
    };

    let data_offset_base = if flags & TFHD_BASE_DATA_OFFSET != 0 {
        tfhd.base_data_offset
    } else {
        moof_offset
    };

    let mut cumulative_size = 0u64;
    let mut samples = Vec::with_capacity(trun.sample_count.min(4096) as usize);
    for i in 0..trun.sample_count {
        let duration = trun.entries[i as usize]
            .duration
            .unwrap_or(default_sample_duration);
        let size = trun.entries[i as usize].size.unwrap_or(default_sample_size);
        let mut sample_flags = trun.entries[i as usize]
            .flags
            .unwrap_or(default_sample_flags);
        let composition_offset = trun.entries[i as usize].composition_offset.unwrap_or(0);

        if i == 0
            && let Some(flags) = trun.first_sample_flags
        {
            sample_flags = flags;
        }

        let data_offset = if let Some(d) = trun.data_offset {
            data_offset_base + d as u64 + cumulative_size
        } else {
            data_offset_base + cumulative_size
        };

        samples.push(FragmentSample {
            duration,
            size,
            flags: sample_flags,
            composition_offset,
            data_offset,
        });
        cumulative_size += size;
    }

    Ok(Some(TrackFragment {
        track_id,
        base_decode_time,
        default_sample_duration,
        default_sample_size,
        default_sample_flags,
        first_sample_flags: trun.first_sample_flags,
        samples,
        data_offset_base,
        moof_offset,
    }))
}

const TFHD_BASE_DATA_OFFSET: u32 = 0x0000_0001;
const TFHD_SAMPLE_DESCRIPTION_INDEX: u32 = 0x0000_0002;
const TFHD_DEFAULT_SAMPLE_DURATION: u32 = 0x0000_0008;
const TFHD_DEFAULT_SAMPLE_SIZE: u32 = 0x0000_0010;
const TFHD_DEFAULT_SAMPLE_FLAGS: u32 = 0x0000_0020;

#[derive(Debug, Clone, Copy, Default)]
struct Tfhd {
    pub flags: u32,
    pub track_id: u32,
    pub base_data_offset: u64,
    pub sample_description_index: u32,
    pub default_sample_duration: u64,
    pub default_sample_size: u64,
    pub default_sample_flags: u32,
}

fn parse_tfhd(data: &[u8]) -> Result<Tfhd, Mp4Error> {
    let (version, flags, body) = read_fullbox_header(data)?;
    let _ = version;
    let mut cursor = Mp4Cursor::new(body);
    let track_id = cursor.read_u32()?;
    let mut tfhd = Tfhd {
        flags,
        track_id,
        ..Default::default()
    };
    if flags & TFHD_BASE_DATA_OFFSET != 0 {
        tfhd.base_data_offset = cursor.read_u64()?;
    }
    if flags & TFHD_SAMPLE_DESCRIPTION_INDEX != 0 {
        tfhd.sample_description_index = cursor.read_u32()?;
    }
    if flags & TFHD_DEFAULT_SAMPLE_DURATION != 0 {
        tfhd.default_sample_duration = cursor.read_u32()? as u64;
    }
    if flags & TFHD_DEFAULT_SAMPLE_SIZE != 0 {
        tfhd.default_sample_size = cursor.read_u32()? as u64;
    }
    if flags & TFHD_DEFAULT_SAMPLE_FLAGS != 0 {
        tfhd.default_sample_flags = cursor.read_u32()?;
    }
    Ok(tfhd)
}

#[derive(Debug, Clone, Copy)]
struct Tfdt {
    pub base_media_decode_time: u64,
}

fn parse_tfdt(data: &[u8]) -> Result<Tfdt, Mp4Error> {
    let (version, _flags, body) = read_fullbox_header(data)?;
    if body.len() < if version == 1 { 8 } else { 4 } {
        return Err(Mp4Error::NeedMoreData);
    }
    let base = if version == 1 {
        u64::from_be_bytes([
            body[0], body[1], body[2], body[3], body[4], body[5], body[6], body[7],
        ])
    } else {
        u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as u64
    };
    Ok(Tfdt {
        base_media_decode_time: base,
    })
}

const TRUN_DATA_OFFSET_PRESENT: u32 = 0x0000_0001;
const TRUN_FIRST_SAMPLE_FLAGS_PRESENT: u32 = 0x0000_0004;
const TRUN_SAMPLE_DURATION_PRESENT: u32 = 0x0000_0100;
const TRUN_SAMPLE_SIZE_PRESENT: u32 = 0x0000_0200;
const TRUN_SAMPLE_FLAGS_PRESENT: u32 = 0x0000_0400;
const TRUN_SAMPLE_COMPOSITION_TIME_OFFSET_PRESENT: u32 = 0x0000_0800;

#[derive(Debug, Clone)]
struct Trun {
    pub sample_count: u32,
    pub data_offset: Option<i32>,
    pub first_sample_flags: Option<u32>,
    pub entries: Vec<TrunEntry>,
}

#[derive(Debug, Clone, Copy, Default)]
struct TrunEntry {
    pub duration: Option<u64>,
    pub size: Option<u64>,
    pub flags: Option<u32>,
    pub composition_offset: Option<i64>,
}

fn parse_trun(data: &[u8]) -> Result<Trun, Mp4Error> {
    let (version, flags, body) = read_fullbox_header(data)?;
    if body.len() < 4 {
        return Err(Mp4Error::NeedMoreData);
    }
    let sample_count = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    if sample_count as usize > crate::boxes::MAX_TABLE_ENTRIES {
        return Err(Mp4Error::LimitExceeded {
            limit: "trun entries",
        });
    }
    let mut cursor = Mp4Cursor::new(&body[4..]);
    let data_offset = if flags & TRUN_DATA_OFFSET_PRESENT != 0 {
        Some(cursor.read_i32()?)
    } else {
        None
    };
    let first_sample_flags = if flags & TRUN_FIRST_SAMPLE_FLAGS_PRESENT != 0 {
        Some(cursor.read_u32()?)
    } else {
        None
    };

    let mut entries = Vec::with_capacity(sample_count.min(4096) as usize);
    for _ in 0..sample_count {
        let mut e = TrunEntry::default();
        if flags & TRUN_SAMPLE_DURATION_PRESENT != 0 {
            e.duration = Some(cursor.read_u32()? as u64);
        }
        if flags & TRUN_SAMPLE_SIZE_PRESENT != 0 {
            e.size = Some(cursor.read_u32()? as u64);
        }
        if flags & TRUN_SAMPLE_FLAGS_PRESENT != 0 {
            e.flags = Some(cursor.read_u32()?);
        }
        if flags & TRUN_SAMPLE_COMPOSITION_TIME_OFFSET_PRESENT != 0 {
            if version == 1 {
                e.composition_offset = Some(i64::from(cursor.read_i32()?));
            } else {
                e.composition_offset = Some(i64::from(cursor.read_u32()?));
            }
        }
        entries.push(e);
    }

    Ok(Trun {
        sample_count,
        data_offset,
        first_sample_flags,
        entries,
    })
}

/// Extract `MediaPacket`s from an `mdat` buffer using a parsed `TrackFragment`.
pub fn emit_packets(
    tf: &TrackFragment,
    mdat_buf: &BufferRef<'static>,
    mdat_data_offset: u64,
    track: &TrackData,
    sequence: &mut u64,
    epoch: StreamEpoch,
) -> Result<Vec<MediaPacket<'static>>, Mp4Error> {
    let mut out = Vec::with_capacity(tf.samples.len());
    let mut dts = tf.base_decode_time;
    let timebase = track.track.timebase;
    let mdat_len = mdat_buf.len();
    for s in &tf.samples {
        let start = s.data_offset.saturating_sub(mdat_data_offset) as usize;
        let end = start.saturating_add(s.size as usize);
        if end > mdat_len {
            return Err(Mp4Error::invalid_input(
                3301,
                Some("sample extends beyond mdat"),
            ));
        }
        let payload = mdat_buf.clone().slice(start..end);
        let pts = dts.saturating_add(s.composition_offset as u64);
        let is_key = !is_non_sync_sample(s.flags);
        let time = MediaTime::from_ticks(
            Some(pts as i64),
            Some(dts as i64),
            Some(s.duration as i64),
            timebase,
        );
        let seq = SequenceNumber::new(*sequence);
        *sequence += 1;
        out.push(MediaPacket {
            payload,
            track_id: track.track.id,
            stream_epoch: epoch,
            sequence: seq,
            time,
            flags: PacketFlags {
                is_keyframe: is_key,
                is_corrupt: false,
                is_discontinuity: false,
            },
        });
        dts = dts.saturating_add(s.duration);
    }
    Ok(out)
}

/// Return true if the sample_flags indicate a non-sync (non-key) frame.
fn is_non_sync_sample(flags: u32) -> bool {
    // bit 16: is_non_sync_sample
    ((flags >> 16) & 0x01) != 0
}
