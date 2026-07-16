//! MPEG-PS demuxer tests.

use cheetah_media_types::{CodecId, TrackKind};

use crate::{MpegPsConfig, MpegPsDemuxer, MpegPsError, MpegPsEvent};

fn make_video_pes(payload: &[u8]) -> Vec<u8> {
    let mut pes = Vec::new();
    pes.extend_from_slice(&[0x00, 0x00, 0x01, 0xE0]);

    // PES optional header: PTS present, header_data_length = 5.
    let header_and_payload_len = 3 + 5 + payload.len();
    pes.push((header_and_payload_len >> 8) as u8);
    pes.push((header_and_payload_len & 0xFF) as u8);

    pes.push(0x81); // marker bits + PES_scrambling_control + priority + alignment + copyright + original
    pes.push(0x80); // PTS present
    pes.push(0x05); // header_data_length
    // 5-byte PTS = 0
    pes.extend_from_slice(&[0x21, 0x00, 0x01, 0x00, 0x01]);
    pes.extend_from_slice(payload);
    pes
}

fn collect_events(demuxer: &mut MpegPsDemuxer) -> Vec<MpegPsEvent> {
    let mut out = Vec::new();
    loop {
        match demuxer.next_event() {
            Ok(Some(MpegPsEvent::Eof)) => break,
            Ok(Some(e)) => out.push(e),
            Ok(None) => break,
            Err(MpegPsError::NeedMoreData) => break,
            Err(e) => {
                out.push(MpegPsEvent::Eof);
                panic!("demuxer error: {:?}", e);
            }
        }
    }
    out
}

#[test]
fn demuxer_parses_h264_ps_fixture() {
    let data = include_bytes!("../tests/fixtures/h264_baseline.ps");
    let mut demuxer = MpegPsDemuxer::new(MpegPsConfig::h264());
    demuxer.push(data);
    demuxer.end().unwrap();

    let mut tracks = Vec::new();
    let mut packets = 0;
    for event in collect_events(&mut demuxer) {
        match event {
            MpegPsEvent::Track(t) => tracks.push(t),
            MpegPsEvent::Packet(_) => packets += 1,
            _ => {}
        }
    }

    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].kind, TrackKind::Video);
    assert_eq!(tracks[0].codec, CodecId::H264);
    assert!(matches!(
        tracks[0].codec_config,
        cheetah_media_types::CodecConfig::AvcC(_)
    ));
    assert!(
        packets >= 2,
        "expected at least SPS and IDR packets, got {}",
        packets
    );
}

#[test]
fn pack_header_with_stuffing_is_skipped() {
    let mut data = Vec::new();
    data.extend_from_slice(&[0x00, 0x00, 0x01, 0xBA]); // pack start
    // 10 fixed bytes + 3 stuffing bytes: last byte lower 3 bits = 3, then stuffing.
    data.extend_from_slice(&[0x44, 0x00, 0x04, 0x00, 0x04, 0x01, 0x86, 0x66, 0xCF, 0xF3]);
    data.extend_from_slice(&[0xFF, 0xFF, 0xFF]); // 3 stuffing bytes
    data.extend_from_slice(&make_video_pes(&[0x00, 0x00, 0x00, 0x01, 0x09])); // access unit delimiter

    let mut demuxer = MpegPsDemuxer::new(MpegPsConfig::h264());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let packets: Vec<_> = collect_events(&mut demuxer)
        .into_iter()
        .filter_map(|e| match e {
            MpegPsEvent::Packet(p) => Some(p),
            _ => None,
        })
        .collect();

    assert!(!packets.is_empty());
}

#[test]
fn system_header_is_skipped() {
    let mut data = Vec::new();
    data.extend_from_slice(&[0x00, 0x00, 0x01, 0xBB, 0x00, 0x09]); // system header, length 9
    data.extend_from_slice(&[0xC3, 0x33, 0x67, 0x00, 0x21, 0xFF, 0xE2, 0xE0, 0xE6]);
    data.extend_from_slice(&make_video_pes(&[0x00, 0x00, 0x00, 0x01, 0x09]));

    let mut demuxer = MpegPsDemuxer::new(MpegPsConfig::h264());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let packets: Vec<_> = collect_events(&mut demuxer)
        .into_iter()
        .filter_map(|e| match e {
            MpegPsEvent::Packet(p) => Some(p),
            _ => None,
        })
        .collect();
    assert!(!packets.is_empty());
}

#[test]
fn empty_input_returns_need_more() {
    let mut demuxer = MpegPsDemuxer::new(MpegPsConfig::h264());
    assert_eq!(demuxer.next_event().unwrap(), None);
}

#[test]
fn malformed_start_code_is_recovered() {
    // Leading garbage followed by a pack start code and a tiny video PES.
    let mut data = Vec::new();
    data.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    data.extend_from_slice(&[
        0x00, 0x00, 0x01, 0xBA, 0x44, 0x00, 0x04, 0x00, 0x04, 0x01, 0x86, 0x66, 0xCF, 0xF8,
    ]);
    data.extend_from_slice(&make_video_pes(&[0x00, 0x00, 0x00, 0x01, 0x09]));

    let mut demuxer = MpegPsDemuxer::new(MpegPsConfig::h264());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let packets: Vec<_> = collect_events(&mut demuxer)
        .into_iter()
        .filter_map(|e| match e {
            MpegPsEvent::Packet(p) => Some(p),
            _ => None,
        })
        .collect();
    assert!(!packets.is_empty());
}

#[test]
fn program_end_code_emits_eof() {
    let data = [0x00, 0x00, 0x01, 0xB9];
    let mut demuxer = MpegPsDemuxer::new(MpegPsConfig::h264());
    demuxer.push(&data);
    let event = demuxer.next_event().unwrap();
    assert_eq!(event, Some(MpegPsEvent::Eof));
}

#[test]
fn partial_packet_waits_for_more_data() {
    // Pack header + system header truncated before PES length is complete.
    let data = [
        0x00, 0x00, 0x01, 0xBA, 0x44, 0x00, 0x04, 0x00, 0x04, 0x01, 0x86, 0x66, 0xCF, 0xF8,
    ];
    let mut demuxer = MpegPsDemuxer::new(MpegPsConfig::h264());
    demuxer.push(&data);
    assert_eq!(demuxer.next_event().unwrap(), None);
}

#[test]
fn audio_aac_pes_emits_track_and_packets() {
    // Build an ADTS AAC frame.
    let aac_frame = build_adts_frame(44100, 2, 7);
    let aac_payload = aac_frame.clone();

    // PES header for private stream 1 (0xBD) with PTS only.
    let mut pes = Vec::new();
    pes.extend_from_slice(&[0x00, 0x00, 0x01, 0xBD]);
    pes.extend_from_slice(&[0x00, 0x00]); // placeholder
    pes.push(0x80); // marker + flags
    pes.push(0x80); // PTS present
    pes.push(0x05); // header data length
    // 5-byte PTS = 0
    pes.push(0x21);
    pes.push(0x00);
    pes.push(0x01);
    pes.push(0x00);
    pes.push(0x01);
    pes.extend_from_slice(&aac_payload);

    let payload_len = pes.len() - 6;
    pes[4] = (payload_len >> 8) as u8;
    pes[5] = (payload_len & 0xFF) as u8;

    let mut demuxer = MpegPsDemuxer::new(MpegPsConfig::h264());
    demuxer.push(&pes);
    demuxer.end().unwrap();

    let mut tracks = 0;
    let mut packets = 0;
    for event in collect_events(&mut demuxer) {
        match event {
            MpegPsEvent::Track(_) => tracks += 1,
            MpegPsEvent::Packet(_) => packets += 1,
            _ => {}
        }
    }
    assert_eq!(tracks, 1);
    assert_eq!(packets, 1);
}

#[test]
fn audio_aac_multiple_frames_increase_timestamps() {
    let frame_a = build_adts_frame(44100, 2, 7);
    let frame_b = build_adts_frame(44100, 2, 7);
    let payload = [frame_a.as_slice(), frame_b.as_slice()].concat();

    let mut pes = Vec::new();
    pes.extend_from_slice(&[0x00, 0x00, 0x01, 0xBD]);
    let payload_len = 3 + 5 + payload.len();
    pes.push((payload_len >> 8) as u8);
    pes.push((payload_len & 0xFF) as u8);
    pes.push(0x80);
    pes.push(0x80);
    pes.push(0x05);
    pes.extend_from_slice(&[0x21, 0x00, 0x01, 0x00, 0x01]);
    pes.extend_from_slice(&payload);

    let mut demuxer = MpegPsDemuxer::new(MpegPsConfig::h264());
    demuxer.push(&pes);
    demuxer.end().unwrap();

    let packets: Vec<_> = collect_events(&mut demuxer)
        .into_iter()
        .filter_map(|e| match e {
            MpegPsEvent::Packet(p) => Some(p),
            _ => None,
        })
        .collect();

    assert_eq!(packets.len(), 2);
    let duration = 1024i64 * 90_000 / 44100;
    assert_eq!(packets[0].time.pts.map(|t| t.ticks()), Some(0));
    assert_eq!(packets[1].time.pts.map(|t| t.ticks()), Some(duration));
}

#[test]
fn unbounded_video_pes_flushes_at_eof() {
    // Unbounded video PES (packet_length == 0) with no trailing boundary.
    let mut data = Vec::new();
    data.extend_from_slice(&[0x00, 0x00, 0x01, 0xBA]); // pack start
    data.extend_from_slice(&[0x44, 0x00, 0x04, 0x00, 0x04, 0x01, 0x86, 0x66, 0xCF, 0xF8]);
    data.extend_from_slice(&[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00]); // unbounded video PES
    data.push(0x81);
    data.push(0x80);
    data.push(0x05);
    data.extend_from_slice(&[0x21, 0x00, 0x01, 0x00, 0x01]); // PTS = 0
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x09]); // access unit delimiter

    let mut demuxer = MpegPsDemuxer::new(MpegPsConfig::h264());
    demuxer.push(&data);
    demuxer.end().unwrap();

    let packets: Vec<_> = collect_events(&mut demuxer)
        .into_iter()
        .filter_map(|e| match e {
            MpegPsEvent::Packet(p) => Some(p),
            _ => None,
        })
        .collect();

    assert!(
        !packets.is_empty(),
        "unbounded video PES should flush at EOF"
    );
}

fn build_adts_frame(sample_rate: u32, channels: u8, raw_len: usize) -> Vec<u8> {
    use cheetah_media_bitstream::aac::AdtsHeader;

    let sampling_frequency_index = match sample_rate {
        96000 => 0,
        88200 => 1,
        64000 => 2,
        48000 => 3,
        44100 => 4,
        32000 => 5,
        24000 => 6,
        22050 => 7,
        16000 => 8,
        12000 => 9,
        11025 => 10,
        8000 => 11,
        7350 => 12,
        _ => 4,
    };

    let frame_length = (7 + raw_len) as u16;
    let header = AdtsHeader {
        id: 0,
        layer: 0,
        protection_absent: true,
        profile: 1,
        sampling_frequency_index,
        sampling_frequency: sample_rate,
        private_bit: 0,
        channel_configuration: channels,
        channel_count: channels,
        frame_length,
        buffer_fullness: 0,
        number_of_raw_data_blocks_in_frame: 0,
        crc_present: false,
        samples_per_frame: 1024,
        duration_ms: 1024u32 * 1000 / sample_rate,
    };
    let mut frame = header.build();
    frame.extend_from_slice(&vec![0; raw_len]);
    frame
}
