//! Sans-I/O HLS/LL-HLS client state machine.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::HlsError;
use crate::model::*;
use crate::parser::parse;
use crate::variant::{VariantCapabilities, VariantSelector};

/// Request issued by the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlsAction {
    pub epoch: u64,
    pub kind: ActionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionKind {
    LoadPlaylist {
        url: String,
        is_master: bool,
        blocking: bool,
    },
    LoadSegment {
        uri: String,
        byte_range: Option<ByteRange>,
        discontinuity: bool,
    },
    LoadPart {
        uri: String,
        byte_range: Option<ByteRange>,
    },
    /// Cancel all requests from an older epoch.
    CancelEpoch { epoch: u64 },
    /// Wait before the next poll.
    Wait { duration_ms: u64 },
    /// Report a terminal error to the caller.
    ReportError { error: HlsError },
}

/// Incoming event into the client.
#[derive(Debug, Clone)]
pub enum HlsEvent {
    /// Bytes of a playlist were received.
    PlaylistLoaded { url: String, body: Vec<u8> },
    /// A segment or part download succeeded.
    ResourceLoaded {
        uri: String,
        body: Vec<u8>,
        epoch: u64,
    },
    /// A request failed and may be retried.
    RequestFailed {
        uri: String,
        retryable: bool,
        epoch: u64,
    },
    /// Drive the live reload clock forward.
    Tick { now_ms: u64 },
    /// Jump to a target time in a VOD or finished EVENT playlist.
    Seek { time_ms: u64 },
    /// Stop the client.
    Stop,
}

/// Configuration for the HLS client.
#[derive(Debug, Clone)]
pub struct HlsConfig {
    pub capabilities: VariantCapabilities,
    pub selector: alloc::sync::Arc<dyn VariantSelector>,
    pub target_latency_ms: u64,
    pub max_retry_count: u32,
    pub reload_interval_ms: u64,
    pub block_reload: bool,
}

impl Default for HlsConfig {
    fn default() -> Self {
        Self {
            capabilities: VariantCapabilities::default(),
            selector: alloc::sync::Arc::new(crate::variant::BandwidthSelector),
            target_latency_ms: 3_000,
            max_retry_count: 3,
            reload_interval_ms: 1_000,
            block_reload: true,
        }
    }
}

/// Sans-I/O HLS/LL-HLS client.
#[derive(Debug)]
pub struct HlsClient {
    config: HlsConfig,
    master_url: String,
    master: Option<MasterPlaylist>,
    media_url: Option<String>,
    media: Option<MediaPlaylist>,
    epoch: u64,
    /// In-flight requests keyed by epoch.
    inflight: BTreeMap<u64, ActionKind>,
    selected_variant: Option<String>,
    next_segment_index: usize,
    next_part_index: usize,
    stopped: bool,
    last_reload_ms: Option<u64>,
    retry_counts: BTreeMap<String, u32>,
    last_msn: Option<u64>,
    last_part_msn: Option<u64>,
    last_part_index: Option<usize>,
}

impl HlsClient {
    pub fn new(master_url: impl Into<String>, config: HlsConfig) -> Self {
        Self {
            config,
            master_url: master_url.into(),
            master: None,
            media_url: None,
            media: None,
            epoch: 0,
            inflight: BTreeMap::new(),
            selected_variant: None,
            next_segment_index: 0,
            next_part_index: 0,
            stopped: false,
            last_reload_ms: None,
            retry_counts: BTreeMap::new(),
            last_msn: None,
            last_part_msn: None,
            last_part_index: None,
        }
    }

    pub fn master_url(&self) -> &str {
        &self.master_url
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn stopped(&self) -> bool {
        self.stopped
    }

    /// Start the client. Returns the initial actions.
    pub fn start(&mut self) -> Vec<HlsAction> {
        if self.stopped {
            return Vec::new();
        }
        self.request_epoch(ActionKind::LoadPlaylist {
            url: self.master_url.clone(),
            is_master: true,
            blocking: false,
        })
    }

    /// Process an incoming event and produce new actions.
    pub fn handle_event(&mut self, event: HlsEvent) -> Vec<HlsAction> {
        if self.stopped {
            return Vec::new();
        }
        match event {
            HlsEvent::PlaylistLoaded { url, body } => self.on_playlist_loaded(url, body),
            HlsEvent::ResourceLoaded { uri, epoch, .. } => self.on_resource_loaded(uri, epoch),
            HlsEvent::RequestFailed {
                uri,
                retryable,
                epoch,
                ..
            } => self.on_request_failed(uri, retryable, epoch),
            HlsEvent::Tick { now_ms } => self.on_tick(now_ms),
            HlsEvent::Seek { time_ms } => self.on_seek(time_ms),
            HlsEvent::Stop => {
                self.stopped = true;
                self.inflight.clear();
                Vec::new()
            }
        }
    }

    fn on_playlist_loaded(&mut self, url: String, body: Vec<u8>) -> Vec<HlsAction> {
        let is_master = self.master_url == url;
        self.remove_inflight_playlist(&url, is_master);
        let text = match core::str::from_utf8(&body) {
            Ok(t) => t,
            Err(_) => {
                return self.stop_with_error(HlsError::Utf8Error);
            }
        };
        let base = &url;
        match parse(text, base) {
            Ok(Playlist::Master(master)) => {
                self.master = Some(master);
                let selected_uri = match self.config.selector.select(
                    &self.master.as_ref().unwrap().variants,
                    &self.config.capabilities,
                ) {
                    Ok(v) => v.uri.clone(),
                    Err(e) => return self.stop_with_error(e),
                };
                self.selected_variant = Some(selected_uri.clone());
                let mut actions = self.bump_epoch();
                actions.extend(self.request_epoch(ActionKind::LoadPlaylist {
                    url: selected_uri,
                    is_master: false,
                    blocking: false,
                }));
                actions
            }
            Ok(Playlist::Media(media)) => {
                let reload = self.media_url.as_ref() == Some(&url);
                if !reload {
                    self.next_segment_index = 0;
                    self.next_part_index = 0;
                    self.last_msn = None;
                    self.last_part_msn = None;
                    self.last_part_index = None;
                } else {
                    let (seg, part) = resume_position(
                        &media,
                        self.last_msn,
                        self.last_part_msn,
                        self.last_part_index,
                    );
                    self.next_segment_index = seg;
                    self.next_part_index = part;
                }
                self.media = Some(media);
                self.media_url = Some(url);
                let mut actions = if reload {
                    Vec::new()
                } else {
                    self.bump_epoch()
                };
                actions.extend(self.schedule_downloads());
                actions
            }
            Err(e) => self.stop_with_error(e),
        }
    }

    fn on_resource_loaded(&mut self, uri: String, epoch: u64) -> Vec<HlsAction> {
        let kind = self.inflight.remove(&epoch);
        if let Some(ref k) = kind {
            match k {
                ActionKind::LoadSegment { uri: u, .. } => {
                    if let Some((idx, seg)) = find_segment(&self.media, u) {
                        if Some(seg.media_sequence) >= self.last_msn {
                            self.last_msn = Some(seg.media_sequence);
                        }
                        self.last_part_msn = None;
                        self.last_part_index = None;
                        // If this segment is the one currently being walked, advance past it.
                        if idx == self.next_segment_index {
                            self.next_segment_index = idx.saturating_add(1);
                            self.next_part_index = 0;
                        }
                    }
                }
                ActionKind::LoadPart { uri: u, .. } => {
                    if let Some((seg_idx, part_idx, _part, seg)) = find_part(&self.media, u) {
                        self.last_part_msn = Some(seg.media_sequence);
                        self.last_part_index = Some(part_idx);
                        if seg_idx == self.next_segment_index && part_idx >= self.next_part_index {
                            self.next_part_index = part_idx.saturating_add(1);
                        }
                    }
                }
                _ => {}
            }
        }
        if kind.is_some() {
            self.retry_counts.remove(&uri);
        }
        self.schedule_downloads()
    }

    fn on_request_failed(&mut self, uri: String, retryable: bool, epoch: u64) -> Vec<HlsAction> {
        let kind = self.inflight.remove(&epoch);
        if kind.is_none() {
            // Stale failure for a request we no longer track.
            return Vec::new();
        }
        if !retryable {
            return self.stop_with_error(HlsError::Unsupported {
                feature: alloc::format!("non-retryable request failed: {}", uri),
            });
        }
        if let Some(k) = kind {
            let count = self.retry_counts.entry(uri.clone()).or_insert(0);
            if *count >= self.config.max_retry_count {
                return self.stop_with_error(HlsError::Unsupported {
                    feature: alloc::format!("retries exhausted for {}", uri),
                });
            }
            *count = count.saturating_add(1);
            self.request_epoch(k)
        } else {
            // Stale failure for a request we no longer track.
            Vec::new()
        }
    }

    fn on_tick(&mut self, now_ms: u64) -> Vec<HlsAction> {
        let mut actions = Vec::new();
        if self.media.is_some() && !self.media.as_ref().unwrap().is_vod() {
            // Do not pile up multiple playlist reloads.
            if !self.inflight.values().any(|k| {
                matches!(
                    k,
                    ActionKind::LoadPlaylist {
                        is_master: false,
                        ..
                    }
                )
            }) {
                let interval = if let Some(part_target) = self
                    .media
                    .as_ref()
                    .unwrap()
                    .part_inf
                    .as_ref()
                    .map(|p| p.part_target)
                {
                    // LL-HLS: poll at part-target cadence. Reject non-finite
                    // or unrepresentable values to avoid u64::MAX / overflow.
                    let part_target_ms = part_target * 1000.0;
                    if part_target_ms.is_finite()
                        && part_target_ms >= 1.0
                        && part_target_ms <= u64::MAX as f64
                    {
                        part_target_ms as u64
                    } else {
                        self.config.reload_interval_ms
                    }
                } else {
                    self.config.reload_interval_ms
                };
                if self
                    .last_reload_ms
                    .map(|t| now_ms >= t.saturating_add(interval))
                    .unwrap_or(true)
                {
                    self.last_reload_ms = Some(now_ms);
                    if let Some(url) = self.media_url.clone() {
                        let blocking = self.config.block_reload
                            && self
                                .media
                                .as_ref()
                                .unwrap()
                                .server_control
                                .as_ref()
                                .map(|s| s.can_block_reload)
                                .unwrap_or(false);
                        actions.extend(self.request_epoch(ActionKind::LoadPlaylist {
                            url,
                            is_master: false,
                            blocking,
                        }));
                    }
                }
            }
        }
        actions
    }

    fn on_seek(&mut self, time_ms: u64) -> Vec<HlsAction> {
        let media = match self.media.as_ref() {
            Some(m) => m,
            None => {
                return self.stop_with_error(HlsError::Unsupported {
                    feature: "seek before media playlist loaded".into(),
                });
            }
        };
        if !media.is_vod() {
            return self.stop_with_error(HlsError::Unsupported {
                feature: "seek only supported for VOD playlists".into(),
            });
        }
        let time_s = (time_ms as f64) / 1000.0;
        if time_s < 0.0 || time_s > media.duration {
            return self.stop_with_error(HlsError::SeekOutOfRange);
        }
        let idx = find_segment_index_by_time(media, time_s);
        let mut actions = self.bump_epoch();
        self.next_segment_index = idx;
        self.next_part_index = 0;
        self.last_msn = None;
        self.last_part_msn = None;
        self.last_part_index = None;
        actions.extend(self.schedule_downloads());
        actions
    }

    fn schedule_downloads(&mut self) -> Vec<HlsAction> {
        let mut actions = Vec::new();
        let media = match self.media.as_ref() {
            Some(m) => m,
            None => return actions,
        };

        while self.next_segment_index < media.segments.len() {
            let seg = &media.segments[self.next_segment_index];

            // Download any independent parts first, one at a time.
            while self.next_part_index < seg.parts.len() {
                let part = &seg.parts[self.next_part_index];
                self.next_part_index += 1;
                if !part.independent || part.gap {
                    // For LL-HLS only preload independent parts as usable decode points.
                    // GAP parts are placeholders for unavailable content and must be skipped.
                    continue;
                }
                let uri = part.uri.clone();
                if self.is_inflight(&uri) {
                    continue;
                }
                let byte_range = part.byte_range;
                actions.extend(self.request_epoch(ActionKind::LoadPart { uri, byte_range }));
                return actions; // one at a time
            }

            let uri = seg.uri.clone();
            if self.is_inflight(&uri) {
                // Already downloading this segment; advance and continue checking.
                self.next_segment_index += 1;
                self.next_part_index = 0;
                continue;
            }
            let byte_range = seg.byte_range;
            let discontinuity = seg.discontinuity;
            self.next_segment_index += 1;
            self.next_part_index = 0;
            actions.extend(self.request_epoch(ActionKind::LoadSegment {
                uri,
                byte_range,
                discontinuity,
            }));
            return actions; // one request per scheduling call
        }

        actions
    }

    fn is_inflight(&self, uri: &str) -> bool {
        self.inflight.values().any(|k| action_uri(k) == Some(uri))
    }

    fn remove_inflight_playlist(&mut self, url: &str, is_master: bool) {
        self.inflight.retain(|_, k| {
            if let ActionKind::LoadPlaylist {
                url: u,
                is_master: m,
                ..
            } = k
            {
                u != url || *m != is_master
            } else {
                true
            }
        });
    }

    fn request_epoch(&mut self, kind: ActionKind) -> Vec<HlsAction> {
        self.epoch = self.epoch.saturating_add(1);
        let action = HlsAction {
            epoch: self.epoch,
            kind: kind.clone(),
        };
        self.inflight.insert(self.epoch, kind);
        vec![action]
    }

    fn bump_epoch(&mut self) -> Vec<HlsAction> {
        self.epoch = self.epoch.saturating_add(1);
        let mut actions = Vec::new();
        self.inflight.retain(|&e, _| {
            if e < self.epoch {
                actions.push(HlsAction {
                    epoch: e,
                    kind: ActionKind::CancelEpoch { epoch: e },
                });
                false
            } else {
                true
            }
        });
        self.retry_counts.clear();
        actions
    }

    fn stop_with_error(&mut self, error: HlsError) -> Vec<HlsAction> {
        self.stopped = true;
        self.inflight.clear();
        vec![HlsAction {
            epoch: 0,
            kind: ActionKind::ReportError { error },
        }]
    }
}

fn action_uri(kind: &ActionKind) -> Option<&str> {
    match kind {
        ActionKind::LoadPlaylist { url, .. } => Some(url.as_str()),
        ActionKind::LoadSegment { uri, .. } => Some(uri.as_str()),
        ActionKind::LoadPart { uri, .. } => Some(uri.as_str()),
        _ => None,
    }
}

fn find_segment_index_by_time(media: &MediaPlaylist, time_s: f64) -> usize {
    if time_s <= 0.0 || media.segments.is_empty() {
        return 0;
    }
    let mut acc = 0.0;
    for (idx, seg) in media.segments.iter().enumerate() {
        acc += seg.duration;
        if acc > time_s {
            return idx;
        }
    }
    // At or beyond the last boundary: start from the final segment.
    media.segments.len().saturating_sub(1)
}

fn find_segment<'a>(media: &'a Option<MediaPlaylist>, uri: &str) -> Option<(usize, &'a Segment)> {
    let media = media.as_ref()?;
    media
        .segments
        .iter()
        .enumerate()
        .find(|(_, s)| s.uri == uri)
}

fn find_part<'a>(
    media: &'a Option<MediaPlaylist>,
    uri: &str,
) -> Option<(usize, usize, &'a Part, &'a Segment)> {
    let media = media.as_ref()?;
    for (seg_idx, seg) in media.segments.iter().enumerate() {
        for (part_idx, part) in seg.parts.iter().enumerate() {
            if part.uri == uri {
                return Some((seg_idx, part_idx, part, seg));
            }
        }
    }
    None
}

fn resume_position(
    media: &MediaPlaylist,
    last_msn: Option<u64>,
    last_part_msn: Option<u64>,
    last_part_index: Option<usize>,
) -> (usize, usize) {
    let part_seg_idx = last_part_msn.and_then(|part_msn| {
        media
            .segments
            .iter()
            .position(|s| s.media_sequence == part_msn)
    });
    if let Some(seg_idx) = part_seg_idx {
        let next_part = last_part_index.map(|i| i + 1).unwrap_or(0);
        if next_part < media.segments[seg_idx].parts.len() {
            return (seg_idx, next_part);
        } else if seg_idx + 1 < media.segments.len() {
            return (seg_idx + 1, 0);
        } else {
            return (media.segments.len(), 0);
        }
    }
    if let Some(msn) = last_msn {
        if let Some(idx) = media.segments.iter().position(|s| s.media_sequence > msn) {
            return (idx, 0);
        }
        return (media.segments.len(), 0);
    }
    (0, 0)
}
