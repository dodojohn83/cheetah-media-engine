//! PES packet assembly and PTS/DTS extraction.

use alloc::vec::Vec;

use cheetah_media_bitstream::ByteCursor;
use cheetah_media_types::{MediaTime, TimeBase, Timestamp};

use crate::TsError;

/// Maximum PES packet size we accept.
pub const MAX_PES_SIZE: usize = 4 * 1024 * 1024;

/// A parsed PES header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PesHeader {
    pub stream_id: u8,
    pub packet_length: u16,
    pub pts: Option<Timestamp>,
    pub dts: Option<Timestamp>,
    pub header_size: usize,
}

/// Assembled PES payload with its header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PesOutput {
    pub header: PesHeader,
    pub payload: Vec<u8>,
}

/// State machine for assembling PES packets from TS payloads.
#[derive(Debug, Default)]
pub struct PesAssembler {
    raw: Vec<u8>,
    expected_length: Option<usize>,
    header_parsed: bool,
    header: Option<PesHeader>,
}

impl PesAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed payload bytes. `payload_unit_start` true means this is the beginning of a PES packet.
    ///
    /// Returns all PES packets completed by this feed.
    pub fn feed(
        &mut self,
        payload: &[u8],
        payload_unit_start: bool,
    ) -> Result<Vec<PesOutput>, TsError> {
        let mut outputs = Vec::new();

        if payload_unit_start {
            // Finalize any previous PES before starting a new one.
            self.flush_into(&mut outputs);
            self.reset();

            if payload.len() < 3 || payload[0] != 0x00 || payload[1] != 0x00 || payload[2] != 0x01 {
                if payload.len() < 3 {
                    self.raw.extend_from_slice(payload);
                    return Ok(outputs);
                }
                return Err(TsError::invalid_input(2101, Some("PES missing start code")));
            }
            self.raw.extend_from_slice(payload);
            self.try_complete_into(&mut outputs)?;
            return Ok(outputs);
        }

        if self.raw.is_empty() && !self.header_parsed {
            // No active PES; ignore.
            return Ok(outputs);
        }

        if self.raw.len() + payload.len() > MAX_PES_SIZE {
            self.reset();
            return Err(TsError::LimitExceeded {
                limit: "pes buffer",
            });
        }

        self.raw.extend_from_slice(payload);
        self.try_complete_into(&mut outputs)?;
        Ok(outputs)
    }

    fn reset(&mut self) {
        self.raw.clear();
        self.expected_length = None;
        self.header_parsed = false;
        self.header = None;
    }

    fn flush_into(&mut self, outputs: &mut Vec<PesOutput>) {
        if self.header_parsed
            && !self.raw.is_empty()
            && let Some(header) = self.header
        {
            let end = self
                .expected_length
                .unwrap_or(self.raw.len())
                .min(self.raw.len());
            let payload = self.raw[header.header_size..end].to_vec();
            outputs.push(PesOutput { header, payload });
        }
        self.reset();
    }

    fn try_complete_into(&mut self, outputs: &mut Vec<PesOutput>) -> Result<(), TsError> {
        if !self.header_parsed {
            if self.raw.len() < 9 {
                return Ok(());
            }
            let header = parse_pes_header(&self.raw)?;
            self.header = Some(header);
            self.header_parsed = true;
            if header.packet_length != 0 {
                self.expected_length = Some((header.packet_length as usize) + 6);
            }
            if let Some(expected) = self.expected_length
                && self.raw.len() >= expected
            {
                let payload = self.raw[header.header_size..expected].to_vec();
                outputs.push(PesOutput { header, payload });
                self.reset();
            }
            return Ok(());
        }

        if let Some(expected) = self.expected_length
            && self.raw.len() >= expected
        {
            let header = self.header.ok_or_else(|| {
                TsError::invalid_input(2106, Some("PES header missing after parse"))
            })?;
            let payload = self.raw[header.header_size..expected].to_vec();
            outputs.push(PesOutput { header, payload });
            self.reset();
        }
        Ok(())
    }
}

/// Parse the PES header from `data` and return header size and timestamps.
pub fn parse_pes_header(data: &[u8]) -> Result<PesHeader, TsError> {
    if data.len() < 9 {
        return Err(TsError::NeedMoreData);
    }
    if data[0] != 0x00 || data[1] != 0x00 || data[2] != 0x01 {
        return Err(TsError::invalid_input(2103, Some("bad PES start code")));
    }
    let stream_id = data[3];
    let packet_length = u16::from_be_bytes([data[4], data[5]]);
    let mut cursor = ByteCursor::new(&data[6..]);
    let flags1 = cursor.read_u8().map_err(|_| TsError::NeedMoreData)?;
    let marker = (flags1 >> 6) & 0x03;
    if marker != 0x02 {
        return Err(TsError::invalid_input(
            2104,
            Some("PES marker bits invalid"),
        ));
    }
    let flags2 = cursor.read_u8().map_err(|_| TsError::NeedMoreData)?;
    let pts_dts_flags = (flags2 >> 6) & 0x03;
    let header_data_length = cursor.read_u8().map_err(|_| TsError::NeedMoreData)? as usize;
    let header_size = 9 + header_data_length;
    if data.len() < header_size {
        return Err(TsError::NeedMoreData);
    }

    let mut pts = None;
    let mut dts = None;
    let mut cursor2 = ByteCursor::new(&data[9..header_size]);
    if pts_dts_flags & 0x02 != 0 {
        let buf = cursor2.read_bytes(5).map_err(|_| TsError::NeedMoreData)?;
        pts = Some(parse_timestamp(buf));
    }
    if pts_dts_flags == 0x03 {
        let buf = cursor2.read_bytes(5).map_err(|_| TsError::NeedMoreData)?;
        dts = Some(parse_timestamp(buf));
    }

    Ok(PesHeader {
        stream_id,
        packet_length,
        pts,
        dts,
        header_size,
    })
}

/// Build a `MediaTime` from optional 90 kHz PTS/DTS timestamps.
pub fn media_time_from_pes(pts: Option<Timestamp>, dts: Option<Timestamp>) -> MediaTime {
    MediaTime::new(pts, dts, None, TimeBase::TS_90K)
}

/// Parse a 33-bit MPEG-1/2 timestamp from a 5-byte buffer.
fn parse_timestamp(buf: &[u8]) -> Timestamp {
    let high = (u64::from(buf[0]) >> 1) & 0x07;
    let mid = (u64::from(buf[1]) << 8 | u64::from(buf[2])) >> 1;
    let low = (u64::from(buf[3]) << 8 | u64::from(buf[4])) >> 1;
    let ticks = (high << 30) | (mid << 15) | low;
    Timestamp::new(ticks as i64)
}

/// True if `stream_id` indicates a video stream.
pub const fn is_video_stream(stream_id: u8) -> bool {
    stream_id >= 0xE0 && stream_id <= 0xEF
}

/// True if `stream_id` indicates an audio stream.
pub const fn is_audio_stream(stream_id: u8) -> bool {
    (stream_id >= 0xC0 && stream_id <= 0xDF) || stream_id == 0xBD
}
