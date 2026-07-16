use cheetah_media_bitstream::h264::H264CodecConfig;
use cheetah_media_types::{CodecConfig, CodecId};

use crate::demuxer::{find_start_code, nal_payload};
use crate::tests::{collect_events, default_config, idr, make_annexb, non_idr, pps, sps};
use crate::{AnnexBDemuxer, AnnexbEvent};

#[test]
fn demuxer_emits_track_and_packets() {
    let data = make_annexb(&[sps(), pps(), idr(), non_idr()]);
    let mut demuxer = AnnexBDemuxer::new(default_config());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let mut events = collect_events(&mut demuxer);
    // Eof is not asserted because it is consumed by collect_events.
    assert!(!events.is_empty());

    // The Track event carries the AvcC config once SPS+PPS are known.
    let track = match events.remove(0) {
        AnnexbEvent::Track(t) => t,
        other => panic!("expected Track, got {:?}", other),
    };
    assert_eq!(track.codec, CodecId::H264);
    assert!(matches!(track.codec_config, CodecConfig::AvcC(_)));
    assert!(track.video_format.is_some());

    // Parameter set NALs are not duplicated as packets; the next events are slices.
    let mut idr_seen = false;
    let mut non_idr_seen = false;
    for event in events {
        if let AnnexbEvent::Packet(p) = event {
            if p.flags.is_keyframe {
                assert_eq!(p.payload.as_ref(), idr().as_slice());
                idr_seen = true;
            } else if p.payload.as_ref() == non_idr().as_slice() {
                non_idr_seen = true;
            }
        }
    }
    assert!(idr_seen);
    assert!(non_idr_seen);
}

#[test]
fn avcc_config_is_valid() {
    let data = make_annexb(&[sps(), pps(), idr()]);
    let mut demuxer = AnnexBDemuxer::new(default_config());
    demuxer.push(&data);

    let track = match demuxer.next_event().unwrap().unwrap() {
        AnnexbEvent::Track(t) => t,
        _ => panic!("expected Track"),
    };

    if let CodecConfig::AvcC(bytes) = &track.codec_config {
        let cfg = H264CodecConfig::parse(bytes).unwrap();
        assert_eq!(cfg.sps_list.len(), 1);
        assert_eq!(cfg.pps_list.len(), 1);
        assert!(cfg.width > 0);
        assert!(cfg.height > 0);
        assert!(!cfg.codec_string.is_empty());
    } else {
        panic!("expected AvcC config");
    }
}

#[test]
fn avcc_config_preserves_emulation_prevention_bytes() {
    // Append an EPB sequence to the SPS NAL. The AvcC record must store the
    // raw NAL bytes (with EPB intact) so downstream decoders can unescape
    // them exactly once. The protected value 0x02 is chosen so the wire
    // sequence does not form a start code.
    let mut sps_with_epb = sps();
    sps_with_epb.extend_from_slice(&[0x00, 0x00, 0x03, 0x02]);

    let data = make_annexb(&[sps_with_epb.clone(), pps(), idr()]);
    let mut demuxer = AnnexBDemuxer::new(default_config());
    demuxer.push(&data);

    let track = match demuxer.next_event().unwrap().unwrap() {
        AnnexbEvent::Track(t) => t,
        _ => panic!("expected Track"),
    };

    if let CodecConfig::AvcC(bytes) = &track.codec_config {
        let cfg = H264CodecConfig::parse(bytes).unwrap();
        assert_eq!(cfg.sps_list[0], sps_with_epb);
        assert!(cfg.width > 0);
        assert!(cfg.height > 0);
    } else {
        panic!("expected AvcC config");
    }
}

#[test]
fn packet_sequence_and_timestamp_match() {
    let data = make_annexb(&[sps(), pps(), idr()]);
    let mut demuxer = AnnexBDemuxer::new(default_config());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let packets: Vec<_> = collect_events(&mut demuxer)
        .into_iter()
        .filter_map(|e| match e {
            AnnexbEvent::Packet(p) => Some(p),
            _ => None,
        })
        .collect();

    for (i, packet) in packets.iter().enumerate() {
        assert_eq!(packet.sequence.get(), i as u64);
        assert_eq!(
            packet.time.pts.map(|t| t.ticks()),
            Some(i as i64),
            "pts mismatch at sequence {}",
            i
        );
        assert_eq!(
            packet.time.dts.map(|t| t.ticks()),
            Some(i as i64),
            "dts mismatch at sequence {}",
            i
        );
    }
}

#[test]
fn split_push_works() {
    let data = make_annexb(&[sps(), pps(), idr()]);
    let mut demuxer = AnnexBDemuxer::new(default_config());

    for chunk in data.chunks(3) {
        demuxer.push(chunk);
        let _ = collect_events(&mut demuxer);
    }

    demuxer.end().unwrap();
    let events = collect_events(&mut demuxer);
    assert!(events.iter().any(|e| matches!(e, AnnexbEvent::Eof)));
}

#[test]
fn empty_push_returns_need_more() {
    let mut demuxer = AnnexBDemuxer::new(default_config());
    demuxer.push(&[]);
    assert!(demuxer.next_event().unwrap().is_none());
}

#[test]
fn need_more_until_next_start_code() {
    let mut data = make_annexb(&[sps(), pps()]);
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
    data.extend_from_slice(&idr()[..2]); // incomplete IDR

    let mut demuxer = AnnexBDemuxer::new(default_config());
    demuxer.push(&data);

    // SPS+PPS produce a Track; the incomplete IDR needs more bytes.
    let events = collect_events(&mut demuxer);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], AnnexbEvent::Track(_)));

    // Finish the IDR.
    demuxer.push(&idr()[2..]);
    demuxer.end().unwrap();

    let idr_packet = collect_events(&mut demuxer)
        .into_iter()
        .filter_map(|e| match e {
            AnnexbEvent::Packet(p) => Some(p),
            _ => None,
        })
        .find(|p| p.flags.is_keyframe)
        .expect("expected an IDR packet");
    assert_eq!(idr_packet.payload.as_ref(), idr().as_slice());
}

#[test]
fn malformed_input_is_rejected() {
    let mut demuxer = AnnexBDemuxer::new(default_config());
    let mut data = vec![0x00, 0x00, 0x00, 0x01, 0x67];
    data.extend_from_slice(&vec![0x00; 5000]);
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x68]);
    demuxer.push(&data);
    assert!(demuxer.next_event().is_err());
}

#[test]
fn unsupported_codec_errors() {
    let mut cfg = default_config();
    cfg.codec = CodecId::Aac;
    let mut demuxer = AnnexBDemuxer::new(cfg);
    demuxer.push(&[0x00, 0x00, 0x00, 0x01, 0x40, 0x01]);
    assert!(matches!(
        demuxer.next_event(),
        Err(crate::AnnexbError::UnsupportedCodec)
    ));
}

#[test]
fn reset_clears_state() {
    let data = make_annexb(&[sps(), pps(), idr()]);
    let mut demuxer = AnnexBDemuxer::new(default_config());
    demuxer.push(&data);
    demuxer.reset();
    assert_eq!(demuxer.buffer_len(), 0);
    assert!(demuxer.next_event().unwrap().is_none());
}

#[test]
fn end_emits_eof_and_final_packet() {
    let data = make_annexb(&[sps(), pps(), idr()]);
    let mut demuxer = AnnexBDemuxer::new(default_config());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let events = collect_events(&mut demuxer);
    let mut packet_count = 0;
    let mut eof = false;
    for event in &events {
        match event {
            AnnexbEvent::Packet(_) => packet_count += 1,
            AnnexbEvent::Eof => eof = true,
            _ => {}
        }
    }
    assert_eq!(packet_count, 1); // only the IDR slice is emitted as a packet
    assert!(eof);
}

#[test]
fn parameter_set_change_re_emits_track() {
    let sps1 = sps();
    let pps1 = pps();
    let mut pps2 = pps();
    pps2[2] = 0xff; // different PPS

    let data = make_annexb(&[sps1.clone(), pps1.clone(), sps1.clone(), pps2.clone()]);
    let mut demuxer = AnnexBDemuxer::new(default_config());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let track_count = collect_events(&mut demuxer)
        .into_iter()
        .filter(|e| matches!(e, AnnexbEvent::Track(_)))
        .count();
    // First track after initial SPS+PPS, second track after PPS change.
    assert_eq!(track_count, 2);
}

#[test]
fn emulation_prevention_not_treated_as_start_code() {
    // Build a NAL that contains 00 00 03 01 inside its payload.
    let mut nal_with_epb = idr();
    nal_with_epb.extend_from_slice(&[0x00, 0x00, 0x03, 0x01, 0x00, 0x00, 0x03, 0x00]);
    let data = make_annexb(&[sps(), pps(), nal_with_epb.clone()]);

    let mut demuxer = AnnexBDemuxer::new(default_config());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let found_packet = collect_events(&mut demuxer)
        .into_iter()
        .filter_map(|e| match e {
            AnnexbEvent::Packet(p) => Some(p),
            _ => None,
        })
        .any(|p| p.flags.is_keyframe && p.payload.as_ref() == nal_with_epb.as_slice());
    assert!(found_packet);
}

#[test]
fn emulation_prevention_preserved_across_nal_boundary() {
    // First NAL ends with an EPB sequence (00 00 03 00) and is immediately
    // followed by a 3-byte start code. The protected 0x00 must remain in the
    // first NAL payload and must not be consumed as part of the next start code.
    let mut nal1 = idr();
    nal1.extend_from_slice(&[0x00, 0x00, 0x03, 0x00]);

    let mut data = Vec::new();
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
    data.extend_from_slice(&nal1);
    data.extend_from_slice(&[0x00, 0x00, 0x01]);
    data.extend_from_slice(&non_idr());

    let mut demuxer = AnnexBDemuxer::new(default_config());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let packets: Vec<_> = collect_events(&mut demuxer)
        .into_iter()
        .filter_map(|e| match e {
            AnnexbEvent::Packet(p) => Some(p),
            _ => None,
        })
        .collect();

    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0].payload.as_ref(), nal1.as_slice());
    assert_eq!(packets[1].payload.as_ref(), non_idr().as_slice());
}

#[test]
fn nal_payload_helper_skips_header() {
    assert_eq!(nal_payload(&[0x67, 0x42, 0x00]), &[0x42, 0x00]);
}

#[test]
fn find_start_code_returns_none_for_short_input() {
    assert_eq!(find_start_code(&[0x00, 0x00], 0), None);
    assert_eq!(find_start_code(&[0x00, 0x00, 0x00], 0), None);
}
