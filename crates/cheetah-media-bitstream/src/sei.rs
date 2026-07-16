//! H.264/H.265 Supplemental Enhancement Information (SEI) message parsing.
//!
//! SEI payloads live inside the RBSP of a NAL unit. Before using this module,
//! callers should remove emulation prevention three-bytes with
//! [`crate::rbsp::unescape_rbsp`].

extern crate alloc;

use alloc::vec::Vec;

/// Errors that can occur while parsing SEI messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeiError {
    /// Reached the end of the RBSP while reading a message header/payload.
    Truncated,
    /// Encountered a malformed message (e.g. invalid payload-size encoding).
    InvalidMessage,
    /// Too many SEI messages in a single NAL.
    TooManyMessages,
    /// A single SEI payload exceeded the configured maximum.
    PayloadTooLarge,
}

impl core::fmt::Display for SeiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => write!(f, "SEI message truncated"),
            Self::InvalidMessage => write!(f, "SEI message invalid"),
            Self::TooManyMessages => write!(f, "too many SEI messages"),
            Self::PayloadTooLarge => write!(f, "SEI payload too large"),
        }
    }
}

/// A single SEI message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeiMessage {
    /// SEI `payloadType`.
    pub payload_type: u32,
    /// Raw SEI `payload` bytes.
    pub payload: Vec<u8>,
}

/// Parse all SEI messages in an unescaped SEI RBSP.
///
/// The RBSP may optionally end with a `rbsp_trailing_bits` byte. Trailing
/// zero bits and the stop bit are ignored; empty messages (zero-byte payloads)
/// are preserved when valid.
///
/// `max_messages` and `max_payload_len` bound resource use for malformed streams.
pub fn parse_sei_messages(
    rbsp: &[u8],
    max_messages: usize,
    max_payload_len: usize,
) -> Result<Vec<SeiMessage>, SeiError> {
    let mut messages = Vec::new();
    let mut i = 0;

    while i < rbsp.len() {
        // An SEI RBSP ends with rbsp_trailing_bits: a single 1 bit followed by
        // zero bits. When the payload is byte-aligned, the trailing bits byte is
        // 0x80; optional zero padding bytes may follow. If the remainder is
        // such a pattern, we are done.
        if rbsp[i] == 0x80 && rbsp[i + 1..].iter().all(|&b| b == 0) {
            break;
        }

        if messages.len() >= max_messages {
            return Err(SeiError::TooManyMessages);
        }

        let mut payload_type = 0u32;
        let mut payload_size = 0usize;

        // payload_type uses 0xFF as a continuation byte.
        let mut seen_value = false;
        while i < rbsp.len() {
            let b = rbsp[i];
            i += 1;
            if b == 0xff {
                payload_type = payload_type
                    .checked_add(255)
                    .ok_or(SeiError::InvalidMessage)?;
            } else {
                payload_type = payload_type
                    .checked_add(u32::from(b))
                    .ok_or(SeiError::InvalidMessage)?;
                seen_value = true;
                break;
            }
        }
        if !seen_value {
            // Reached end with only continuation bytes.
            return Err(SeiError::Truncated);
        }

        // payload_size uses the same continuation scheme.
        seen_value = false;
        while i < rbsp.len() {
            let b = rbsp[i];
            i += 1;
            if b == 0xff {
                payload_size = payload_size
                    .checked_add(255)
                    .ok_or(SeiError::InvalidMessage)?;
            } else {
                let add = usize::from(b);
                payload_size = payload_size
                    .checked_add(add)
                    .ok_or(SeiError::InvalidMessage)?;
                seen_value = true;
                break;
            }
        }
        if !seen_value {
            return Err(SeiError::Truncated);
        }

        if payload_size > max_payload_len {
            return Err(SeiError::PayloadTooLarge);
        }

        if i + payload_size > rbsp.len() {
            // Truncated; but if the only missing bytes are trailing zero bits,
            // we tolerate a shorter final payload. The stop bit in rbsp_trailing_bits
            // means there is at least one non-zero bit after the payload, but malformed
            // encoders sometimes omit it. We treat missing payload as an error unless
            // the remainder is all zeros (padding after a stop bit).
            let available = rbsp.len() - i;
            if rbsp[i..].iter().all(|&b| b == 0) && available <= payload_size {
                messages.push(SeiMessage {
                    payload_type,
                    payload: rbsp[i..].to_vec(),
                });
                break;
            }
            return Err(SeiError::Truncated);
        }

        let payload = rbsp[i..i + payload_size].to_vec();
        i += payload_size;
        messages.push(SeiMessage {
            payload_type,
            payload,
        });
    }

    Ok(messages)
}

/// Parse SEI messages using sensible stream defaults.
///
/// Defaults: 256 messages max, 64 KiB per payload.
pub fn parse_sei(rbsp: &[u8]) -> Result<Vec<SeiMessage>, SeiError> {
    parse_sei_messages(rbsp, 256, 64 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rbsp_yields_empty_messages() {
        let out = parse_sei(b"").unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn single_small_message() {
        // payload_type = 4, payload_size = 5, payload = b"hello"
        let rbsp = [0x04, 0x05, b'h', b'e', b'l', b'l', b'o'];
        let out = parse_sei(&rbsp).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].payload_type, 4);
        assert_eq!(out[0].payload, b"hello");
    }

    #[test]
    fn continuation_bytes_for_type_and_size() {
        // payload_type = 255 + 1 = 256, payload_size = 255 + 2 = 257
        let mut rbsp = vec![0xff, 0x01, 0xff, 0x02];
        rbsp.resize(rbsp.len() + 257, 0xab);

        let out = parse_sei(&rbsp).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].payload_type, 256);
        assert_eq!(out[0].payload.len(), 257);
        assert!(out[0].payload.iter().all(|&b| b == 0xab));
    }

    #[test]
    fn multiple_messages() {
        // message 1: type 1, size 2, payload "ab"
        // message 2: type 2, size 0
        let rbsp = [0x01, 0x02, b'a', b'b', 0x02, 0x00];
        let out = parse_sei(&rbsp).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].payload_type, 1);
        assert_eq!(out[0].payload, b"ab");
        assert_eq!(out[1].payload_type, 2);
        assert!(out[1].payload.is_empty());
    }

    #[test]
    fn rbsp_trailing_bits_stop_byte_is_accepted() {
        // A standards-compliant SEI NAL RBSP ends with rbsp_trailing_bits,
        // which for byte-aligned payload is 0x80 (single stop bit + zeros).
        let mut rbsp = vec![0x04, 0x05, b'h', b'e', b'l', b'l', b'o'];
        rbsp.push(0x80);
        let out = parse_sei(&rbsp).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].payload_type, 4);
        assert_eq!(out[0].payload, b"hello");
    }

    #[test]
    fn rbsp_trailing_bits_with_zero_padding_is_accepted() {
        let mut rbsp = vec![0x04, 0x05, b'h', b'e', b'l', b'l', b'o', 0x80];
        rbsp.extend_from_slice(&[0x00, 0x00]);
        let out = parse_sei(&rbsp).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].payload_type, 4);
        assert_eq!(out[0].payload, b"hello");
    }

    #[test]
    fn truncated_type_continuation_is_error() {
        let rbsp = [0xff];
        assert!(matches!(parse_sei(&rbsp), Err(SeiError::Truncated)));
    }

    #[test]
    fn truncated_size_continuation_is_error() {
        let rbsp = [0x04, 0xff];
        assert!(matches!(parse_sei(&rbsp), Err(SeiError::Truncated)));
    }

    #[test]
    fn payload_past_end_is_error() {
        let rbsp = [0x04, 0x05, b'h', b'e'];
        assert!(matches!(parse_sei(&rbsp), Err(SeiError::Truncated)));
    }

    #[test]
    fn max_messages_limit() {
        // Three tiny messages, but limit set to 2.
        let rbsp = [0x01, 0x01, b'a', 0x02, 0x01, b'b', 0x03, 0x01, b'c'];
        assert!(matches!(
            parse_sei_messages(&rbsp, 2, 1024),
            Err(SeiError::TooManyMessages)
        ));
    }

    #[test]
    fn max_payload_limit() {
        let rbsp = [0x04, 0x05, b'a', b'a', b'a', b'a', b'a'];
        assert!(matches!(
            parse_sei_messages(&rbsp, 256, 4),
            Err(SeiError::PayloadTooLarge)
        ));
    }
}
