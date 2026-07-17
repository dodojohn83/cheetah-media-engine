use cheetah_media_bitstream::h265::H265CodecConfig;
use cheetah_media_types::{CodecConfig, CodecId, MetadataItem, MetadataSource};

use crate::tests::{
    collect_events, h265_config, h265_idr, h265_non_idr, h265_pps, h265_sps, h265_vps, make_annexb,
};
use crate::{AnnexBDemuxer, AnnexbEvent};

#[test]
fn hevc_demuxer_emits_track_and_packets() {
    let data = make_annexb(&[
        h265_vps(),
        h265_sps(),
        h265_pps(),
        h265_idr(),
        h265_non_idr(),
    ]);
    let mut demuxer = AnnexBDemuxer::new(h265_config());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let mut events = collect_events(&mut demuxer);
    assert!(!events.is_empty());

    let track = match events.remove(0) {
        AnnexbEvent::Track(t) => t,
        other => panic!("expected Track, got {:?}", other),
    };
    assert_eq!(track.codec, CodecId::H265);
    if let CodecConfig::HevcC(bytes) = &track.codec_config {
        let cfg = H265CodecConfig::parse(bytes).unwrap();
        assert_eq!(cfg.sps_list.len(), 1);
        assert_eq!(cfg.pps_list.len(), 1);
        assert!(!cfg.codec_string.is_empty());
    } else {
        panic!("expected HevcC config");
    }

    let mut idr_seen = false;
    let mut non_idr_seen = false;
    for event in events {
        if let AnnexbEvent::Packet(p) = event {
            if p.flags.is_keyframe {
                assert_eq!(p.payload.as_ref(), h265_idr().as_slice());
                idr_seen = true;
            } else if p.payload.as_ref() == h265_non_idr().as_slice() {
                non_idr_seen = true;
            }
        }
    }
    assert!(idr_seen);
    assert!(non_idr_seen);
}

#[test]
fn hevc_demuxer_works_without_vps() {
    // VPS is optional for the demuxer; it falls back to SPS profile/tier/level.
    let data = make_annexb(&[h265_sps(), h265_pps(), h265_idr()]);
    let mut demuxer = AnnexBDemuxer::new(h265_config());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let track = collect_events(&mut demuxer)
        .into_iter()
        .find_map(|e| match e {
            AnnexbEvent::Track(t) => Some(t),
            _ => None,
        })
        .expect("expected Track");
    assert_eq!(track.codec, CodecId::H265);
    assert!(matches!(track.codec_config, CodecConfig::HevcC(_)));
}

#[test]
fn hevc_config_preserves_emulation_prevention_bytes() {
    let mut sps_with_epb = h265_sps();
    // Append an EPB sequence. The 0x00 protected value is chosen so the wire
    // sequence does not form a start code.
    sps_with_epb.extend_from_slice(&[0x00, 0x00, 0x03, 0x00]);

    let data = make_annexb(&[h265_vps(), sps_with_epb.clone(), h265_pps(), h265_idr()]);
    let mut demuxer = AnnexBDemuxer::new(h265_config());
    demuxer.push(&data);

    let track = collect_events(&mut demuxer)
        .into_iter()
        .find_map(|e| match e {
            AnnexbEvent::Track(t) => Some(t),
            _ => None,
        })
        .expect("expected Track");

    if let CodecConfig::HevcC(bytes) = &track.codec_config {
        let cfg = H265CodecConfig::parse(bytes).unwrap();
        assert_eq!(cfg.sps_list[0], sps_with_epb);
    } else {
        panic!("expected HevcC config");
    }
}

#[test]
fn hevc_parameter_set_change_re_emits_track() {
    let vps = h265_vps();
    let sps = h265_sps();
    let pps1 = h265_pps();
    let mut pps2 = h265_pps();
    pps2[3] = 0xff; // different PPS payload

    let data = make_annexb(&[vps.clone(), sps.clone(), pps1, vps, sps, pps2]);
    let mut demuxer = AnnexBDemuxer::new(h265_config());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let track_count = collect_events(&mut demuxer)
        .into_iter()
        .filter(|e| matches!(e, AnnexbEvent::Track(_)))
        .count();
    assert_eq!(track_count, 2);
}

#[test]
fn hevc_emulation_prevention_not_treated_as_start_code() {
    let mut nal_with_epb = h265_idr();
    nal_with_epb.extend_from_slice(&[0x00, 0x00, 0x03, 0x01, 0x00, 0x00, 0x03, 0x00]);
    let data = make_annexb(&[h265_vps(), h265_sps(), h265_pps(), nal_with_epb.clone()]);

    let mut demuxer = AnnexBDemuxer::new(h265_config());
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
fn demuxer_extracts_h265_sei_metadata() {
    // H.265 SEI NAL (type 39) with payload_type=4, size=5, "hello".
    // Two-byte NAL header: forbidden 0, nal_type=39, layer_id=0, temporal_id=1.
    let sei = vec![0x4e, 0x01, 0x04, 0x05, b'h', b'e', b'l', b'l', b'o'];
    let data = make_annexb(&[h265_vps(), h265_sps(), h265_pps(), sei, h265_idr()]);
    let mut demuxer = AnnexBDemuxer::new(h265_config());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let mut metadata: Vec<MetadataItem> = Vec::new();
    for event in collect_events(&mut demuxer) {
        if let AnnexbEvent::Metadata(items) = event {
            metadata.extend(items);
        }
    }

    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata[0].source, MetadataSource::Sei);
    assert_eq!(metadata[0].key, 4);
    assert_eq!(metadata[0].value, b"hello");
}
