//! Tests for the ISOBMFF parser, muxer, and demuxer.

use alloc::vec::Vec;
use cheetah_media_types::{
    CodecConfig, CodecId, MediaPacket, MediaTime, SequenceNumber, StreamEpoch, TimeBase, TrackId,
    TrackInfo, TrackKind,
};

use crate::{
    FragmentedMp4Muxer, IsobmffDemuxer, Mp4Event, ProgressiveMp4Muxer, SegmentOutput, TrackConfig,
    boxes::{BoxHeader, iter_boxes, read_fullbox_header, types},
    fragment::{FragmentSample, TrackFragment, emit_packets},
    moov::{TrackData, TrexDefaults},
};

fn make_audio_config() -> TrackConfig {
    let asc = cheetah_media_bitstream::AudioSpecificConfig {
        audio_object_type: 2,
        sampling_frequency_index: 4,
        sampling_frequency: 44100,
        channel_configuration: 1,
        channel_count: 1,
    };
    TrackConfig {
        track_id: 1,
        kind: TrackKind::Audio,
        codec: CodecId::Aac,
        codec_config: CodecConfig::AacAudioSpecificConfig(asc.build()),
        timescale: 44100,
        sample_entry_type: types::MP4A,
        width: 0,
        height: 0,
        sample_rate: 44100,
        channel_count: 1,
        default_sample_duration: 1024,
    }
}

fn make_video_config() -> TrackConfig {
    let sps = [
        0x67u8, 0x42, 0x00, 0x1e, 0xe9, 0x42, 0x10, 0x89, 0xf3, 0x22, 0xcb, 0x80,
    ];
    let pps = [0x68u8, 0xce, 0x3c, 0x80];
    let mut avcc = Vec::with_capacity(30);
    avcc.extend_from_slice(&[
        1,    // configurationVersion
        0x42, // profile
        0x00, // profile compatibility
        0x1e, // level
        0xff, // lengthSizeMinusOne=3
        0xe1, // sps count
    ]);
    avcc.extend_from_slice(&(sps.len() as u16).to_be_bytes());
    avcc.extend_from_slice(&sps);
    avcc.push(1); // pps count
    avcc.extend_from_slice(&(pps.len() as u16).to_be_bytes());
    avcc.extend_from_slice(&pps);

    TrackConfig {
        track_id: 1,
        kind: TrackKind::Video,
        codec: CodecId::H264,
        codec_config: CodecConfig::AvcC(avcc),
        timescale: 30_000,
        sample_entry_type: types::AVC1,
        width: 320,
        height: 240,
        sample_rate: 0,
        channel_count: 0,
        default_sample_duration: 3000,
    }
}

fn make_audio_packet(
    track_id: u32,
    sequence: u64,
    dts: i64,
    payload: Vec<u8>,
) -> MediaPacket<'static> {
    let time = MediaTime::from_timescale(Some(dts), Some(dts), Some(1024), 44100).unwrap();
    let mut pkt = MediaPacket::new(
        payload,
        TrackId::new(track_id).unwrap(),
        StreamEpoch::new(0),
        SequenceNumber::new(sequence),
        time,
    );
    pkt.flags.is_keyframe = true;
    pkt
}

fn make_video_packet(
    track_id: u32,
    sequence: u64,
    dts: i64,
    pts: i64,
    keyframe: bool,
    payload: Vec<u8>,
) -> MediaPacket<'static> {
    let time = MediaTime::from_timescale(Some(pts), Some(dts), Some(3000), 30_000).unwrap();
    let mut pkt = MediaPacket::new(
        payload,
        TrackId::new(track_id).unwrap(),
        StreamEpoch::new(0),
        SequenceNumber::new(sequence),
        time,
    );
    pkt.flags.is_keyframe = keyframe;
    pkt
}

fn collect_packets_from_muxer_and_demux(
    muxer: &mut FragmentedMp4Muxer,
) -> Vec<MediaPacket<'static>> {
    let SegmentOutput {
        init_segment,
        media_segment,
        ..
    } = muxer.flush_segment().unwrap().unwrap();

    let mut demuxer = IsobmffDemuxer::new();
    demuxer.push(init_segment.unwrap().as_ref());
    let media = media_segment.unwrap();
    demuxer.push(media.as_ref());

    let mut packets = Vec::new();
    for _ in 0..100 {
        match demuxer.next_event().unwrap() {
            None => break,
            Some(Mp4Event::Packet(p)) => packets.push(p),
            Some(_) => {}
        }
    }
    packets
}

// Helpers for standard/progressive MP4 verification.
fn find_box(data: &[u8], fourcc: u32) -> Option<(BoxHeader, &[u8])> {
    iter_boxes(data, 0, 4)
        .ok()?
        .find(|item| {
            item.as_ref()
                .map(|(h, _)| h.box_type == fourcc)
                .unwrap_or(false)
        })
        .and_then(|item| item.ok())
}

fn read_u32_be(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn parse_stsz(body: &[u8]) -> Vec<u32> {
    if body.len() < 8 {
        return Vec::new();
    }
    let sample_size = read_u32_be(body, 4);
    let sample_count = read_u32_be(body, 8) as usize;
    if sample_size != 0 {
        // all samples same size
        return vec![sample_size; sample_count];
    }
    let mut sizes = Vec::with_capacity(sample_count);
    for i in 0..sample_count {
        let off = 12 + i * 4;
        if off + 4 > body.len() {
            break;
        }
        sizes.push(read_u32_be(body, off));
    }
    sizes
}

fn parse_stco(body: &[u8]) -> Vec<u64> {
    if body.len() < 8 {
        return Vec::new();
    }
    let entry_count = read_u32_be(body, 4) as usize;
    let mut offsets = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        let off = 8 + i * 4;
        if off + 4 > body.len() {
            break;
        }
        offsets.push(read_u32_be(body, off) as u64);
    }
    offsets
}

fn parse_stss(body: &[u8]) -> Vec<u32> {
    if body.len() < 8 {
        return Vec::new();
    }
    let entry_count = read_u32_be(body, 4) as usize;
    let mut syncs = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        let off = 8 + i * 4;
        if off + 4 > body.len() {
            break;
        }
        syncs.push(read_u32_be(body, off));
    }
    syncs
}

fn total_duration_from_stts(body: &[u8]) -> u64 {
    if body.len() < 8 {
        return 0;
    }
    let entry_count = read_u32_be(body, 4) as usize;
    let mut total = 0u64;
    for i in 0..entry_count {
        let off = 8 + i * 8;
        if off + 8 > body.len() {
            break;
        }
        let count = read_u32_be(body, off) as u64;
        let delta = read_u32_be(body, off + 4) as u64;
        total += count * delta;
    }
    total
}

#[test]
fn parse_small_box_header() {
    let buf = [
        0x00, 0x00, 0x00, 0x08, // size = 8
        b'f', b't', b'y', b'p', // type = ftyp
    ];
    let header = BoxHeader::parse(&buf, 0).unwrap();
    assert_eq!(header.size, 8);
    assert_eq!(header.box_type, types::FTYP);
    assert_eq!(header.header_size, 8);
}

#[test]
fn parse_extended_size_box() {
    let mut buf = [0u8; 16];
    buf[0..4].copy_from_slice(&[0x00, 0x00, 0x00, 0x01]); // size = 1
    buf[4..8].copy_from_slice(b"mdat");
    buf[8..16].copy_from_slice(&16u64.to_be_bytes()); // ext size = 16
    let header = BoxHeader::parse(&buf, 0).unwrap();
    assert_eq!(header.size, 16);
    assert_eq!(header.box_type, types::MDAT);
    assert_eq!(header.header_size, 16);
}

#[test]
fn iter_boxes_with_nested_children() {
    // A moov box with a free sub-box.
    let mut buf = Vec::new();
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x10]); // size = 16
    buf.extend_from_slice(b"moov");
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x08]); // inner size
    buf.extend_from_slice(b"free");
    let items: Vec<_> = iter_boxes(&buf[8..], 8, 4).unwrap().collect();
    assert_eq!(items.len(), 1);
    let (h, body) = items.into_iter().next().unwrap().unwrap();
    assert_eq!(h.box_type, types::FREE);
    assert!(body.is_empty());
}

#[test]
fn audio_mux_demux_roundtrip() {
    let mut muxer = FragmentedMp4Muxer::new();
    muxer.configure(make_audio_config());

    let payloads: Vec<_> = (0..3).map(|i| vec![i as u8; 16]).collect();
    for (i, payload) in payloads.iter().cloned().enumerate() {
        muxer
            .push_packet(make_audio_packet(1, i as u64, i as i64 * 1024, payload))
            .unwrap();
    }

    let out = collect_packets_from_muxer_and_demux(&mut muxer);
    assert_eq!(out.len(), 3);
    for (i, pkt) in out.iter().enumerate() {
        assert_eq!(pkt.payload.as_ref(), &payloads[i]);
        assert!(pkt.flags.is_keyframe);
    }
}

#[test]
fn video_mux_demux_roundtrip_preserves_payloads_and_timestamps() {
    let mut muxer = FragmentedMp4Muxer::new();
    muxer.configure(make_video_config());

    let payloads = [vec![1u8; 10], vec![2u8; 10], vec![3u8; 10]];
    muxer
        .push_packet(make_video_packet(1, 0, 0, 0, true, payloads[0].clone()))
        .unwrap();
    muxer
        .push_packet(make_video_packet(
            1,
            1,
            3000,
            3000,
            false,
            payloads[1].clone(),
        ))
        .unwrap();
    muxer
        .push_packet(make_video_packet(
            1,
            2,
            6000,
            6000,
            true,
            payloads[2].clone(),
        ))
        .unwrap();

    let out = collect_packets_from_muxer_and_demux(&mut muxer);
    assert_eq!(out.len(), 3);
    for (i, pkt) in out.iter().enumerate() {
        assert_eq!(pkt.payload.as_ref(), &payloads[i]);
    }
    assert!(out[0].flags.is_keyframe);
    assert!(!out[1].flags.is_keyframe);
    assert!(out[2].flags.is_keyframe);
}

#[test]
fn demuxer_rejects_zero_size_box() {
    let data = [0x00, 0x00, 0x00, 0x00, b'f', b't', b'y', b'p'];
    let mut demuxer = IsobmffDemuxer::new();
    demuxer.push(&data);
    assert!(demuxer.next_event().is_err());
}

#[test]
fn negative_composition_offset_roundtrip() {
    let mut muxer = FragmentedMp4Muxer::new();
    muxer.configure(make_video_config());

    // pts = dts - 1000 ticks (B-frame-like negative CTS).
    let mut pkt = make_video_packet(1, 0, 0, 0, true, vec![1u8; 10]);
    pkt.flags.is_keyframe = true;
    muxer.push_packet(pkt).unwrap();

    let mut pkt2 = make_video_packet(1, 1, 3000, 2000, false, vec![2u8; 10]);
    pkt2.flags.is_keyframe = false;
    muxer.push_packet(pkt2).unwrap();

    // Need a closing keyframe so the segment is flushed.
    let mut pkt3 = make_video_packet(1, 2, 6000, 6000, true, vec![3u8; 10]);
    pkt3.flags.is_keyframe = true;
    muxer.push_packet(pkt3).unwrap();

    let out = collect_packets_from_muxer_and_demux(&mut muxer);
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].time.pts.map(|t| t.ticks()), Some(0));
    assert_eq!(out[0].time.dts.map(|t| t.ticks()), Some(0));
    assert_eq!(out[1].time.pts.map(|t| t.ticks()), Some(2000));
    assert_eq!(out[1].time.dts.map(|t| t.ticks()), Some(3000));
    assert!(out[1].time.pts.map(|t| t.ticks()) < out[1].time.dts.map(|t| t.ticks()));
    assert_eq!(out[2].time.pts.map(|t| t.ticks()), Some(6000));
    assert_eq!(out[2].time.dts.map(|t| t.ticks()), Some(6000));
}

#[test]
fn multi_track_av_sync_roundtrip() {
    let mut muxer = FragmentedMp4Muxer::new();
    let mut audio_cfg = make_audio_config();
    let mut video_cfg = make_video_config();
    audio_cfg.track_id = 1;
    video_cfg.track_id = 2;
    muxer.configure(audio_cfg);
    muxer.configure(video_cfg);

    let audio_payloads: Vec<_> = (0..3).map(|i| vec![0xa0 + i; 16]).collect();
    let video_payloads: Vec<_> = (0..3).map(|i| vec![0x10 + i; 10]).collect();

    for (i, payload) in audio_payloads.iter().cloned().enumerate() {
        let a = make_audio_packet(1, i as u64, i as i64 * 1024, payload);
        muxer.push_packet(a).unwrap();
    }
    for (i, payload) in video_payloads.iter().cloned().enumerate() {
        let v = make_video_packet(
            2,
            i as u64 + 10,
            i as i64 * 3000,
            i as i64 * 3000,
            true,
            payload,
        );
        muxer.push_packet(v).unwrap();
    }

    let out = collect_packets_from_muxer_and_demux(&mut muxer);
    let audio_out: Vec<_> = out.iter().filter(|p| p.track_id.get() == 1).collect();
    let video_out: Vec<_> = out.iter().filter(|p| p.track_id.get() == 2).collect();
    assert_eq!(audio_out.len(), 3);
    assert_eq!(video_out.len(), 3);
    for (i, pkt) in audio_out.iter().enumerate() {
        assert_eq!(pkt.payload.as_ref(), &audio_payloads[i]);
    }
    for (i, pkt) in video_out.iter().enumerate() {
        assert_eq!(pkt.payload.as_ref(), &video_payloads[i]);
        assert!(pkt.flags.is_keyframe);
    }
}

#[test]
fn config_change_emits_new_init_segment() {
    let mut muxer = FragmentedMp4Muxer::new();
    muxer.configure(make_audio_config());
    muxer
        .push_packet(make_audio_packet(1, 0, 0, vec![0u8; 16]))
        .unwrap();

    let first = muxer.flush_segment().unwrap().unwrap();
    assert!(first.init_segment.is_some());

    // Same config: no new init segment.
    muxer
        .push_packet(make_audio_packet(1, 1, 1024, vec![1u8; 16]))
        .unwrap();
    let second = muxer.flush_segment().unwrap().unwrap();
    assert!(second.init_segment.is_none());

    // Change timescale -> new init segment.
    let mut new_cfg = make_audio_config();
    new_cfg.timescale = 48000;
    muxer.configure(new_cfg);
    muxer
        .push_packet(make_audio_packet(1, 2, 1024, vec![2u8; 16]))
        .unwrap();
    let third = muxer.flush_segment().unwrap().unwrap();
    assert!(third.init_segment.is_some());
}

#[test]
fn fuzz_random_bytes_no_panic() {
    // Deterministic pseudo-random bytes.
    let mut state = 0x12345678u32;
    let mut buf = Vec::with_capacity(4096);
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        buf.push((state & 0xff) as u8);
    }

    let mut demuxer = IsobmffDemuxer::new();
    demuxer.push(&buf);
    for _ in 0..100 {
        match demuxer.next_event() {
            Ok(None) | Err(_) => break,
            Ok(Some(_)) => {}
        }
    }
}

#[test]
fn malicious_size_offset_count_returns_error() {
    // A moof box with a traf/trun claiming 0xffff samples but providing no data.
    let mut buf = Vec::new();
    // moof size = 48 (header 8 + body 40)
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x30]);
    buf.extend_from_slice(b"moof");
    // mfhd size = 16
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x10]);
    buf.extend_from_slice(b"mfhd");
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // version/flags
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // sequence
    // traf size = 24 (header 8 + body 16)
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x18]);
    buf.extend_from_slice(b"traf");
    // trun size = 20 (header 8 + body 12): fullbox 4 + sample_count 4 + data_offset 4
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x14]);
    buf.extend_from_slice(b"trun");
    buf.extend_from_slice(&[0x01, 0x00, 0x0f, 0x01]); // version 1, all flags
    buf.extend_from_slice(&0x0000_ffffu32.to_be_bytes()); // sample_count huge
    buf.extend_from_slice(&0u32.to_be_bytes()); // data_offset

    let mut demuxer = IsobmffDemuxer::new();
    demuxer.push(&buf);
    assert!(demuxer.next_event().is_err());
}

#[test]
fn progressive_mp4_audio_roundtrip() {
    let mut muxer = ProgressiveMp4Muxer::new();
    muxer.configure(make_audio_config());

    let payloads: Vec<_> = (0..3).map(|i| vec![i as u8; 16]).collect();
    for (i, payload) in payloads.iter().cloned().enumerate() {
        muxer
            .push_packet(make_audio_packet(1, i as u64, i as i64 * 1024, payload))
            .unwrap();
    }

    let mp4 = muxer.finish().unwrap();
    let (ftyp, _) = find_box(&mp4, types::FTYP).unwrap();
    assert_eq!(ftyp.box_type, types::FTYP);

    let (_moov_header, moov_body) = find_box(&mp4, types::MOOV).unwrap();
    let (mdat_header, mdat_body) = find_box(&mp4, types::MDAT).unwrap();

    // moov -> trak -> mdia -> minf -> stbl
    let (_, trak_body) = find_box(moov_body, types::TRAK).unwrap();
    let (_, mdia_body) = find_box(trak_body, types::MDIA).unwrap();
    let (_, minf_body) = find_box(mdia_body, types::MINF).unwrap();
    let (_, stbl_body) = find_box(minf_body, types::STBL).unwrap();

    let (_, stsz_body) = find_box(stbl_body, types::STSZ).unwrap();
    let (_, stco_body) = find_box(stbl_body, types::STCO).unwrap();

    let sizes = parse_stsz(stsz_body);
    let offsets = parse_stco(stco_body);

    assert_eq!(sizes.len(), 3);
    assert_eq!(sizes, vec![16u32; 3]);
    assert_eq!(offsets.len(), 1);

    let mdat_data_offset = mdat_header.body_offset();
    let chunk_start = offsets[0];
    assert_eq!(chunk_start, mdat_data_offset);

    let mut pos = (chunk_start - mdat_data_offset) as usize;
    for (i, expected) in payloads.iter().enumerate() {
        let end = pos + sizes[i] as usize;
        assert_eq!(&mdat_body[pos..end], expected.as_slice());
        pos = end;
    }
    assert_eq!(pos, mdat_body.len());

    // stts total duration
    let (_, stts_body) = find_box(stbl_body, types::STTS).unwrap();
    assert_eq!(total_duration_from_stts(stts_body), 3 * 1024);
}

#[test]
fn progressive_mp4_video_roundtrip_with_b_frames() {
    let mut muxer = ProgressiveMp4Muxer::new();
    muxer.configure(make_video_config());

    let payloads = [vec![1u8; 10], vec![2u8; 10], vec![3u8; 10]];
    muxer
        .push_packet(make_video_packet(1, 0, 0, 0, true, payloads[0].clone()))
        .unwrap();
    muxer
        .push_packet(make_video_packet(
            1,
            1,
            3000,
            2000,
            false,
            payloads[1].clone(),
        ))
        .unwrap();
    muxer
        .push_packet(make_video_packet(
            1,
            2,
            6000,
            6000,
            true,
            payloads[2].clone(),
        ))
        .unwrap();

    let mp4 = muxer.finish().unwrap();

    let (_moov_header, moov_body) = find_box(&mp4, types::MOOV).unwrap();
    let (mdat_header, mdat_body) = find_box(&mp4, types::MDAT).unwrap();

    let (_, trak_body) = find_box(moov_body, types::TRAK).unwrap();
    let (_, mdia_body) = find_box(trak_body, types::MDIA).unwrap();
    let (_, minf_body) = find_box(mdia_body, types::MINF).unwrap();
    let (_, stbl_body) = find_box(minf_body, types::STBL).unwrap();

    let (_, stsz_body) = find_box(stbl_body, types::STSZ).unwrap();
    let (_, stco_body) = find_box(stbl_body, types::STCO).unwrap();
    let (_, stss_body) = find_box(stbl_body, types::STSS).unwrap();
    let (_, ctts_body) = find_box(stbl_body, types::CTTS).unwrap();

    let sizes = parse_stsz(stsz_body);
    let offsets = parse_stco(stco_body);
    let syncs = parse_stss(stss_body);

    assert_eq!(sizes, vec![10u32; 3]);
    assert_eq!(offsets[0], mdat_header.body_offset());

    let mut pos = (offsets[0] - mdat_header.body_offset()) as usize;
    for (i, expected) in payloads.iter().enumerate() {
        let end = pos + sizes[i] as usize;
        assert_eq!(&mdat_body[pos..end], expected.as_slice());
        pos = end;
    }

    assert_eq!(syncs, vec![1, 3]);

    // ctts version 1, one entry for the B-frame negative offset.
    let (_, _, ctts_body_after) = read_fullbox_header(ctts_body).unwrap();
    let ctts_entry_count = read_u32_be(ctts_body_after, 0);
    assert!(ctts_entry_count >= 1);
    let mut ctts_off = 4usize;
    let mut total_ctts_count = 0u32;
    for _ in 0..ctts_entry_count {
        let count = read_u32_be(ctts_body_after, ctts_off);
        let offset = i32::from_be_bytes([
            ctts_body_after[ctts_off + 4],
            ctts_body_after[ctts_off + 5],
            ctts_body_after[ctts_off + 6],
            ctts_body_after[ctts_off + 7],
        ]);
        total_ctts_count += count;
        if total_ctts_count == 2 {
            assert_eq!(offset, -1000);
            break;
        }
        ctts_off += 8;
    }

    let (_, stts_body) = find_box(stbl_body, types::STTS).unwrap();
    assert_eq!(total_duration_from_stts(stts_body), 3 * 3000);
}

use proptest::prelude::*;

proptest! {
    #[test]
    fn isobmff_demuxer_arbitrary_bytes_do_not_panic(bytes in prop::collection::vec(0u8..=255, 0..2048)) {
        let mut demuxer = IsobmffDemuxer::new();
        demuxer.push(&bytes);
        for _ in 0..64 {
            match demuxer.next_event() {
                Ok(None) => break,
                Err(_) => break,
                _ => {}
            }
        }
        let _ = demuxer.next_event();
    }
}

fn make_video_track_data() -> TrackData {
    TrackData {
        track: TrackInfo::new(
            TrackId::new(1).unwrap(),
            TrackKind::Video,
            CodecId::H264,
            TimeBase::new(1, 30_000).unwrap(),
        ),
        timescale: 30_000,
        track_type: types::AVC1,
        trex: TrexDefaults::default(),
    }
}

#[test]
fn emit_packets_rejects_sample_offset_before_mdat() {
    let track = make_video_track_data();
    let tf = TrackFragment {
        track_id: 1,
        base_decode_time: 0,
        default_sample_duration: 3000,
        default_sample_size: 0,
        default_sample_flags: 0,
        first_sample_flags: None,
        samples: vec![FragmentSample {
            duration: 3000,
            size: 2,
            flags: 0,
            composition_offset: 0,
            data_offset: 5,
        }],
        data_offset_base: 0,
        moof_offset: 0,
    };
    let mdat = cheetah_media_types::BufferRef::from_owned(vec![0xab, 0xcd]);
    let mut seq = 0;
    let res = emit_packets(&tf, &mdat, 10, &track, &mut seq, StreamEpoch::new(0));
    assert!(res.is_err());
}

#[test]
fn emit_packets_rejects_sample_size_overflow() {
    let track = make_video_track_data();
    let tf = TrackFragment {
        track_id: 1,
        base_decode_time: 0,
        default_sample_duration: 3000,
        default_sample_size: 0,
        default_sample_flags: 0,
        first_sample_flags: None,
        samples: vec![FragmentSample {
            duration: 3000,
            size: u64::MAX,
            flags: 0,
            composition_offset: 0,
            data_offset: 10,
        }],
        data_offset_base: 0,
        moof_offset: 0,
    };
    let mdat = cheetah_media_types::BufferRef::from_owned(vec![0xab, 0xcd]);
    let mut seq = 0;
    let res = emit_packets(&tf, &mdat, 10, &track, &mut seq, StreamEpoch::new(0));
    assert!(res.is_err());
}
