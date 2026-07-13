//! HLS / LL-HLS playlist and segment client.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Error returned by the HLS client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HlsError {
    MissingExtM3u,
    MalformedStreamInf,
}

/// A single HLS variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub bandwidth: u32,
    pub codecs: String,
    pub uri: String,
}

/// Parsed HLS master playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterPlaylist {
    pub variants: Vec<Variant>,
}

/// Parse a simple HLS master playlist.
///
/// Only handles `#EXT-X-STREAM-INF` variants with `BANDWIDTH` and `CODECS`.
pub fn parse_master(playlist: &str) -> Result<MasterPlaylist, HlsError> {
    if !playlist.starts_with("#EXTM3U") {
        return Err(HlsError::MissingExtM3u);
    }

    let mut variants = Vec::new();
    let mut pending_inf: Option<(u32, String)> = None;

    for line in playlist.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            if line.starts_with("#EXT-X-STREAM-INF:") {
                let attr = line.trim_start_matches("#EXT-X-STREAM-INF:").trim();
                let bandwidth =
                    extract_u32(attr, "BANDWIDTH=").ok_or(HlsError::MalformedStreamInf)?;
                let codecs = extract_quoted(attr, "CODECS=")
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                pending_inf = Some((bandwidth, codecs));
            }
            continue;
        }

        if let Some((bandwidth, codecs)) = pending_inf.take() {
            variants.push(Variant {
                bandwidth,
                codecs,
                uri: line.to_string(),
            });
        }
    }

    Ok(MasterPlaylist { variants })
}

fn extract_u32(text: &str, key: &str) -> Option<u32> {
    let start = text.find(key)? + key.len();
    let end = text[start..].find(|c: char| !c.is_ascii_digit())?;
    text[start..start + end].parse().ok()
}

fn extract_quoted<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let start = text.find(key)? + key.len();
    let text = &text[start..];
    let text = text.strip_prefix('"')?;
    let end = text.find('"')?;
    Some(&text[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    const MASTER: &str = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42e00a,mp4a.40.2"
playlist_1.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=5000000,CODECS="avc1.64001f,mp4a.40.2"
playlist_2.m3u8
"#;

    #[test]
    fn parse_master_ok() {
        let m = parse_master(MASTER).unwrap();
        assert_eq!(m.variants.len(), 2);
        assert_eq!(m.variants[0].bandwidth, 1_000_000);
        assert_eq!(m.variants[0].uri, "playlist_1.m3u8");
    }

    #[test]
    fn parse_missing_extm3u_fails() {
        assert_eq!(parse_master("not a playlist"), Err(HlsError::MissingExtM3u));
    }
}
