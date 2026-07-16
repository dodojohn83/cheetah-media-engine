//! Data models for HLS master and media playlists.

use alloc::string::String;
use alloc::vec::Vec;

/// A parsed HLS playlist.
#[derive(Debug, Clone, PartialEq)]
pub enum Playlist {
    Master(MasterPlaylist),
    Media(MediaPlaylist),
}

/// Master playlist.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MasterPlaylist {
    pub variants: Vec<Variant>,
    pub i_frame_variants: Vec<Variant>,
    pub media_renditions: Vec<MediaRendition>,
    pub independent_segments: bool,
    pub start: Option<StartPoint>,
    pub session_keys: Vec<Key>,
    pub session_data: Vec<SessionData>,
    pub variables: Vec<Variable>,
}

/// Media playlist.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaPlaylist {
    pub target_duration: f64,
    pub media_sequence: u64,
    pub discontinuity_sequence: u64,
    pub playlist_type: Option<PlaylistType>,
    pub end_list: bool,
    pub i_frames_only: bool,
    pub independent_segments: bool,
    pub segments: Vec<Segment>,
    pub part_inf: Option<PartInf>,
    pub server_control: Option<ServerControl>,
    pub preload_hint: Option<PreloadHint>,
    pub skip: Option<Skip>,
    pub rendition_reports: Vec<RenditionReport>,
    /// Total duration in seconds for VOD playlists (sum of segment durations).
    pub duration: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaylistType {
    #[default]
    Vod,
    Event,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub uri: String,
    pub bandwidth: u32,
    pub average_bandwidth: Option<u32>,
    pub codecs: Vec<String>,
    pub resolution: Option<(u32, u32)>,
    pub frame_rate: Option<f64>,
    pub video_range: String,
    pub hdcp_level: String,
    pub audio_group: Option<String>,
    pub video_group: Option<String>,
    pub subtitle_group: Option<String>,
    pub closed_captions_group: Option<String>,
    pub associated_independent_segments: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRendition {
    pub kind: RenditionType,
    pub uri: Option<String>,
    pub group_id: String,
    pub language: Option<String>,
    pub assoc_language: Option<String>,
    pub name: String,
    pub default: bool,
    pub auto_select: bool,
    pub forced: bool,
    pub in_stream_id: Option<String>,
    pub characteristics: Vec<String>,
    pub channels: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenditionType {
    Audio,
    Video,
    Subtitles,
    ClosedCaptions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StartPoint {
    pub time_offset: f64,
    pub precise: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    pub method: String,
    pub uri: Option<String>,
    pub iv: Option<String>,
    pub key_format: String,
    pub key_format_versions: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionData {
    pub data_id: String,
    pub value: Option<String>,
    pub uri: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub value: Option<String>,
    pub import: Option<String>,
    pub quote: Option<char>,
}

/// A media segment.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Segment {
    pub duration: f64,
    pub title: Option<String>,
    pub uri: String,
    pub byte_range: Option<ByteRange>,
    pub map: Option<Map>,
    pub key: Option<Key>,
    pub discontinuity: bool,
    pub program_date_time: Option<String>,
    pub gaps: u32,
    pub parts: Vec<Part>,
    pub independent: bool,
    pub media_sequence: u64,
}

/// A partial segment in LL-HLS.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Part {
    pub duration: f64,
    pub uri: String,
    pub independent: bool,
    pub gap: bool,
    pub byte_range: Option<ByteRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub length: u64,
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Map {
    pub uri: String,
    pub byte_range: Option<ByteRange>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartInf {
    pub part_target: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServerControl {
    pub can_block_reload: bool,
    pub hold_back: Option<f64>,
    pub part_hold_back: Option<f64>,
    pub can_skip_until: Option<f64>,
    pub can_skip_dateranges: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreloadHint {
    pub kind: PreloadHintType,
    pub uri: String,
    pub byte_range: Option<ByteRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreloadHintType {
    Part,
    Map,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skip {
    pub skipped_segments: u32,
    pub recently_removed_dateranges: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenditionReport {
    pub uri: String,
    pub last_msn: u64,
    pub last_part: Option<u64>,
}

impl Variant {
    pub fn new(uri: String, bandwidth: u32) -> Self {
        Self {
            uri,
            bandwidth,
            average_bandwidth: None,
            codecs: Vec::new(),
            resolution: None,
            frame_rate: None,
            video_range: String::new(),
            hdcp_level: String::new(),
            audio_group: None,
            video_group: None,
            subtitle_group: None,
            closed_captions_group: None,
            associated_independent_segments: false,
        }
    }
}

impl MediaPlaylist {
    pub fn is_vod(&self) -> bool {
        self.playlist_type == Some(PlaylistType::Vod) || self.end_list
    }
}

impl Default for Variant {
    fn default() -> Self {
        Self::new(String::new(), 0)
    }
}
