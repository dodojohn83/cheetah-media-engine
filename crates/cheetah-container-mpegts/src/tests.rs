//! Unit tests for the MPEG-TS demuxer.

use alloc::vec::Vec;

use cheetah_media_types::{CodecId, TrackKind};

use crate::*;

fn ts_packet(pid: u16, payload_unit_start: bool, payload: &[u8], cc: u8) -> Vec<u8> {
    let mut pkt = vec![0xff; 188];
    pkt[0] = 0x47;
    let pid_hi = (((pid >> 8) & 0x1f) as u8) | if payload_unit_start { 0x40 } else { 0 };
    pkt[1] = pid_hi;
    pkt[2] = (pid & 0xff) as u8;
    // adaptation_field_control = 1 (payload only)
    pkt[3] = (1 << 4) | (cc & 0x0f);
    let len = payload.len().min(184);
    pkt[4..4 + len].copy_from_slice(&payload[..len]);
    pkt
}

fn pat_section(programs: &[(u16, u16)]) -> Vec<u8> {
    let n = programs.len();
    let section_length = 9 + 4 * n;
    let mut s = vec![0u8; section_length + 3];
    s[0] = 0x00;
    s[1] = 0xb0 | ((section_length >> 8) & 0x0f) as u8;
    s[2] = (section_length & 0xff) as u8;
    s[3..5].copy_from_slice(&[0x00, 0x01]);
    s[5] = 0xc1;
    s[6] = 0x00;
    s[7] = 0x00;
    for (i, (pn, pmt_pid)) in programs.iter().enumerate() {
        let off = 8 + i * 4;
        s[off..off + 2].copy_from_slice(&pn.to_be_bytes());
        s[off + 2..off + 4].copy_from_slice(&(0xe000u16 | pmt_pid).to_be_bytes());
    }
    s
}

fn pmt_section(program_number: u16, pcr_pid: u16, streams: &[(u8, u16)]) -> Vec<u8> {
    let program_info_length = 0;
    let stream_bytes = 5 * streams.len();
    // 9 bytes of fixed header after the length field plus 4 bytes CRC.
    let section_length = 13 + program_info_length + stream_bytes;
    let mut s = vec![0u8; section_length + 3];
    s[0] = 0x02;
    s[1] = 0xb0 | ((section_length >> 8) & 0x0f) as u8;
    s[2] = (section_length & 0xff) as u8;
    s[3..5].copy_from_slice(&program_number.to_be_bytes());
    s[5] = 0xc1;
    s[6] = 0x00;
    s[7] = 0x00;
    s[8..10].copy_from_slice(&(0xe000u16 | pcr_pid).to_be_bytes());
    s[10..12].copy_from_slice(&(0xf000u16 | program_info_length as u16).to_be_bytes());
    let mut off = 12;
    for (st, pid) in streams {
        s[off] = *st;
        s[off + 1..off + 3].copy_from_slice(&(0xe000u16 | pid).to_be_bytes());
        s[off + 3..off + 5].copy_from_slice(&0xf000u16.to_be_bytes());
        off += 5;
    }
    s
}

fn timestamp_bytes(ts: u64, nibble: u8) -> [u8; 5] {
    let b0 = ((nibble & 0x0f) << 4) | ((((ts >> 30) & 0x07) as u8) << 1) | 1;
    let b1 = ((ts >> 22) & 0xff) as u8;
    let b2 = ((((ts >> 15) & 0x7f) as u8) << 1) | 1;
    let b3 = ((ts >> 7) & 0xff) as u8;
    let b4 = (((ts & 0x7f) as u8) << 1) | 1;
    [b0, b1, b2, b3, b4]
}

fn pes_packet(stream_id: u8, payload: &[u8], pts: Option<u64>, dts: Option<u64>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0x00, 0x00, 0x01, stream_id]);

    let mut flags2 = 0u8;
    let mut optional = Vec::new();
    if let Some(p) = pts {
        let nibble = if dts.is_some() { 0x03 } else { 0x02 };
        optional.extend_from_slice(&timestamp_bytes(p, nibble));
    }
    if let Some(d) = dts {
        optional.extend_from_slice(&timestamp_bytes(d, 0x01));
    }
    if pts.is_some() && dts.is_some() {
        flags2 = 0xc0;
    } else if pts.is_some() {
        flags2 = 0x80;
    }

    let header_data_length = optional.len() as u16;
    let packet_length = (3 + header_data_length as usize + payload.len()) as u16;
    out.extend_from_slice(&packet_length.to_be_bytes());
    // marker bits '10', no scrambling, data_alignment_indicator=0, priority=0, copyright=0, original=0
    out.push(0x80);
    out.push(flags2);
    out.push(header_data_length as u8);
    out.extend_from_slice(&optional);
    out.extend_from_slice(payload);
    out
}

fn build_h264_pes_payload() -> Vec<u8> {
    build_large_h264_pes_payload(0)
}

fn build_large_h264_pes_payload(min_size: usize) -> Vec<u8> {
    let sps = [
        0x67u8, // NAL header type 7
        0x42, 0x00, 0x1e, // profile/constraints/level
        0xe9, 0x42, 0x10, 0x89, 0xf3, 0x22, 0xcb, 0x80,
    ];
    let pps = [0x68u8, 0xce, 0x3c, 0x80];
    let mut es = Vec::new();
    es.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
    es.extend_from_slice(&sps);
    es.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
    es.extend_from_slice(&pps);
    es.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
    // Single IDR NAL with large payload to force multi-packet PES when needed.
    es.push(0x65);
    while es.len() < min_size {
        es.push(0x80);
    }
    es
}

#[test]
fn parse_packet_header() {
    let pkt = ts_packet(0x100, true, &[], 0);
    let p = packet::TsPacket::parse(&pkt).unwrap();
    assert_eq!(p.pid, 0x100);
    assert!(p.payload_unit_start);
    assert_eq!(p.continuity_counter, 0);
    assert!(!p.transport_error);
}

#[test]
fn parse_lost_sync_byte_fails() {
    let pkt = vec![0xff; 188];
    assert_eq!(packet::TsPacket::parse(&pkt), Err(TsError::LostSync));
}

#[test]
fn section_assembler_completes_pat() {
    let section = pat_section(&[(1, 0x100)]);
    let mut asm = SectionAssembler::new();
    let mut payload = Vec::new();
    payload.push(0x00); // pointer field
    payload.extend_from_slice(&section);
    assert!(asm.feed(&payload, true).unwrap().is_some());
}

#[test]
fn section_assembler_completes_across_packets() {
    let section = pat_section(&[(1, 0x100)]);
    let mut payload = Vec::new();
    payload.push(0x00);
    payload.extend_from_slice(&section);
    let split = payload.len() / 2;
    let mut asm = SectionAssembler::new();
    assert!(asm.feed(&payload[..split], true).unwrap().is_none());
    assert!(asm.feed(&payload[split..], false).unwrap().is_some());
}

#[test]
fn pes_assembler_parses_pts_dts() {
    let payload = pes_packet(0xe0, &[0x01, 0x02, 0x03], Some(90000), Some(45000));
    let mut asm = PesAssembler::new();
    let out = asm.feed(&payload, true).unwrap().pop().unwrap();
    assert_eq!(out.header.stream_id, 0xe0);
    assert_eq!(
        out.header.pts,
        Some(cheetah_media_types::Timestamp::new(90000))
    );
    assert_eq!(
        out.header.dts,
        Some(cheetah_media_types::Timestamp::new(45000))
    );
    assert_eq!(out.payload, [0x01, 0x02, 0x03]);
}

#[test]
fn pes_assembler_unknown_length_finalizes_on_new_pusi() {
    // Build a PES with packet_length 0 (unknown) and no trailing payload bytes.
    let mut p = Vec::new();
    p.extend_from_slice(&[0x00, 0x00, 0x01, 0xe0]);
    p.extend_from_slice(&[0x00, 0x00]); // packet_length 0
    p.extend_from_slice(&[0x80, 0x80, 0x05]);
    p.extend_from_slice(&timestamp_bytes(90000, 0x02));
    p.extend_from_slice(&[0x01, 0x02]);

    let mut asm = PesAssembler::new();
    assert!(asm.feed(&p, true).unwrap().is_empty());

    let p2 = pes_packet(0xe0, &[0x03, 0x04], Some(90000 + 1800), None);
    let outputs = asm.feed(&p2, true).unwrap();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].payload, [0x01, 0x02]);
    assert_eq!(outputs[1].payload, [0x03, 0x04]);
}

#[test]
fn demuxer_basic_video_flow() {
    let mut stream = Vec::new();

    // PAT
    let mut pat_payload = vec![0x00];
    pat_payload.extend_from_slice(&pat_section(&[(1, 0x100)]));
    stream.extend_from_slice(&ts_packet(0x000, true, &pat_payload, 0));

    // PMT
    let mut pmt_payload = vec![0x00];
    pmt_payload.extend_from_slice(&pmt_section(1, 0x102, &[(0x1b, 0x101)]));
    stream.extend_from_slice(&ts_packet(0x100, true, &pmt_payload, 0));

    // Video PES
    let es = build_h264_pes_payload();
    let pes = pes_packet(0xe0, &es, Some(90000), None);
    stream.extend_from_slice(&ts_packet(0x101, true, &pes, 0));

    let mut demux = TsDemuxer::new();
    demux.push(&stream);

    let mut tracks = Vec::new();
    let mut packets = Vec::new();
    while let Some(event) = demux.next_event().unwrap() {
        match event {
            TsEvent::Track(t) => tracks.push(t),
            TsEvent::Packet(p) => packets.push(p),
            TsEvent::Clock(_) => {}
        }
    }

    assert!(!tracks.is_empty(), "expected at least one track event");
    let video_track = tracks
        .iter()
        .find(|t| t.kind == TrackKind::Video && t.codec == CodecId::H264)
        .expect("expected H.264 video track");
    assert!(!packets.is_empty(), "expected at least one packet");

    let packet = packets
        .iter()
        .find(|p| p.track_id == video_track.id)
        .expect("expected packet for video track");
    assert!(packet.flags.is_keyframe);
    let data: &[u8] = packet.payload.as_ref();
    assert!(data.windows(4).any(|w| w == [0x00, 0x00, 0x00, 0x01]));
}

#[test]
fn demuxer_chunk_splitting() {
    let mut stream = Vec::new();
    let mut pat_payload = vec![0x00];
    pat_payload.extend_from_slice(&pat_section(&[(1, 0x100)]));
    stream.extend_from_slice(&ts_packet(0x000, true, &pat_payload, 0));
    let mut pmt_payload = vec![0x00];
    pmt_payload.extend_from_slice(&pmt_section(1, 0x102, &[(0x1b, 0x101)]));
    stream.extend_from_slice(&ts_packet(0x100, true, &pmt_payload, 0));
    let es = build_large_h264_pes_payload(250);
    let pes = pes_packet(0xe0, &es, Some(90000), None);
    assert!(pes.len() > 188, "PES should span multiple TS packets");
    stream.extend_from_slice(&ts_packet(0x101, true, &pes[..184], 0));
    stream.extend_from_slice(&ts_packet(0x101, false, &pes[184..], 1));

    let mut demux = TsDemuxer::new();
    let mut packets = 0usize;
    for b in stream {
        demux.push(&[b]);
        while let Some(event) = demux.next_event().unwrap() {
            if matches!(event, TsEvent::Packet(_)) {
                packets += 1;
            }
        }
    }
    assert!(
        packets > 0,
        "expected at least one packet after chunk splitting"
    );
}

#[test]
fn demuxer_timestamp_wrap() {
    let pts_first = (1u64 << 33) - 4500;
    let pts_second = 4500u64;

    let mut stream = Vec::new();
    let mut pat_payload = vec![0x00];
    pat_payload.extend_from_slice(&pat_section(&[(1, 0x100)]));
    stream.extend_from_slice(&ts_packet(0x000, true, &pat_payload, 0));
    let mut pmt_payload = vec![0x00];
    pmt_payload.extend_from_slice(&pmt_section(1, 0x102, &[(0x1b, 0x101)]));
    stream.extend_from_slice(&ts_packet(0x100, true, &pmt_payload, 0));

    let es = build_h264_pes_payload();
    let pes1 = pes_packet(0xe0, &es, Some(pts_first), None);
    stream.extend_from_slice(&ts_packet(0x101, true, &pes1, 0));
    let pes2 = pes_packet(0xe0, &es, Some(pts_second), None);
    stream.extend_from_slice(&ts_packet(0x101, true, &pes2, 1));

    let mut demux = TsDemuxer::new();
    demux.push(&stream);

    let mut times = Vec::new();
    while let Some(event) = demux.next_event().unwrap() {
        if let TsEvent::Packet(p) = event {
            times.push(p.time.pts.unwrap_or_default().ticks());
        }
    }
    assert_eq!(times.len(), 2);
    assert!(
        times[1] > times[0],
        "wrapped PTS should unwrap to a larger value"
    );
}

#[test]
fn demuxer_continuity_loss_and_duplicate() {
    let mut stream = Vec::new();
    let mut pat_payload = vec![0x00];
    pat_payload.extend_from_slice(&pat_section(&[(1, 0x100)]));
    stream.extend_from_slice(&ts_packet(0x000, true, &pat_payload, 0));
    let mut pmt_payload = vec![0x00];
    pmt_payload.extend_from_slice(&pmt_section(1, 0x102, &[(0x1b, 0x101)]));
    stream.extend_from_slice(&ts_packet(0x100, true, &pmt_payload, 0));

    let es = build_large_h264_pes_payload(250);
    let pes = pes_packet(0xe0, &es, Some(90000), None);
    assert!(pes.len() > 188);
    let first = &pes[..184];
    let second = &pes[184..];
    stream.extend_from_slice(&ts_packet(0x101, true, first, 0));
    // duplicate with same cc should be ignored
    stream.extend_from_slice(&ts_packet(0x101, true, first, 0));
    // skip cc 1 to create loss
    stream.extend_from_slice(&ts_packet(0x101, false, second, 2));

    let mut demux = TsDemuxer::new();
    demux.push(&stream);
    let mut packets = 0usize;
    while let Some(event) = demux.next_event().unwrap() {
        if matches!(event, TsEvent::Packet(_)) {
            packets += 1;
        }
    }
    assert_eq!(packets, 0, "continuity loss should discard PES");
    assert_eq!(demux.diagnostics().discontinuities, 1);
}

#[test]
fn demuxer_transport_error_resets_pes() {
    let mut stream = Vec::new();
    let mut pat_payload = vec![0x00];
    pat_payload.extend_from_slice(&pat_section(&[(1, 0x100)]));
    stream.extend_from_slice(&ts_packet(0x000, true, &pat_payload, 0));
    let mut pmt_payload = vec![0x00];
    pmt_payload.extend_from_slice(&pmt_section(1, 0x102, &[(0x1b, 0x101)]));
    stream.extend_from_slice(&ts_packet(0x100, true, &pmt_payload, 0));

    let es = build_large_h264_pes_payload(250);
    let pes = pes_packet(0xe0, &es, Some(90000), None);
    assert!(pes.len() > 188);
    stream.extend_from_slice(&ts_packet(0x101, true, &pes[..184], 0));

    let mut bad = ts_packet(0x101, false, &pes[184..], 1);
    bad[1] |= 0x80; // transport_error_indicator
    stream.extend_from_slice(&bad);

    let mut demux = TsDemuxer::new();
    demux.push(&stream);
    let mut packets = 0usize;
    while let Some(event) = demux.next_event().unwrap() {
        if matches!(event, TsEvent::Packet(_)) {
            packets += 1;
        }
    }
    assert_eq!(packets, 0);
    assert_eq!(demux.diagnostics().discontinuities, 1);
}

#[test]
fn demuxer_pmt_update_adds_track() {
    let mut stream = Vec::new();

    let mut pat_payload = vec![0x00];
    pat_payload.extend_from_slice(&pat_section(&[(1, 0x100)]));
    stream.extend_from_slice(&ts_packet(0x000, true, &pat_payload, 0));

    let mut pmt1 = vec![0x00];
    pmt1.extend_from_slice(&pmt_section(1, 0x102, &[(0x1b, 0x101)]));
    stream.extend_from_slice(&ts_packet(0x100, true, &pmt1, 0));

    // Updated PMT adds an audio stream.
    let mut pmt2 = vec![0x00];
    pmt2.extend_from_slice(&pmt_section(1, 0x102, &[(0x1b, 0x101), (0x0f, 0x103)]));
    stream.extend_from_slice(&ts_packet(0x100, true, &pmt2, 1));

    let mut demux = TsDemuxer::new();
    demux.push(&stream);

    let mut track_kinds = Vec::new();
    while let Some(event) = demux.next_event().unwrap() {
        if let TsEvent::Track(t) = event {
            track_kinds.push(t.kind);
        }
    }
    assert_eq!(
        track_kinds
            .iter()
            .filter(|&&k| k == TrackKind::Audio)
            .count(),
        1
    );
    assert_eq!(
        track_kinds
            .iter()
            .filter(|&&k| k == TrackKind::Video)
            .count(),
        1
    );
}

#[test]
fn demuxer_multi_program() {
    let mut stream = Vec::new();
    let mut pat_payload = vec![0x00];
    pat_payload.extend_from_slice(&pat_section(&[(1, 0x100), (2, 0x200)]));
    stream.extend_from_slice(&ts_packet(0x000, true, &pat_payload, 0));

    let mut pmt1 = vec![0x00];
    pmt1.extend_from_slice(&pmt_section(1, 0x102, &[(0x1b, 0x101)]));
    stream.extend_from_slice(&ts_packet(0x100, true, &pmt1, 0));

    let mut pmt2 = vec![0x00];
    pmt2.extend_from_slice(&pmt_section(2, 0x202, &[(0x0f, 0x201)]));
    stream.extend_from_slice(&ts_packet(0x200, true, &pmt2, 0));

    let mut demux = TsDemuxer::new();
    demux.push(&stream);
    let mut tracks = 0usize;
    while let Some(event) = demux.next_event().unwrap() {
        if matches!(event, TsEvent::Track(_)) {
            tracks += 1;
        }
    }
    assert_eq!(tracks, 2);
}

#[test]
fn demuxer_fifo_track_before_packet() {
    let mut stream = Vec::new();
    let mut pat_payload = vec![0x00];
    pat_payload.extend_from_slice(&pat_section(&[(1, 0x100)]));
    stream.extend_from_slice(&ts_packet(0x000, true, &pat_payload, 0));
    let mut pmt_payload = vec![0x00];
    pmt_payload.extend_from_slice(&pmt_section(1, 0x102, &[(0x1b, 0x101)]));
    stream.extend_from_slice(&ts_packet(0x100, true, &pmt_payload, 0));

    let es = build_h264_pes_payload();
    let pes = pes_packet(0xe0, &es, Some(90000), None);
    stream.extend_from_slice(&ts_packet(0x101, true, &pes, 0));

    let mut demux = TsDemuxer::new();
    demux.push(&stream);

    let mut saw_track = false;
    while let Some(event) = demux.next_event().unwrap() {
        match event {
            TsEvent::Track(_) => saw_track = true,
            TsEvent::Packet(_) => {
                assert!(saw_track, "Track event must precede Packet event");
                return;
            }
            TsEvent::Clock(_) => {}
        }
    }
    panic!("expected at least one Packet event");
}

#[test]
fn demuxer_buffer_compaction_does_not_panic() {
    let mut stream = Vec::new();
    let mut pat_payload = vec![0x00];
    pat_payload.extend_from_slice(&pat_section(&[(1, 0x100)]));
    stream.extend_from_slice(&ts_packet(0x000, true, &pat_payload, 0));
    let mut pmt_payload = vec![0x00];
    pmt_payload.extend_from_slice(&pmt_section(1, 0x102, &[(0x1b, 0x101)]));
    stream.extend_from_slice(&ts_packet(0x100, true, &pmt_payload, 0));

    let es = build_h264_pes_payload();
    let mut cc = 0u8;
    for _ in 0..30 {
        let pes = pes_packet(0xe0, &es, Some(90000 + u64::from(cc) * 1800), None);
        stream.extend_from_slice(&ts_packet(0x101, true, &pes, cc));
        cc = (cc + 1) & 0x0f;
    }

    let mut demux = TsDemuxer::new();
    demux.push(&stream);

    let mut packets = 0usize;
    while let Some(event) = demux.next_event().unwrap() {
        if matches!(event, TsEvent::Packet(_)) {
            packets += 1;
        }
    }
    assert!(packets >= 30, "expected at least 30 packets after buffer compaction");
}
