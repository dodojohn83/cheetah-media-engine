//! MPEG-TS transport stream parser.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

use cheetah_media_bitstream::ByteCursor;

/// Error returned by the MPEG-TS parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsError {
    WrongSyncByte,
    PacketTooShort,
}

/// A 188-byte transport stream packet header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TsPacket {
    pub pid: u16,
    pub payload_unit_start: bool,
    pub adaptation_field_control: u8,
    pub continuity_counter: u8,
    pub transport_error: bool,
}

/// Parse the 4-byte header of a 188-byte TS packet.
///
/// The `pid` is masked to 13 bits as per ISO/IEC 13818-1.
pub fn parse_packet_header(input: &[u8]) -> Result<TsPacket, TsError> {
    if input.len() < 188 {
        return Err(TsError::PacketTooShort);
    }
    let mut cursor = ByteCursor::new(input);
    let sync = cursor.read_u8().map_err(|_| TsError::PacketTooShort)?;
    if sync != 0x47 {
        return Err(TsError::WrongSyncByte);
    }
    let b0 = cursor.read_u8().map_err(|_| TsError::PacketTooShort)?;
    let b1 = cursor.read_u8().map_err(|_| TsError::PacketTooShort)?;
    let b2 = cursor.read_u8().map_err(|_| TsError::PacketTooShort)?;

    let pid = u16::from(b0 & 0x1F) << 8 | u16::from(b1);
    Ok(TsPacket {
        pid,
        payload_unit_start: (b0 & 0x40) != 0,
        adaptation_field_control: (b2 >> 4) & 0x03,
        continuity_counter: b2 & 0x0F,
        transport_error: (b0 & 0x80) != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet_with_pid(pid: u16) -> [u8; 188] {
        let mut pkt = [0u8; 188];
        pkt[0] = 0x47;
        pkt[1] = ((pid >> 8) as u8) & 0x1F;
        pkt[2] = (pid & 0xFF) as u8;
        pkt[3] = 0x00;
        pkt
    }

    #[test]
    fn parse_pid_zero() {
        let pkt = packet_with_pid(0);
        let p = parse_packet_header(&pkt).unwrap();
        assert_eq!(p.pid, 0);
        assert!(!p.payload_unit_start);
    }

    #[test]
    fn parse_payload_unit_start() {
        let mut pkt = packet_with_pid(0x100);
        pkt[1] |= 0x40;
        let p = parse_packet_header(&pkt).unwrap();
        assert_eq!(p.pid, 0x100);
        assert!(p.payload_unit_start);
    }

    #[test]
    fn parse_short_packet_fails() {
        assert_eq!(parse_packet_header(&[0x47]), Err(TsError::PacketTooShort));
    }

    #[test]
    fn parse_wrong_sync_fails() {
        let mut pkt = packet_with_pid(0);
        pkt[0] = 0x48;
        assert_eq!(parse_packet_header(&pkt), Err(TsError::WrongSyncByte));
    }
}
