//! HLS/LL-HLS playlist parser.

use alloc::string::{String, ToString};
use core::str::FromStr;

use crate::error::HlsError;
use crate::model::*;

const MAX_LINES: usize = 1_000_000;
const MAX_TAGS: usize = 10_000;
const MAX_LINE_LEN: usize = 16_384;
const MAX_VARIANTS: usize = 1_000;
const MAX_SEGMENTS: usize = 100_000;
const MAX_PARTS: usize = 1_000_000;
const MAX_MEDIA_RENDITIONS: usize = 1_000;

/// Parse either a master or media playlist from `input` and resolve relative
/// URIs against `base_uri`.
pub fn parse(input: &str, base_uri: &str) -> Result<Playlist, HlsError> {
    if !input.starts_with("#EXTM3U") {
        return Err(HlsError::MissingExtM3u);
    }

    if (input.len() as u64) > (MAX_LINE_LEN as u64).saturating_mul(MAX_LINES as u64) {
        return Err(HlsError::LimitExceeded {
            limit: "playlist total size",
        });
    }

    // First pass: detect whether master or media by checking for STREAM-INF.
    let mut saw_stream_inf = false;
    let mut saw_media_segment = false;
    let mut tag_count = 0u32;
    for line in input.lines() {
        if line.starts_with("#EXT-X-STREAM-INF:") || line.starts_with("#EXT-X-I-FRAME-STREAM-INF:")
        {
            saw_stream_inf = true;
        }
        if line.starts_with("#EXTINF:") || line.starts_with("#EXT-X-TARGETDURATION:") {
            saw_media_segment = true;
        }
        if line.starts_with("#EXT") {
            tag_count = tag_count.saturating_add(1);
            if tag_count as usize > MAX_TAGS {
                return Err(HlsError::LimitExceeded { limit: "tag count" });
            }
        }
    }

    if saw_stream_inf && saw_media_segment {
        return Err(HlsError::malformed(
            1,
            "playlist contains both master and media tags",
        ));
    }

    if saw_stream_inf {
        Ok(Playlist::Master(parse_master(input, base_uri)?))
    } else {
        Ok(Playlist::Media(parse_media(input, base_uri)?))
    }
}

pub fn parse_master(input: &str, base_uri: &str) -> Result<MasterPlaylist, HlsError> {
    let mut pl = MasterPlaylist::default();
    let mut pending_variant: Option<Variant> = None;
    let mut line_no = 0u32;

    for raw in input.lines() {
        line_no = line_no.saturating_add(1);
        if raw.len() > MAX_LINE_LEN {
            return Err(HlsError::LimitExceeded {
                limit: "line length",
            });
        }
        let line = raw.trim();
        if line.is_empty() || line == "#EXTM3U" {
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            let attrs = parse_attributes(tag)?;
            let mut v = Variant::new(String::new(), 0);
            v.bandwidth = parse_u32_attr(&attrs, "BANDWIDTH")?;
            v.average_bandwidth = parse_optional_u32(&attrs, "AVERAGE-BANDWIDTH")?;
            if let Some(codecs) = attrs.get("CODECS") {
                v.codecs = split_commas(codecs).collect();
            }
            v.resolution = parse_resolution(attrs.get("RESOLUTION"))?;
            v.frame_rate = parse_optional_f64(&attrs, "FRAME-RATE")?;
            if let Some(r) = attrs.get("VIDEO-RANGE") {
                v.video_range = r.clone();
            }
            if let Some(r) = attrs.get("HDCP-LEVEL") {
                v.hdcp_level = r.clone();
            }
            v.audio_group = attrs.get("AUDIO").cloned();
            v.video_group = attrs.get("VIDEO").cloned();
            v.subtitle_group = attrs.get("SUBTITLES").cloned();
            v.closed_captions_group = attrs.get("CLOSED-CAPTIONS").cloned();
            // #EXT-X-STREAM-INF has no independent-segments attribute; retain the default false.
            pending_variant = Some(v);
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-I-FRAME-STREAM-INF:") {
            let attrs = parse_attributes(tag)?;
            let mut v = Variant::new(String::new(), 0);
            v.bandwidth = parse_u32_attr(&attrs, "BANDWIDTH")?;
            v.average_bandwidth = parse_optional_u32(&attrs, "AVERAGE-BANDWIDTH")?;
            if let Some(codecs) = attrs.get("CODECS") {
                v.codecs = split_commas(codecs).collect();
            }
            v.resolution = parse_resolution(attrs.get("RESOLUTION"))?;
            v.frame_rate = parse_optional_f64(&attrs, "FRAME-RATE")?;
            if let Some(uri) = attrs.get("URI") {
                v.uri = resolve_url(base_uri, uri)?;
            } else {
                return Err(HlsError::missing_tag("URI"));
            }
            if pl.i_frame_variants.len() >= MAX_VARIANTS {
                return Err(HlsError::LimitExceeded {
                    limit: "i-frame variants",
                });
            }
            pl.i_frame_variants.push(v);
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-MEDIA:") {
            let attrs = parse_attributes(tag)?;
            let rend = parse_media_rendition(&attrs)?;
            if pl.media_renditions.len() >= MAX_MEDIA_RENDITIONS {
                return Err(HlsError::LimitExceeded {
                    limit: "media renditions",
                });
            }
            pl.media_renditions.push(rend);
            continue;
        }

        if line == "#EXT-X-INDEPENDENT-SEGMENTS" {
            pl.independent_segments = true;
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-START:") {
            let attrs = parse_attributes(tag)?;
            pl.start = Some(StartPoint {
                time_offset: parse_signed_f64_attr(&attrs, "TIME-OFFSET")?,
                precise: attrs
                    .get("PRECISE")
                    .map(|s| s.eq_ignore_ascii_case("YES"))
                    .unwrap_or(false),
            });
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-SESSION-KEY:") {
            let attrs = parse_attributes(tag)?;
            pl.session_keys.push(parse_key(&attrs)?);
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-SESSION-DATA:") {
            let attrs = parse_attributes(tag)?;
            let data = SessionData {
                data_id: clone_attr(&attrs, "DATA-ID")?,
                value: attrs.get("VALUE").cloned(),
                uri: attrs.get("URI").cloned(),
                language: attrs.get("LANGUAGE").cloned(),
            };
            pl.session_data.push(data);
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-DEFINE:") {
            let attrs = parse_attributes(tag)?;
            let var = Variable {
                name: clone_attr(&attrs, "NAME")?,
                value: attrs.get("VALUE").cloned(),
                import: attrs.get("IMPORT").cloned(),
                quote: attrs.get("QUOTES").and_then(|s| s.chars().next()),
            };
            pl.variables.push(var);
            continue;
        }

        if line.starts_with('#') {
            // Ignore unknown tags.
            continue;
        }

        if let Some(mut v) = pending_variant.take() {
            v.uri = resolve_url(base_uri, line)?;
            if pl.variants.len() >= MAX_VARIANTS {
                return Err(HlsError::LimitExceeded { limit: "variants" });
            }
            pl.variants.push(v);
        }
    }

    if pl.variants.is_empty() && pl.i_frame_variants.is_empty() && pl.media_renditions.is_empty() {
        return Err(HlsError::malformed(
            1,
            "master playlist contains no variants",
        ));
    }

    Ok(pl)
}

pub fn parse_media(input: &str, base_uri: &str) -> Result<MediaPlaylist, HlsError> {
    let mut pl = MediaPlaylist::default();
    let mut current_segment = Segment::default();
    let mut line_no = 0u32;
    let mut segment_count = 0usize;
    let mut part_count = 0usize;
    let mut msn = 0u64;

    for raw in input.lines() {
        line_no = line_no.saturating_add(1);
        if raw.len() > MAX_LINE_LEN {
            return Err(HlsError::LimitExceeded {
                limit: "line length",
            });
        }
        let line = raw.trim();
        if line.is_empty() || line == "#EXTM3U" {
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
            pl.target_duration = parse_f64_raw(tag.trim())?;
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            pl.media_sequence = parse_u64_raw(tag.trim())?;
            msn = pl.media_sequence;
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-DISCONTINUITY-SEQUENCE:") {
            pl.discontinuity_sequence = parse_u64_raw(tag.trim())?;
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-PLAYLIST-TYPE:") {
            pl.playlist_type = Some(match tag.trim() {
                "VOD" => PlaylistType::Vod,
                "EVENT" => PlaylistType::Event,
                other => {
                    return Err(HlsError::invalid_attr(
                        "#EXT-X-PLAYLIST-TYPE",
                        "value",
                        other,
                    ));
                }
            });
            continue;
        }

        if line == "#EXT-X-ENDLIST" {
            pl.end_list = true;
            continue;
        }

        if line == "#EXT-X-I-FRAMES-ONLY" {
            pl.i_frames_only = true;
            continue;
        }

        if line == "#EXT-X-INDEPENDENT-SEGMENTS" {
            pl.independent_segments = true;
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXTINF:") {
            let (dur, title) = parse_extinf(tag)?;
            current_segment.duration = dur;
            current_segment.title = title;
            continue;
        }

        if line == "#EXT-X-DISCONTINUITY" {
            current_segment.discontinuity = true;
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            current_segment.byte_range = Some(parse_byte_range_raw(tag.trim())?);
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-MAP:") {
            let attrs = parse_attributes(tag)?;
            current_segment.map = Some(Map {
                uri: resolve_url(base_uri, clone_attr(&attrs, "URI")?.as_str())?,
                byte_range: attrs
                    .get("BYTERANGE")
                    .map(|s| parse_byte_range_raw(s))
                    .transpose()?,
            });
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-KEY:") {
            let attrs = parse_attributes(tag)?;
            current_segment.key = Some(parse_key(&attrs)?);
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-PROGRAM-DATE-TIME:") {
            current_segment.program_date_time = Some(tag.trim().to_string());
            continue;
        }

        if line == "#EXT-X-GAP" {
            current_segment.gaps = current_segment.gaps.saturating_add(1);
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-PART:") {
            let attrs = parse_attributes(tag)?;
            let part = parse_part(&attrs, base_uri)?;
            part_count = part_count.saturating_add(1);
            if part_count > MAX_PARTS {
                return Err(HlsError::LimitExceeded {
                    limit: "part count",
                });
            }
            current_segment.parts.push(part);
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-PART-INF:") {
            let attrs = parse_attributes(tag)?;
            pl.part_inf = Some(PartInf {
                part_target: parse_f64_attr(&attrs, "PART-TARGET")?,
            });
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-SERVER-CONTROL:") {
            let attrs = parse_attributes(tag)?;
            pl.server_control = Some(ServerControl {
                can_block_reload: attrs
                    .get("CAN-BLOCK-RELOAD")
                    .map(|s| s.eq_ignore_ascii_case("YES"))
                    .unwrap_or(false),
                hold_back: parse_optional_f64(&attrs, "HOLD-BACK")?,
                part_hold_back: parse_optional_f64(&attrs, "PART-HOLD-BACK")?,
                can_skip_until: parse_optional_f64(&attrs, "CAN-SKIP-UNTIL")?,
                can_skip_dateranges: attrs
                    .get("CAN-SKIP-DATERANGES")
                    .map(|s| s.eq_ignore_ascii_case("YES"))
                    .unwrap_or(false),
            });
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-PRELOAD-HINT:") {
            let attrs = parse_attributes(tag)?;
            pl.preload_hint = Some(parse_preload_hint(&attrs, base_uri)?);
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-SKIP:") {
            let attrs = parse_attributes(tag)?;
            let skipped = parse_u32_attr(&attrs, "SKIPPED-SEGMENTS")?;
            let removed = attrs
                .get("RECENTLY-REMOVED-DATERANGES")
                .map(|s| split_commas(s).collect())
                .unwrap_or_default();
            pl.skip = Some(Skip {
                skipped_segments: skipped,
                recently_removed_dateranges: removed,
            });
            msn = msn.saturating_add(skipped as u64);
            continue;
        }

        if let Some(tag) = line.strip_prefix("#EXT-X-RENDITION-REPORT:") {
            let attrs = parse_attributes(tag)?;
            let report = RenditionReport {
                uri: resolve_url(base_uri, clone_attr(&attrs, "URI")?.as_str())?,
                last_msn: parse_u64_attr(&attrs, "LAST-MSN")?,
                last_part: parse_optional_u64(&attrs, "LAST-PART")?,
            };
            pl.rendition_reports.push(report);
            continue;
        }

        if line.starts_with('#') {
            continue;
        }

        // The line is a URI. Finish the current segment if it has a duration or parts.
        current_segment.uri = resolve_url(base_uri, line)?;
        current_segment.media_sequence = msn;
        msn = msn.saturating_add(1);
        segment_count = segment_count.saturating_add(1);
        if segment_count > MAX_SEGMENTS {
            return Err(HlsError::LimitExceeded { limit: "segments" });
        }
        let finished = core::mem::take(&mut current_segment);
        current_segment.map = finished.map.clone();
        current_segment.key = finished.key.clone();
        pl.segments.push(finished);
    }

    if pl.target_duration == 0.0 && !pl.segments.is_empty() {
        return Err(HlsError::MissingTag {
            tag: "#EXT-X-TARGETDURATION".into(),
        });
    }

    pl.duration = pl
        .segments
        .iter()
        .try_fold(0.0, |acc, s| {
            let sum = acc + s.duration;
            if sum.is_finite() { Some(sum) } else { None }
        })
        .ok_or(HlsError::LimitExceeded {
            limit: "playlist duration",
        })?;

    Ok(pl)
}

fn parse_media_rendition(attrs: &AttrMap) -> Result<MediaRendition, HlsError> {
    let kind = match clone_attr(attrs, "TYPE")?.as_str() {
        "AUDIO" => RenditionType::Audio,
        "VIDEO" => RenditionType::Video,
        "SUBTITLES" => RenditionType::Subtitles,
        "CLOSED-CAPTIONS" => RenditionType::ClosedCaptions,
        other => return Err(HlsError::invalid_attr("#EXT-X-MEDIA", "TYPE", other)),
    };
    Ok(MediaRendition {
        kind,
        uri: attrs.get("URI").cloned(),
        group_id: clone_attr(attrs, "GROUP-ID")?,
        language: attrs.get("LANGUAGE").cloned(),
        assoc_language: attrs.get("ASSOC-LANGUAGE").cloned(),
        name: clone_attr(attrs, "NAME")?,
        default: attrs
            .get("DEFAULT")
            .map(|s| s.eq_ignore_ascii_case("YES"))
            .unwrap_or(false),
        auto_select: attrs
            .get("AUTO-SELECT")
            .map(|s| s.eq_ignore_ascii_case("YES"))
            .unwrap_or(false),
        forced: attrs
            .get("FORCED")
            .map(|s| s.eq_ignore_ascii_case("YES"))
            .unwrap_or(false),
        in_stream_id: attrs.get("INSTREAM-ID").cloned(),
        characteristics: attrs
            .get("CHARACTERISTICS")
            .map(|s| split_commas(s).collect())
            .unwrap_or_default(),
        channels: attrs.get("CHANNELS").cloned(),
    })
}

fn parse_key(attrs: &AttrMap) -> Result<Key, HlsError> {
    Ok(Key {
        method: clone_attr(attrs, "METHOD")?,
        uri: attrs.get("URI").cloned(),
        iv: attrs.get("IV").cloned(),
        key_format: attrs
            .get("KEYFORMAT")
            .cloned()
            .unwrap_or_else(|| "identity".to_string()),
        key_format_versions: attrs
            .get("KEYFORMATVERSIONS")
            .cloned()
            .unwrap_or_else(|| "1".to_string()),
    })
}

fn parse_part(attrs: &AttrMap, base_uri: &str) -> Result<Part, HlsError> {
    Ok(Part {
        duration: parse_f64_attr(attrs, "DURATION")?,
        uri: resolve_url(base_uri, clone_attr(attrs, "URI")?.as_str())?,
        independent: attrs
            .get("INDEPENDENT")
            .map(|s| s.eq_ignore_ascii_case("YES"))
            .unwrap_or(false),
        gap: attrs
            .get("GAP")
            .map(|s| s.eq_ignore_ascii_case("YES"))
            .unwrap_or(false),
        byte_range: attrs
            .get("BYTERANGE")
            .map(|s| parse_byte_range_raw(s))
            .transpose()?,
    })
}

fn parse_preload_hint(attrs: &AttrMap, base_uri: &str) -> Result<PreloadHint, HlsError> {
    let kind = match clone_attr(attrs, "TYPE")?.as_str() {
        "PART" => PreloadHintType::Part,
        "MAP" => PreloadHintType::Map,
        other => return Err(HlsError::invalid_attr("#EXT-X-PRELOAD-HINT", "TYPE", other)),
    };
    Ok(PreloadHint {
        kind,
        uri: resolve_url(base_uri, clone_attr(attrs, "URI")?.as_str())?,
        byte_range: attrs
            .get("BYTERANGE")
            .map(|s| parse_byte_range_raw(s))
            .transpose()?,
    })
}

fn parse_extinf(tag: &str) -> Result<(f64, Option<String>), HlsError> {
    let comma = tag.find(',').unwrap_or(tag.len());
    let dur_str = &tag[..comma];
    let title = if comma < tag.len() {
        Some(tag[comma + 1..].to_string())
    } else {
        None
    };
    Ok((parse_f64_raw(dur_str.trim())?, title))
}

fn parse_attributes(input: &str) -> Result<AttrMap, HlsError> {
    let mut map = AttrMap::new();
    let mut i = 0usize;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        // Skip leading whitespace/commas.
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b',') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        // Read key.
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' {
            i += 1;
        }
        if i >= bytes.len() {
            return Err(HlsError::malformed(0, "attribute missing '='"));
        }
        let key = &input[key_start..i];
        i += 1; // skip '='
        if i >= bytes.len() {
            map.insert(key.to_string(), String::new());
            break;
        }
        let value = if bytes[i] == b'"' {
            i += 1;
            let value_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            if i >= bytes.len() {
                return Err(HlsError::malformed(0, "unterminated quoted attribute"));
            }
            let value = &input[value_start..i];
            i += 1; // skip closing quote
            value.to_string()
        } else {
            let value_start = i;
            while i < bytes.len() && bytes[i] != b',' {
                i += 1;
            }
            input[value_start..i].trim().to_string()
        };
        map.insert(key.to_string(), value);
    }
    Ok(map)
}

type AttrMap = alloc::collections::BTreeMap<String, String>;

fn clone_attr(attrs: &AttrMap, key: &str) -> Result<String, HlsError> {
    attrs
        .get(key)
        .cloned()
        .ok_or_else(|| HlsError::missing_tag(key))
}

fn parse_u32_attr(attrs: &AttrMap, key: &str) -> Result<u32, HlsError> {
    let v = clone_attr(attrs, key)?;
    v.parse::<u32>()
        .map_err(|_| HlsError::invalid_attr("", key, v))
}

fn parse_optional_u32(attrs: &AttrMap, key: &str) -> Result<Option<u32>, HlsError> {
    match attrs.get(key) {
        Some(v) => v
            .parse::<u32>()
            .map(Some)
            .map_err(|_| HlsError::invalid_attr("", key, v.clone())),
        None => Ok(None),
    }
}

fn parse_u64_attr(attrs: &AttrMap, key: &str) -> Result<u64, HlsError> {
    let v = clone_attr(attrs, key)?;
    v.parse::<u64>()
        .map_err(|_| HlsError::invalid_attr("", key, v))
}

fn parse_optional_u64(attrs: &AttrMap, key: &str) -> Result<Option<u64>, HlsError> {
    match attrs.get(key) {
        Some(v) => v
            .parse::<u64>()
            .map(Some)
            .map_err(|_| HlsError::invalid_attr("", key, v.clone())),
        None => Ok(None),
    }
}

fn parse_signed_f64_attr(attrs: &AttrMap, key: &str) -> Result<f64, HlsError> {
    let v = clone_attr(attrs, key)?;
    parse_signed_f64_raw(&v)
}

fn parse_f64_attr(attrs: &AttrMap, key: &str) -> Result<f64, HlsError> {
    let v = clone_attr(attrs, key)?;
    parse_f64_raw(&v)
}

fn parse_optional_f64(attrs: &AttrMap, key: &str) -> Result<Option<f64>, HlsError> {
    match attrs.get(key) {
        Some(v) => parse_f64_raw(v).map(Some),
        None => Ok(None),
    }
}

fn parse_f64_raw(s: &str) -> Result<f64, HlsError> {
    let v = f64::from_str(s).map_err(|_| HlsError::invalid_attr("", "", s.to_string()))?;
    if v.is_finite() && v >= 0.0 {
        Ok(v)
    } else {
        Err(HlsError::invalid_attr("", "", s.to_string()))
    }
}

fn parse_signed_f64_raw(s: &str) -> Result<f64, HlsError> {
    let v = f64::from_str(s).map_err(|_| HlsError::invalid_attr("", "", s.to_string()))?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(HlsError::invalid_attr("", "", s.to_string()))
    }
}

fn parse_u64_raw(s: &str) -> Result<u64, HlsError> {
    s.parse::<u64>()
        .map_err(|_| HlsError::invalid_attr("", "", s.to_string()))
}

fn parse_byte_range_raw(s: &str) -> Result<ByteRange, HlsError> {
    let mut parts = s.split('@');
    let length = parts
        .next()
        .ok_or_else(|| HlsError::malformed(0, "empty byterange"))?
        .trim()
        .parse::<u64>()
        .map_err(|_| HlsError::invalid_attr("", "BYTERANGE", s.to_string()))?;
    let offset = match parts.next() {
        Some(o) => Some(
            o.trim()
                .parse::<u64>()
                .map_err(|_| HlsError::invalid_attr("", "BYTERANGE", s.to_string()))?,
        ),
        None => None,
    };
    Ok(ByteRange { length, offset })
}

fn parse_resolution(value: Option<&String>) -> Result<Option<(u32, u32)>, HlsError> {
    match value {
        Some(v) => {
            let mut parts = v.split('x');
            let w = parts
                .next()
                .ok_or_else(|| HlsError::invalid_attr("", "RESOLUTION", v.clone()))?
                .parse::<u32>()
                .map_err(|_| HlsError::invalid_attr("", "RESOLUTION", v.clone()))?;
            let h = parts
                .next()
                .ok_or_else(|| HlsError::invalid_attr("", "RESOLUTION", v.clone()))?
                .parse::<u32>()
                .map_err(|_| HlsError::invalid_attr("", "RESOLUTION", v.clone()))?;
            Ok(Some((w, h)))
        }
        None => Ok(None),
    }
}

fn split_commas<'a>(s: &'a str) -> impl Iterator<Item = String> + 'a {
    s.split(',').map(|s| s.trim().to_string())
}

/// Resolve `uri` against `base_uri`.
fn resolve_url(base_uri: &str, uri: &str) -> Result<String, HlsError> {
    if uri.is_empty() {
        return Err(HlsError::malformed(0, "empty URI"));
    }
    if uri.contains("://") {
        return Ok(uri.to_string());
    }
    if uri.starts_with('/') {
        // Absolute path: keep scheme://authority only.
        if let Some(scheme_end) = base_uri.find("://") {
            let after_scheme = &base_uri[scheme_end + 3..];
            let authority_end = after_scheme.find('/').unwrap_or(after_scheme.len());
            let root = &base_uri[..scheme_end + 3 + authority_end];
            return Ok(format!("{}{}", root, uri));
        }
        return Ok(uri.to_string());
    }
    // Relative path: append to directory of base.
    let base_dir = if let Some(slash) = base_uri.rfind('/') {
        // Make sure we don't match the '/' inside '://'.
        if slash >= 2 && &base_uri[slash - 2..=slash] == "://" {
            // No path component; append a slash after the authority.
            return Ok(format!("{}/{}", base_uri, uri));
        }
        &base_uri[..slash + 1]
    } else {
        base_uri
    };
    Ok(format!("{}{}", base_dir, uri))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_absolute_uri() {
        assert_eq!(
            resolve_url("http://x/a/b.m3u8", "http://y/c.m3u8").unwrap(),
            "http://y/c.m3u8"
        );
    }

    #[test]
    fn resolve_root_path() {
        assert_eq!(
            resolve_url("http://x/a/b.m3u8", "/c.m3u8").unwrap(),
            "http://x/c.m3u8"
        );
    }

    #[test]
    fn resolve_relative_path() {
        assert_eq!(
            resolve_url("http://x/a/b.m3u8", "c.m3u8").unwrap(),
            "http://x/a/c.m3u8"
        );
    }

    #[test]
    fn resolve_relative_no_path() {
        assert_eq!(
            resolve_url("http://example.com", "playlist.m3u8").unwrap(),
            "http://example.com/playlist.m3u8"
        );
        assert_eq!(
            resolve_url("http://example.com/", "playlist.m3u8").unwrap(),
            "http://example.com/playlist.m3u8"
        );
    }
}
