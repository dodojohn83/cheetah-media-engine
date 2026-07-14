//! `moov` / `mvex` parsing into track state.

use alloc::collections::BTreeMap;
use cheetah_media_types::{CodecId, TimeBase, TrackId, TrackInfo, TrackKind};

use crate::Mp4Error;
use crate::boxes::{BoxHeader, Mp4Cursor, iter_boxes, read_fullbox_header, types};
use crate::sample_entry::{SampleEntry, parse_sample_entry};

/// Default per-track fragment values from `trex`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TrexDefaults {
    pub sample_description_index: u32,
    pub default_sample_duration: u64,
    pub default_sample_size: u64,
    pub default_sample_flags: u32,
}

/// Demuxer state extracted from the movie header.
#[derive(Debug, Clone)]
pub struct TrackData {
    pub track: TrackInfo,
    pub timescale: u32,
    pub track_type: u32, // e.g. types::AVC1
    pub trex: TrexDefaults,
}

/// Parse a `moov` box and any `mvex` defaults.
pub fn parse_moov(
    moov_body: &[u8],
    moov_offset: u64,
) -> Result<(BTreeMap<u32, TrackData>, u32), Mp4Error> {
    let mut tracks: BTreeMap<u32, TrackData> = BTreeMap::new();
    let mut movie_timescale = 1000u32;

    for item in iter_boxes(moov_body, moov_offset, 8)? {
        let (header, body) = item?;
        match header.box_type {
            types::MVHD => {
                movie_timescale = parse_mvhd(body)?;
            }
            types::TRAK => {
                if let Some((track_id, td)) = parse_trak(body, header.body_offset())? {
                    tracks.insert(track_id, td);
                }
            }
            types::MVEX => {
                parse_mvex(body, &mut tracks)?;
            }
            _ => {}
        }
    }

    Ok((tracks, movie_timescale))
}

fn parse_mvhd(data: &[u8]) -> Result<u32, Mp4Error> {
    let (version, _, body) = read_fullbox_header(data)?;
    if version == 1 {
        if body.len() < 28 {
            return Err(Mp4Error::NeedMoreData);
        }
        let timescale = u32::from_be_bytes([body[16], body[17], body[18], body[19]]);
        Ok(timescale)
    } else {
        if body.len() < 16 {
            return Err(Mp4Error::NeedMoreData);
        }
        let timescale = u32::from_be_bytes([body[8], body[9], body[10], body[11]]);
        Ok(timescale)
    }
}

fn parse_trak(data: &[u8], trak_offset: u64) -> Result<Option<(u32, TrackData)>, Mp4Error> {
    let mut track_id = None;
    let mut track_info: Option<TrackInfo> = None;
    let mut timescale = 1000u32;
    let mut track_type = 0u32;

    for item in iter_boxes(data, trak_offset, 8)? {
        let (header, body) = item?;
        match header.box_type {
            types::TKHD => {
                track_id = Some(parse_tkhd(body)?);
            }
            types::MDIA => {
                let (ti, ts, tt) = parse_mdia(body, header.body_offset())?;
                track_info = Some(ti);
                timescale = ts;
                track_type = tt;
            }
            _ => {}
        }
    }

    let id = match track_id {
        Some(v) => v,
        None => return Ok(None),
    };
    let info = match track_info {
        Some(v) => v,
        None => {
            return Err(Mp4Error::invalid_input(3201, Some("trak missing mdia")));
        }
    };

    Ok(Some((
        id,
        TrackData {
            track: info,
            timescale,
            track_type,
            trex: TrexDefaults::default(),
        },
    )))
}

fn parse_tkhd(data: &[u8]) -> Result<u32, Mp4Error> {
    let (version, _flags, body) = read_fullbox_header(data)?;
    if version == 1 {
        if body.len() < 24 {
            return Err(Mp4Error::NeedMoreData);
        }
        Ok(u32::from_be_bytes([body[16], body[17], body[18], body[19]]))
    } else {
        if body.len() < 12 {
            return Err(Mp4Error::NeedMoreData);
        }
        Ok(u32::from_be_bytes([body[8], body[9], body[10], body[11]]))
    }
}

fn parse_mdia(data: &[u8], mdia_offset: u64) -> Result<(TrackInfo, u32, u32), Mp4Error> {
    let mut handler = None;
    let mut timescale = 1000u32;
    let mut sample_entry: Option<SampleEntry> = None;

    for item in iter_boxes(data, mdia_offset, 8)? {
        let (header, body) = item?;
        match header.box_type {
            types::MDHD => {
                timescale = parse_mdhd(body)?;
            }
            types::HDLR => {
                handler = Some(parse_hdlr(body)?);
            }
            types::MINF => {
                sample_entry = parse_minf(body, header.body_offset())?;
            }
            _ => {}
        }
    }

    let kind = match handler {
        Some([b'v', b'i', b'd', b'e']) => TrackKind::Video,
        Some([b's', b'o', b'u', b'n']) => TrackKind::Audio,
        _ => return Err(Mp4Error::unsupported(3202, Some("unknown handler type"))),
    };

    // Assign a dummy track id; it is filled from tkhd later.
    let dummy_id = TrackId::new(1).unwrap();
    let codec = sample_entry
        .as_ref()
        .map(|s| s.codec)
        .unwrap_or(CodecId::Unknown(0x0000_0000));
    let timebase = TimeBase::from_timescale(timescale).ok_or(Mp4Error::invalid_input(
        3203,
        Some("invalid mdhd timescale"),
    ))?;
    let mut info = TrackInfo::new(dummy_id, kind, codec, timebase);
    if let Some(entry) = sample_entry {
        entry.apply(&mut info);
    }
    let track_type = if kind == TrackKind::Video {
        types::AVC1
    } else {
        types::MP4A
    };
    Ok((info, timescale, track_type))
}

fn parse_mdhd(data: &[u8]) -> Result<u32, Mp4Error> {
    let (version, _, body) = read_fullbox_header(data)?;
    if version == 1 {
        if body.len() < 28 {
            return Err(Mp4Error::NeedMoreData);
        }
        Ok(u32::from_be_bytes([body[16], body[17], body[18], body[19]]))
    } else {
        if body.len() < 16 {
            return Err(Mp4Error::NeedMoreData);
        }
        Ok(u32::from_be_bytes([body[8], body[9], body[10], body[11]]))
    }
}

fn parse_hdlr(data: &[u8]) -> Result<[u8; 4], Mp4Error> {
    let (_, _, body) = read_fullbox_header(data)?;
    if body.len() < 8 {
        return Err(Mp4Error::NeedMoreData);
    }
    Ok([body[4], body[5], body[6], body[7]])
}

fn parse_minf(data: &[u8], minf_offset: u64) -> Result<Option<SampleEntry>, Mp4Error> {
    for item in iter_boxes(data, minf_offset, 8)? {
        let (header, body) = item?;
        if header.box_type == types::STBL {
            return parse_stbl(body, header.body_offset());
        }
    }
    Ok(None)
}

fn parse_stbl(data: &[u8], stbl_offset: u64) -> Result<Option<SampleEntry>, Mp4Error> {
    for item in iter_boxes(data, stbl_offset, 8)? {
        let (header, body) = item?;
        if header.box_type == types::STSD {
            return parse_stsd(body);
        }
    }
    Ok(None)
}

fn parse_stsd(data: &[u8]) -> Result<Option<SampleEntry>, Mp4Error> {
    if data.len() < 8 {
        return Err(Mp4Error::NeedMoreData);
    }
    let entry_count = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
    if entry_count == 0 {
        return Ok(None);
    }
    if entry_count > crate::boxes::MAX_TABLE_ENTRIES {
        return Err(Mp4Error::LimitExceeded {
            limit: "stsd entries",
        });
    }
    let mut cursor = Mp4Cursor::new(&data[8..]);
    for _ in 0..entry_count {
        let header = BoxHeader::parse(cursor.rest(), 0)?;
        if header.size as usize > cursor.remaining() {
            return Err(Mp4Error::NeedMoreData);
        }
        let body = cursor.read_bytes(header.size as usize)?;
        // The first 8 bytes of `body` are the box header we just parsed; skip them.
        let inner = &body[header.header_size as usize..];
        if let Some(entry) = parse_sample_entry(header.box_type, inner)? {
            return Ok(Some(entry));
        }
    }
    Ok(None)
}

fn parse_mvex(data: &[u8], tracks: &mut BTreeMap<u32, TrackData>) -> Result<(), Mp4Error> {
    for item in iter_boxes(data, 0, 8)? {
        let (header, body) = item?;
        if header.box_type == types::TREX {
            let trex = parse_trex(body)?;
            if let Some(td) = tracks.get_mut(&trex.0) {
                td.trex = trex.1;
            }
        }
    }
    Ok(())
}

fn parse_trex(data: &[u8]) -> Result<(u32, TrexDefaults), Mp4Error> {
    let (_, _, body) = read_fullbox_header(data)?;
    if body.len() < 20 {
        return Err(Mp4Error::NeedMoreData);
    }
    let track_id = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    let sample_description_index = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);
    let default_sample_duration = u32::from_be_bytes([body[8], body[9], body[10], body[11]]) as u64;
    let default_sample_size = u32::from_be_bytes([body[12], body[13], body[14], body[15]]) as u64;
    let default_sample_flags = u32::from_be_bytes([body[16], body[17], body[18], body[19]]);
    Ok((
        track_id,
        TrexDefaults {
            sample_description_index,
            default_sample_duration,
            default_sample_size,
            default_sample_flags,
        },
    ))
}
