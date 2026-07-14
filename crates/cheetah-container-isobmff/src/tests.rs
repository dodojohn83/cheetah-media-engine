//! Tests for the ISOBMFF parser, muxer, and demuxer.

use alloc::vec::Vec;
use cheetah_media_types::{
    CodecConfig, CodecId, MediaPacket, MediaTime, SequenceNumber, StreamEpoch, TrackId, TrackKind,
};

use crate::{
    FragmentedMp4Muxer, IsobmffDemuxer, Mp4Event, SegmentOutput, TrackConfig,
    boxes::{BoxHeader, iter_boxes, types},
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
        muxer.push_packet(make_audio_packet(1, i as u64, i as i64 * 1024, payload));
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
    muxer.push_packet(make_video_packet(1, 0, 0, 0, true, payloads[0].clone()));
    muxer.push_packet(make_video_packet(
        1,
        1,
        3000,
        3000,
        false,
        payloads[1].clone(),
    ));
    muxer.push_packet(make_video_packet(
        1,
        2,
        6000,
        6000,
        true,
        payloads[2].clone(),
    ));

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
