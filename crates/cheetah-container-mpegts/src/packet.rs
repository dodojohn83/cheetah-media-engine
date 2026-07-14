//! MPEG-TS packet header parsing.

use cheetah_media_bitstream::ByteCursor;

use crate::TsError;

/// Transport stream packet header and adaptation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TsPacket {
    pub pid: u16,
    pub payload_unit_start: bool,
    pub adaptation_field_control: u8,
    pub continuity_counter: u8,
    pub transport_error: bool,
    pub discontinuity: bool,
    pub has_pcr: bool,
    pub pcr: Option<u64>,
    pub has_random_access_indicator: bool,
}

impl Default for TsPacket {
    fn default() -> Self {
        Self {
            pid: 0x1fff,
            payload_unit_start: false,
            adaptation_field_control: 1,
            continuity_counter: 0,
            transport_error: false,
            discontinuity: false,
            has_pcr: false,
            pcr: None,
            has_random_access_indicator: false,
        }
    }
}

impl TsPacket {
    /// Parse the 4-byte TS header and adaptation field from `data`.
    ///
    /// `data` must be at least 188 bytes.
    pub fn parse(data: &[u8]) -> Result<Self, TsError> {
        if data.len() < 188 {
            return Err(TsError::PacketTooShort);
        }
        let mut cursor = ByteCursor::new(data);
        let sync = cursor.read_u8().map_err(|_| TsError::PacketTooShort)?;
        if sync != 0x47 {
            return Err(TsError::LostSync);
        }
        let b0 = cursor.read_u8().map_err(|_| TsError::PacketTooShort)?;
        let b1 = cursor.read_u8().map_err(|_| TsError::PacketTooShort)?;
        let b2 = cursor.read_u8().map_err(|_| TsError::PacketTooShort)?;

        let pid = (u16::from(b0 & 0x1F) << 8) | u16::from(b1);
        let payload_unit_start = (b0 & 0x40) != 0;
        let transport_error = (b0 & 0x80) != 0;
        let adaptation_field_control = (b2 >> 4) & 0x03;
        let continuity_counter = b2 & 0x0F;

        let mut has_pcr = false;
        let mut pcr = None;
        let mut has_random_access_indicator = false;
        let mut discontinuity = false;

        if matches!(adaptation_field_control, 2 | 3) {
            // Adaptation field starts at byte 4.
            let adapt_len = cursor.read_u8().map_err(|_| TsError::PacketTooShort)? as usize;
            if adapt_len > 0 {
                let flags = cursor.read_u8().map_err(|_| TsError::PacketTooShort)?;
                discontinuity = (flags & 0x80) != 0;
                has_random_access_indicator = (flags & 0x40) != 0;
                let pcr_flag = (flags & 0x10) != 0;
                if pcr_flag && adapt_len >= 7 {
                    let pcr_bytes = cursor.read_bytes(6).map_err(|_| TsError::PacketTooShort)?;
                    let pcr_base = ((u64::from(pcr_bytes[0]) << 25)
                        | (u64::from(pcr_bytes[1]) << 17)
                        | (u64::from(pcr_bytes[2]) << 9)
                        | (u64::from(pcr_bytes[3]) << 1)
                        | (u64::from(pcr_bytes[4]) >> 7))
                        & 0x1FFFFFFFF;
                    let pcr_ext =
                        ((u16::from(pcr_bytes[4] & 0x01) << 8) | u16::from(pcr_bytes[5])) as u64;
                    pcr = Some(pcr_base * 300 + pcr_ext);
                    has_pcr = true;
                }
            }
        }

        Ok(Self {
            pid,
            payload_unit_start,
            adaptation_field_control,
            continuity_counter,
            transport_error,
            discontinuity,
            has_pcr,
            pcr,
            has_random_access_indicator,
        })
    }

    /// Return the slice of payload bytes within `data` (after adaptation field).
    pub fn payload<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        if self.adaptation_field_control == 2 {
            // Adaptation field only; no payload.
            return &[];
        }
        let adapt_len = if self.adaptation_field_control == 3 {
            // byte 4 is adaptation field length; payload starts after it.
            let len = data.get(4).copied().unwrap_or(0) as usize;
            // The length byte itself and the field it counts are present.
            1usize.saturating_add(len)
        } else {
            0
        };
        let start = 4 + adapt_len;
        if start > data.len() {
            return &[];
        }
        &data[start..]
    }
}
