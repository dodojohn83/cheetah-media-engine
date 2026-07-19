//! Incremental ISOBMFF / fMP4 demuxer.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use cheetah_media_types::{MediaPacket, StreamEpoch, TrackId, TrackInfo};

use crate::Mp4Error;
use crate::boxes::{BoxHeader, types};
use crate::fragment::{TrackFragment, emit_packets, parse_moof};
use crate::moov::{TrackData, parse_moov};

/// Maximum buffered bytes before the ISOBMFF demuxer rejects further input.
const MAX_BUFFER: usize = 64 * 1024 * 1024;

/// Events emitted by the demuxer.
#[derive(Debug, Clone)]
pub enum Mp4Event {
    /// A track declaration from the init segment.
    Track(TrackInfo),
    /// A compressed sample from a media fragment.
    Packet(MediaPacket<'static>),
    /// A stream discontinuity for a track.
    Discontinuity { track_id: u32 },
}

/// Incremental demuxer for fragmented MP4.
#[derive(Debug)]
pub struct IsobmffDemuxer {
    buffer: Vec<u8>,
    consumed: u64,
    pending_events: VecDeque<Mp4Event>,
    pending_fragments: Vec<TrackFragment>,
    tracks: BTreeMap<u32, TrackData>,
    epoch: StreamEpoch,
    sequence: u64,
    moov_seen: bool,
    last_dts: BTreeMap<u32, u64>,
    error: Option<Mp4Error>,
}

impl IsobmffDemuxer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            consumed: 0,
            pending_events: VecDeque::new(),
            pending_fragments: Vec::new(),
            tracks: BTreeMap::new(),
            epoch: StreamEpoch::new(0),
            sequence: 0,
            moov_seen: false,
            last_dts: BTreeMap::new(),
            error: None,
        }
    }

    /// Feed more bytes into the demuxer.
    pub fn push(&mut self, data: &[u8]) {
        if data.is_empty() || self.error.is_some() {
            return;
        }
        if self.buffer.len().saturating_add(data.len()) > MAX_BUFFER {
            self.error = Some(Mp4Error::LimitExceeded {
                limit: "demuxer buffer",
            });
            return;
        }
        self.buffer.extend_from_slice(data);
    }

    /// Return the next event, or `None` if more data is needed.
    pub fn next_event(&mut self) -> Result<Option<Mp4Event>, Mp4Error> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(Some(event));
        }

        if let Some(ref err) = self.error {
            return Err(err.clone());
        }

        loop {
            if self.buffer.len() > MAX_BUFFER {
                return Err(Mp4Error::LimitExceeded {
                    limit: "demuxer buffer",
                });
            }

            let header = match BoxHeader::parse(&self.buffer, self.consumed) {
                Ok(h) => h,
                Err(Mp4Error::NeedMoreData) => return Ok(None),
                Err(e) => return Err(e),
            };

            let box_end = usize::try_from(header.size)
                .map_err(|_| Mp4Error::LimitExceeded { limit: "box size" })?;
            if self.buffer.len() < box_end {
                return Ok(None);
            }

            let box_type = header.box_type;
            let start_offset = header.start_offset;
            let body_offset = header.body_offset();

            // Process the box in a scoped block so the `body` borrow ends before we drain.
            let consumed = {
                let body = &self.buffer[header.header_size as usize..box_end];
                match box_type {
                    types::FTYP | types::FREE | types::SKIP | types::UUID => {}
                    types::MOOV => {
                        let (tracks, _movie_timescale) = parse_moov(body, body_offset)?;
                        self.apply_moov(tracks)?;
                    }
                    types::MOOF => {
                        let track_list: Vec<TrackData> = self.tracks.values().cloned().collect();
                        let fragments = parse_moof(body, start_offset, &track_list)?;
                        self.pending_fragments = fragments;
                    }
                    types::MDAT => {
                        if !self.pending_fragments.is_empty() {
                            let mdat = body.to_vec();
                            self.emit_mdat_packets(mdat, body_offset)?;
                            self.pending_fragments.clear();
                        }
                    }
                    _ => {}
                }
                box_end
            };

            self.consumed += consumed as u64;
            self.buffer.drain(0..consumed);

            if let Some(event) = self.pending_events.pop_front() {
                return Ok(Some(event));
            }
        }
    }

    fn apply_moov(&mut self, tracks: BTreeMap<u32, TrackData>) -> Result<(), Mp4Error> {
        let mut new_tracks = tracks;
        for (id, td) in &mut new_tracks {
            if let Some(track_id) = TrackId::new(*id) {
                td.track.id = track_id;
            } else {
                return Err(Mp4Error::invalid_input(
                    3401,
                    Some("invalid track id 0 in moov"),
                ));
            }
        }

        if self.moov_seen {
            // A new init segment indicates a generation change.
            self.epoch = self.epoch.next();
            self.last_dts.clear();
        }
        self.moov_seen = true;

        for (id, td) in new_tracks {
            let event = Mp4Event::Track(td.track.clone());
            self.tracks.insert(id, td);
            self.pending_events.push_back(event);
        }
        Ok(())
    }

    fn emit_mdat_packets(
        &mut self,
        mdat_body: Vec<u8>,
        mdat_data_offset: u64,
    ) -> Result<(), Mp4Error> {
        use cheetah_media_types::BufferRef;
        let mdat_buf = BufferRef::from_owned(mdat_body);
        let mut fragments = Vec::new();
        core::mem::swap(&mut fragments, &mut self.pending_fragments);
        for tf in fragments {
            let track = match self.tracks.get(&tf.track_id) {
                Some(t) => t,
                None => continue,
            };

            // Detect a backwards tfdt as a discontinuity and bump the epoch.
            let mut epoch = self.epoch;
            let mut discontinuity = false;
            let total_duration = tf
                .samples
                .iter()
                .map(|s| s.duration)
                .try_fold(0u64, |acc, d| acc.checked_add(d));
            let (fragment_end, overflow) = match total_duration {
                Some(sum) => tf.base_decode_time.overflowing_add(sum),
                None => (u64::MAX, true),
            };
            if let Some(prev) = self.last_dts.get(&tf.track_id)
                && (overflow
                    || fragment_end < *prev
                    || tf.base_decode_time < prev.saturating_sub(10_000_000))
            {
                epoch = epoch.next();
                self.epoch = epoch;
                discontinuity = true;
                self.pending_events.push_back(Mp4Event::Discontinuity {
                    track_id: tf.track_id,
                });
            }

            let packets = emit_packets(
                &tf,
                &mdat_buf,
                mdat_data_offset,
                track,
                &mut self.sequence,
                epoch,
            )?;

            if let Some(last) = packets.last()
                && let Some(dts) = last.time.dts
            {
                // DTS is signed internally; clamp negative values to 0 before
                // storing in last_dts so a negative pre-roll timestamp does
                // not wrap to u64::MAX and trigger a false discontinuity.
                self.last_dts
                    .insert(tf.track_id, u64::try_from(dts.ticks()).unwrap_or(0));
            }

            let mut first = true;
            for mut pkt in packets {
                if first && discontinuity {
                    pkt.flags.is_discontinuity = true;
                    first = false;
                }
                self.pending_events.push_back(Mp4Event::Packet(pkt));
            }
        }
        Ok(())
    }
}

impl Default for IsobmffDemuxer {
    fn default() -> Self {
        Self::new()
    }
}
