//! Sans-I/O HLS/LL-HLS client state machine.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::HlsError;
use crate::model::*;
use crate::parser::parse;
use crate::variant::{VariantCapabilities, VariantSelector, select_initial_variant};

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
}

/// Incoming event into the client.
#[derive(Debug, Clone)]
pub enum HlsEvent {
    /// Bytes of a playlist were received.
    PlaylistLoaded { url: String, body: Vec<u8> },
    /// A segment or part download succeeded.
    ResourceLoaded { uri: String, body: Vec<u8> },
    /// A request failed and may be retried.
    RequestFailed { uri: String, retryable: bool },
    /// Drive the live reload clock forward.
    Tick { now_ms: u64 },
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
    pending: BTreeSet<u64>,
    selected_variant: Option<String>,
    next_segment_index: usize,
    next_part_index: usize,
    stopped: bool,
    last_reload_ms: Option<u64>,
    retry_counts: alloc::collections::BTreeMap<String, u32>,
}

impl HlsClient {
    pub fn new(master_url: impl Into<String>, config: HlsConfig) -> Self {
        Self {
            config,
            master_url: master_url.into(),
            master: None,
            media_url: None,
            media: None,
            epoch: 1,
            pending: BTreeSet::new(),
            selected_variant: None,
            next_segment_index: 0,
            next_part_index: 0,
            stopped: false,
            last_reload_ms: None,
            retry_counts: alloc::collections::BTreeMap::new(),
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
            HlsEvent::ResourceLoaded { uri, .. } => self.on_resource_loaded(uri),
            HlsEvent::RequestFailed { uri, retryable } => self.on_request_failed(uri, retryable),
            HlsEvent::Tick { now_ms } => self.on_tick(now_ms),
            HlsEvent::Stop => {
                self.stopped = true;
                self.pending.clear();
                Vec::new()
            }
        }
    }

    fn on_playlist_loaded(&mut self, url: String, body: Vec<u8>) -> Vec<HlsAction> {
        let text = match core::str::from_utf8(&body) {
            Ok(t) => t,
            Err(_) => {
                return vec![error_action(HlsError::Utf8Error)];
            }
        };
        let base = &url;
        match parse(text, base) {
            Ok(Playlist::Master(master)) => {
                self.master = Some(master);
                self.bump_epoch();
                let selected = match select_initial_variant(
                    &self.master.as_ref().unwrap().variants,
                    &self.config.capabilities,
                ) {
                    Ok(v) => v,
                    Err(e) => return vec![error_action(e)],
                };
                self.selected_variant = Some(selected.uri.clone());
                self.request_epoch(ActionKind::LoadPlaylist {
                    url: selected.uri.clone(),
                    is_master: false,
                    blocking: false,
                })
            }
            Ok(Playlist::Media(media)) => {
                self.media = Some(media);
                self.media_url = Some(url);
                self.next_segment_index = 0;
                self.next_part_index = 0;
                self.bump_epoch();
                self.schedule_downloads()
            }
            Err(e) => vec![error_action(e)],
        }
    }

    fn on_resource_loaded(&mut self, uri: String) -> Vec<HlsAction> {
        self.retry_counts.remove(&uri);
        // In a real implementation the payload would be delivered to the caller.
        // For the state machine we simply schedule the next items.
        self.schedule_downloads()
    }

    fn on_request_failed(&mut self, uri: String, retryable: bool) -> Vec<HlsAction> {
        if !retryable {
            self.stopped = true;
            return vec![error_action(HlsError::Unsupported {
                feature: alloc::format!("non-retryable request failed: {}", uri),
            })];
        }
        let count = self.retry_counts.entry(uri.clone()).or_insert(0);
        if *count >= self.config.max_retry_count {
            self.stopped = true;
            return vec![error_action(HlsError::Unsupported {
                feature: alloc::format!("retries exhausted for {}", uri),
            })];
        }
        *count += 1;
        // Re-issue the same request in the next tick.
        self.request_epoch(ActionKind::LoadSegment {
            uri,
            byte_range: None,
            discontinuity: false,
        })
    }

    fn on_tick(&mut self, now_ms: u64) -> Vec<HlsAction> {
        let mut actions = Vec::new();
        if self.media.is_some() && !self.media.as_ref().unwrap().end_list {
            let interval = if let Some(part_target) = self
                .media
                .as_ref()
                .unwrap()
                .part_inf
                .as_ref()
                .map(|p| p.part_target)
            {
                // LL-HLS: poll at part-target cadence.
                (part_target * 1000.0).max(1.0) as u64
            } else {
                self.config.reload_interval_ms
            };
            if self
                .last_reload_ms
                .map(|t| now_ms >= t + interval)
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
        actions
    }

    fn schedule_downloads(&mut self) -> Vec<HlsAction> {
        let mut actions = Vec::new();
        let media = match self.media.as_ref() {
            Some(m) => m,
            None => return actions,
        };

        if self.next_segment_index < media.segments.len() {
            let seg = &media.segments[self.next_segment_index];

            // Download any independent parts first.
            while self.next_part_index < seg.parts.len() {
                let part = &seg.parts[self.next_part_index];
                self.next_part_index += 1;
                if !part.independent && !part.gap {
                    // For LL-HLS only preload independent parts as usable decode points.
                    continue;
                }
                let uri = part.uri.clone();
                let byte_range = part.byte_range;
                actions.extend(self.request_epoch(ActionKind::LoadPart { uri, byte_range }));
                return actions; // one at a time
            }

            let uri = seg.uri.clone();
            let byte_range = seg.byte_range;
            let discontinuity = seg.discontinuity;
            self.next_segment_index += 1;
            self.next_part_index = 0;
            actions.extend(self.request_epoch(ActionKind::LoadSegment {
                uri,
                byte_range,
                discontinuity,
            }));
            return actions; // one segment per scheduling call
        }

        actions
    }

    fn request_epoch(&mut self, kind: ActionKind) -> Vec<HlsAction> {
        let action = HlsAction {
            epoch: self.epoch,
            kind,
        };
        self.pending.insert(self.epoch);
        vec![action]
    }

    fn bump_epoch(&mut self) {
        self.epoch = self.epoch.saturating_add(1);
        self.pending.retain(|&e| e >= self.epoch);
        self.retry_counts.clear();
    }
}

fn error_action(_error: HlsError) -> HlsAction {
    HlsAction {
        epoch: 0,
        kind: ActionKind::Wait { duration_ms: 0 },
    }
}
