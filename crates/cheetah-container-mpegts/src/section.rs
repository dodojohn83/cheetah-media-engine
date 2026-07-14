//! PSI section assembly and PAT/PMT parsing.

use alloc::vec::Vec;
use cheetah_media_bitstream::ByteCursor;

use crate::TsError;

/// Maximum allowed single section payload length in bytes.
const MAX_SECTION_SIZE: usize = 4096;

/// Assemble MPEG-TS PSI sections from packet payloads.
#[derive(Debug, Default)]
pub struct SectionAssembler {
    buffer: Vec<u8>,
    section_length: Option<usize>,
}

impl SectionAssembler {
    pub const fn new() -> Self {
        Self {
            buffer: Vec::new(),
            section_length: None,
        }
    }

    /// Feed a payload belonging to this PID.
    ///
    /// Returns `Some(complete section data)` when a section has been assembled.
    pub fn feed(
        &mut self,
        payload: &[u8],
        payload_unit_start: bool,
    ) -> Result<Option<Vec<u8>>, TsError> {
        if payload_unit_start {
            // The first byte is the pointer field.
            if payload.is_empty() {
                return Ok(None);
            }
            let pointer = payload[0] as usize;
            // Start of a new section. Reset any stale assembler.
            self.buffer.clear();
            self.section_length = None;

            if pointer + 1 > payload.len() {
                return Err(TsError::invalid_input(
                    2001,
                    Some("PUSI pointer beyond payload"),
                ));
            }
            let section_data = &payload[1 + pointer..];
            return self.append(section_data);
        }

        // Continuation of a section in progress.
        if self.buffer.is_empty() && self.section_length.is_none() {
            // No active section; ignore this payload.
            return Ok(None);
        }
        self.append(payload)
    }

    fn append(&mut self, data: &[u8]) -> Result<Option<Vec<u8>>, TsError> {
        if data.is_empty() {
            return Ok(None);
        }
        self.buffer.extend_from_slice(data);
        if self.buffer.len() > MAX_SECTION_SIZE {
            return Err(TsError::LimitExceeded {
                limit: "section assembler",
            });
        }

        if self.section_length.is_none() && self.buffer.len() >= 3 {
            let len = (((self.buffer[1] as usize) & 0x0F) << 8) | (self.buffer[2] as usize);
            // Length is the number of bytes after the length field, plus 3 for the header prefix.
            self.section_length = Some(len + 3);
        }

        if let Some(len) = self.section_length
            && self.buffer.len() >= len
        {
            let section = self.buffer[..len].to_vec();
            self.buffer.drain(..len);
            self.section_length = None;
            return Ok(Some(section));
        }
        Ok(None)
    }
}

/// A parsed PAT entry mapping program number to PMT PID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatEntry {
    pub program_number: u16,
    pub pmt_pid: u16,
}

/// Parse a PAT section.
pub fn parse_pat(section: &[u8]) -> Result<Vec<PatEntry>, TsError> {
    if section.len() < 8 {
        return Err(TsError::NeedMoreData);
    }
    let table_id = section[0];
    if table_id != 0x00 {
        return Err(TsError::invalid_input(2002, Some("PAT table id mismatch")));
    }
    let section_length = (((section[1] as usize) & 0x0F) << 8) | (section[2] as usize);
    let total = section_length + 3;
    if section.len() < total {
        return Err(TsError::NeedMoreData);
    }
    // section_syntaxIndicator = (section[1] >> 7) & 1; must be 1.
    // Skip CRC32 check for now (last 4 bytes).
    let data = &section[..total];
    if data.len() < 12 {
        return Err(TsError::invalid_input(2003, Some("PAT section too short")));
    }
    let entry_bytes = data.len() - 12; // 8 header + 4 crc
    if !entry_bytes.is_multiple_of(4) {
        return Err(TsError::invalid_input(
            2004,
            Some("PAT entry length misaligned"),
        ));
    }
    let mut entries = Vec::new();
    let mut cursor = ByteCursor::new(&data[8..data.len() - 4]);
    while !cursor.is_empty() {
        let program = cursor
            .read_u16_be()
            .map_err(|_| TsError::invalid_input(2005, Some("PAT entry truncated")))?;
        let pid_word = cursor
            .read_u16_be()
            .map_err(|_| TsError::invalid_input(2005, Some("PAT entry truncated")))?;
        let pmt_pid = pid_word & 0x1FFF;
        if program == 0 {
            // Network PID; ignore for demux.
            continue;
        }
        entries.push(PatEntry {
            program_number: program,
            pmt_pid,
        });
    }
    Ok(entries)
}

/// Stream type and elementary PID from a PMT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PmtStream {
    pub stream_type: u8,
    pub elementary_pid: u16,
}

/// Parse a PMT section, returning the PCR PID and stream list.
pub fn parse_pmt(section: &[u8]) -> Result<(u16, Vec<PmtStream>), TsError> {
    if section.len() < 12 {
        return Err(TsError::NeedMoreData);
    }
    if section[0] != 0x02 {
        return Err(TsError::invalid_input(2006, Some("PMT table id mismatch")));
    }
    let section_length = (((section[1] as usize) & 0x0F) << 8) | (section[2] as usize);
    let total = section_length + 3;
    if section.len() < total {
        return Err(TsError::NeedMoreData);
    }
    let data = &section[..total];
    if data.len() < 16 {
        return Err(TsError::invalid_input(2007, Some("PMT section too short")));
    }
    let pcr_pid = (u16::from(data[8]) & 0x1F) << 8 | u16::from(data[9]);
    let program_info_length = (u16::from(data[10]) & 0x0F) << 8 | u16::from(data[11]);
    let stream_start = 12 + program_info_length as usize;
    if stream_start > data.len() - 4 {
        return Err(TsError::invalid_input(
            2008,
            Some("PMT program info length overflow"),
        ));
    }
    let mut streams = Vec::new();
    let mut cursor = ByteCursor::new(&data[stream_start..data.len() - 4]);
    while cursor.remaining() >= 5 {
        let stream_type = cursor.read_u8().map_err(|_| TsError::NeedMoreData)?;
        let pid_word = cursor.read_u16_be().map_err(|_| TsError::NeedMoreData)?;
        let elementary_pid = pid_word & 0x1FFF;
        let len_word = cursor.read_u16_be().map_err(|_| TsError::NeedMoreData)?;
        let es_info_length = (len_word & 0x0FFF) as usize;
        if es_info_length > 0 {
            // Descriptors are skipped for now.
            cursor
                .skip(es_info_length)
                .map_err(|_| TsError::invalid_input(2009, Some("PMT ES info overflow")))?;
        }
        streams.push(PmtStream {
            stream_type,
            elementary_pid,
        });
    }
    Ok((pcr_pid, streams))
}

/// CRC32 used for PSI sections (MPEG-2 model).
#[allow(dead_code)]
pub fn crc32_mpeg(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &b in data {
        crc ^= u32::from(b) << 24;
        for _ in 0..8 {
            if (crc & 0x80000000) != 0 {
                crc = (crc << 1) ^ 0x04C11DB7;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}
