//! Integration tests for the HLS/LL-HLS client.

use alloc::vec;

use crate::client::{ActionKind, HlsClient, HlsConfig, HlsEvent};
use crate::model::*;
use crate::parser::{parse, parse_master, parse_media};
use crate::variant::{VariantCapabilities, select_initial_variant};

const MASTER: &str = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42e00a,mp4a.40.2",RESOLUTION=640x360
playlist_1.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=5000000,CODECS="avc1.64001f,mp4a.40.2",RESOLUTION=1920x1080
playlist_2.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2500000
playlist_3.m3u8
"#;

const MEDIA: &str = r#"#EXTM3U
#EXT-X-TARGETDURATION:6
#EXT-X-MEDIA-SEQUENCE:0
#EXTINF:6.000,
seg0.ts
#EXTINF:6.000,
seg1.ts
#EXT-X-ENDLIST
"#;

#[test]
fn parse_master_ok() {
    let m = parse_master(MASTER, "http://example.com/master.m3u8").unwrap();
    assert_eq!(m.variants.len(), 3);
    assert_eq!(m.variants[0].bandwidth, 1_000_000);
    assert_eq!(m.variants[0].uri, "http://example.com/playlist_1.m3u8");
    assert_eq!(m.variants[0].resolution, Some((640, 360)));
    assert_eq!(m.variants[2].uri, "http://example.com/playlist_3.m3u8");
}

#[test]
fn parse_missing_extm3u_fails() {
    assert!(parse("not a playlist", "http://x").is_err());
}

#[test]
fn parse_media_ok() {
    let media = parse_media(MEDIA, "http://example.com/playlist.m3u8").unwrap();
    assert_eq!(media.target_duration, 6.0);
    assert_eq!(media.media_sequence, 0);
    assert_eq!(media.segments.len(), 2);
    assert_eq!(media.segments[0].uri, "http://example.com/seg0.ts");
    assert!(media.end_list);
}

#[test]
fn parse_llhls_parts() {
    let pl = r#"#EXTM3U
#EXT-X-TARGETDURATION:6
#EXT-X-PART-INF:PART-TARGET=0.5
#EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES,PART-HOLD-BACK=1.5
#EXT-X-MEDIA-SEQUENCE:10
#EXT-X-PART:DURATION=0.5,URI="part10_0.m4s",INDEPENDENT=YES
#EXT-X-PART:DURATION=0.5,URI="part10_1.m4s"
#EXTINF:1.0,
seg10.m4s
"#;
    let media = parse_media(pl, "http://x/").unwrap();
    assert_eq!(media.part_inf.as_ref().unwrap().part_target, 0.5);
    assert!(media.server_control.as_ref().unwrap().can_block_reload);
    assert_eq!(media.segments.len(), 1);
    assert_eq!(media.segments[0].parts.len(), 2);
    assert!(media.segments[0].parts[0].independent);
}

#[test]
fn variant_selection_prefers_highest_under_cap() {
    let master = parse_master(MASTER, "http://x/").unwrap();
    let caps = VariantCapabilities {
        max_bandwidth: Some(3_000_000),
        required_codecs: vec!["avc1".to_string(), "mp4a".to_string()],
        ..Default::default()
    };
    let v = select_initial_variant(&master.variants, &caps).unwrap();
    assert_eq!(v.bandwidth, 1_000_000);
}

#[test]
fn client_start_loads_master() {
    let mut client = HlsClient::new("http://x/master.m3u8", HlsConfig::default());
    let actions = client.start();
    assert_eq!(actions.len(), 1);
    match &actions[0].kind {
        ActionKind::LoadPlaylist { url, is_master, .. } => {
            assert_eq!(url, "http://x/master.m3u8");
            assert!(*is_master);
        }
        _ => panic!("expected LoadPlaylist"),
    }
}

#[test]
fn client_loads_media_then_segments() {
    let mut client = HlsClient::new("http://x/master.m3u8", HlsConfig::default());
    let _ = client.start();
    let body = MASTER.as_bytes().to_vec();
    let actions = client.handle_event(HlsEvent::PlaylistLoaded {
        url: "http://x/master.m3u8".to_string(),
        body,
    });
    let media_url = actions
        .iter()
        .find_map(|a| match &a.kind {
            ActionKind::LoadPlaylist { url, .. } => Some(url.clone()),
            _ => None,
        })
        .expect("expected media playlist load");

    let media_body = MEDIA.as_bytes().to_vec();
    let actions = client.handle_event(HlsEvent::PlaylistLoaded {
        url: media_url,
        body: media_body,
    });
    let uri = actions
        .iter()
        .find_map(|a| match &a.kind {
            ActionKind::LoadSegment { uri, .. } | ActionKind::LoadPart { uri, .. } => {
                Some(uri.clone())
            }
            _ => None,
        })
        .expect("expected LoadSegment or LoadPart");
    assert!(uri.contains("seg0.ts"));
}

#[test]
fn client_stops_cancels_requests() {
    let mut client = HlsClient::new("http://x/master.m3u8", HlsConfig::default());
    let _ = client.start();
    let actions = client.handle_event(HlsEvent::Stop);
    assert!(actions.is_empty());
    assert!(client.stopped());
}

#[test]
fn parse_byte_range_map_and_preload_hint() {
    let pl = r#"#EXTM3U
#EXT-X-TARGETDURATION:6
#EXT-X-MAP:URI="init.mp4",BYTERANGE="1234@0"
#EXT-X-PRELOAD-HINT:TYPE=PART,URI="next0.m4s",BYTERANGE="200@0"
#EXTINF:6.0,
seg0.mp4
"#;
    let media = parse_media(pl, "http://x/").unwrap();
    let map = media.segments[0].map.as_ref().unwrap();
    assert_eq!(map.byte_range.as_ref().unwrap().length, 1234);
    assert_eq!(map.byte_range.as_ref().unwrap().offset, Some(0));
    let hint = media.preload_hint.as_ref().unwrap();
    assert_eq!(hint.kind, PreloadHintType::Part);
    assert_eq!(hint.byte_range.as_ref().unwrap().length, 200);
}

#[test]
fn master_variant_count_limit_rejected() {
    let mut master = String::from("#EXTM3U\n");
    for i in 0..=1000 {
        master.push_str(&alloc::format!("#EXT-X-STREAM-INF:BANDWIDTH={}\n", i + 1));
        master.push_str(&alloc::format!("v{}.m3u8\n", i));
    }
    assert!(parse_master(&master, "http://x/").is_err());
}

#[test]
fn resolve_absolute_path_no_double_slash() {
    let master = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42e00a,mp4a.40.2"
/live/playlist_1.m3u8
"#;
    let m = parse_master(master, "http://example.com").unwrap();
    assert_eq!(m.variants[0].uri, "http://example.com/live/playlist_1.m3u8");

    let m2 = parse_master(master, "http://example.com/").unwrap();
    assert_eq!(
        m2.variants[0].uri,
        "http://example.com/live/playlist_1.m3u8"
    );
}

#[test]
fn llhls_tick_uses_part_target_interval() {
    let media = r#"#EXTM3U
#EXT-X-TARGETDURATION:6
#EXT-X-PART-INF:PART-TARGET=0.5
#EXT-X-SERVER-CONTROL:CAN-BLOCK-LOAD=YES,PART-HOLD-BACK=1.5
#EXT-X-MEDIA-SEQUENCE:0
#EXT-X-PART:DURATION=0.5,URI="part0.m4s",INDEPENDENT=YES
#EXTINF:1.0,
seg0.m4s
"#;
    let mut client = HlsClient::new("http://x/master.m3u8", HlsConfig::default());
    let _ = client.start();
    client.handle_event(HlsEvent::PlaylistLoaded {
        url: "http://x/master.m3u8".to_string(),
        body: MASTER.as_bytes().to_vec(),
    });
    let media_url = "http://example.com/playlist_1.m3u8".to_string();
    client.handle_event(HlsEvent::PlaylistLoaded {
        url: media_url.clone(),
        body: media.as_bytes().to_vec(),
    });

    // First tick should always reload.
    let first = client.handle_event(HlsEvent::Tick { now_ms: 0 });
    assert!(
        first
            .iter()
            .any(|a| matches!(a.kind, ActionKind::LoadPlaylist { .. }))
    );

    // Simulate playlist response so the client does not treat the reload as in-flight.
    client.handle_event(HlsEvent::PlaylistLoaded {
        url: media_url.clone(),
        body: media.as_bytes().to_vec(),
    });

    // 200 ms is less than 0.5 s part target, so no reload.
    let second = client.handle_event(HlsEvent::Tick { now_ms: 200 });
    assert!(
        !second
            .iter()
            .any(|a| matches!(a.kind, ActionKind::LoadPlaylist { .. }))
    );

    // 600 ms exceeds 500 ms part target, reload again.
    let third = client.handle_event(HlsEvent::Tick { now_ms: 600 });
    assert!(
        third
            .iter()
            .any(|a| matches!(a.kind, ActionKind::LoadPlaylist { .. }))
    );
}

#[test]
fn variant_selection_fallback_to_lowest_when_all_exceed_cap() {
    let master = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42e00a,mp4a.40.2"
low.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=5000000,CODECS="avc1.42e00a,mp4a.40.2"
mid.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=10000000,CODECS="avc1.42e00a,mp4a.40.2"
high.m3u8
"#;
    let m = parse_master(master, "http://x/").unwrap();
    let caps = VariantCapabilities {
        max_bandwidth: Some(500_000),
        required_codecs: alloc::vec!["avc1".into()],
        ..VariantCapabilities::default()
    };
    let v = select_initial_variant(&m.variants, &caps).unwrap();
    assert_eq!(v.bandwidth, 1_000_000);
}

#[test]
fn parse_vod_computes_duration() {
    let media = parse_media(MEDIA, "http://example.com/playlist.m3u8").unwrap();
    assert!(media.is_vod());
    assert!((media.duration - 12.0).abs() < f64::EPSILON);
}

#[test]
fn vod_does_not_reload_on_tick() {
    let mut client = HlsClient::new("http://x/master.m3u8", HlsConfig::default());
    let _ = client.start();
    client.handle_event(HlsEvent::PlaylistLoaded {
        url: "http://x/master.m3u8".to_string(),
        body: MASTER.as_bytes().to_vec(),
    });
    let media_url = "http://example.com/playlist_1.m3u8".to_string();
    client.handle_event(HlsEvent::PlaylistLoaded {
        url: media_url.clone(),
        body: MEDIA.as_bytes().to_vec(),
    });

    let actions = client.handle_event(HlsEvent::Tick { now_ms: 10_000 });
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a.kind, ActionKind::LoadPlaylist { .. }))
    );
}

#[test]
fn vod_seek_selects_target_segment() {
    let mut client = HlsClient::new("http://x/master.m3u8", HlsConfig::default());
    let _ = client.start();
    client.handle_event(HlsEvent::PlaylistLoaded {
        url: "http://x/master.m3u8".to_string(),
        body: MASTER.as_bytes().to_vec(),
    });
    let media_url = "http://example.com/playlist_1.m3u8".to_string();
    client.handle_event(HlsEvent::PlaylistLoaded {
        url: media_url.clone(),
        body: MEDIA.as_bytes().to_vec(),
    });

    let actions = client.handle_event(HlsEvent::Seek { time_ms: 7_000 });
    let uri = actions
        .iter()
        .find_map(|a| match &a.kind {
            ActionKind::LoadSegment { uri, .. } => Some(uri.clone()),
            _ => None,
        })
        .expect("expected LoadSegment after seek");
    assert!(uri.contains("seg1.ts"));
}

#[test]
fn vod_seek_out_of_range_stops_client() {
    let mut client = HlsClient::new("http://x/master.m3u8", HlsConfig::default());
    let _ = client.start();
    client.handle_event(HlsEvent::PlaylistLoaded {
        url: "http://x/master.m3u8".to_string(),
        body: MASTER.as_bytes().to_vec(),
    });
    let media_url = "http://example.com/playlist_1.m3u8".to_string();
    client.handle_event(HlsEvent::PlaylistLoaded {
        url: media_url.clone(),
        body: MEDIA.as_bytes().to_vec(),
    });

    let actions = client.handle_event(HlsEvent::Seek { time_ms: 120_000 });
    assert!(
        actions
            .iter()
            .any(|a| matches!(a.kind, ActionKind::ReportError { .. }))
    );
    assert!(client.stopped());
}

#[test]
fn live_event_seek_is_unsupported() {
    let event_media = r#"#EXTM3U
#EXT-X-TARGETDURATION:6
#EXT-X-PLAYLIST-TYPE:EVENT
#EXT-X-MEDIA-SEQUENCE:0
#EXTINF:6.000,
seg0.ts
"#;
    let mut client = HlsClient::new("http://x/master.m3u8", HlsConfig::default());
    let _ = client.start();
    client.handle_event(HlsEvent::PlaylistLoaded {
        url: "http://x/master.m3u8".to_string(),
        body: MASTER.as_bytes().to_vec(),
    });
    client.handle_event(HlsEvent::PlaylistLoaded {
        url: "http://example.com/playlist_1.m3u8".to_string(),
        body: event_media.as_bytes().to_vec(),
    });

    let actions = client.handle_event(HlsEvent::Seek { time_ms: 1_000 });
    assert!(
        actions
            .iter()
            .any(|a| matches!(a.kind, ActionKind::ReportError { .. }))
    );
}

#[test]
fn parse_media_rejects_non_finite_duration() {
    let pl = r#"#EXTM3U
#EXT-X-TARGETDURATION:6
#EXT-X-MEDIA-SEQUENCE:0
#EXTINF:inf,
seg0.ts
"#;
    assert!(parse_media(pl, "http://x/").is_err());
}

#[test]
fn parse_media_rejects_negative_duration() {
    let pl = r#"#EXTM3U
#EXT-X-TARGETDURATION:6
#EXT-X-MEDIA-SEQUENCE:0
#EXTINF:-1.0,
seg0.ts
"#;
    assert!(parse_media(pl, "http://x/").is_err());
}
