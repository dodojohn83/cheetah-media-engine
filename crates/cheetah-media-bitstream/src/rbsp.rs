//! Removal of H.264/H.265 emulation prevention bytes from RBSP payloads.

use alloc::vec::Vec;

/// Remove H.264/H.265 emulation prevention three bytes (`0x00 0x00 0x03`) from RBSP.
///
/// An emulation prevention sequence is `0x00 0x00 0x03 XX` where `XX <= 0x03`;
/// the `0x03` byte is discarded and `XX` is emitted as part of the payload.
/// Sequences that do not match this pattern (including a trailing `0x00 0x00 0x03`)
/// are left intact so arbitrary payloads are not corrupted.
pub fn unescape_rbsp(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if i + 2 < data.len() && data[i] == 0x00 && data[i + 1] == 0x00 && data[i + 2] == 0x03 {
            if i + 3 < data.len() && data[i + 3] <= 0x03 {
                // Emulation prevention: drop the 0x03 and emit the protected byte
                // in the next iteration.
                out.push(0x00);
                out.push(0x00);
                i += 3;
            } else {
                // Not a valid EPB (trailing or followed by > 0x03): keep the 0x03.
                out.push(0x00);
                out.push(0x00);
                out.push(0x03);
                i += 3;
            }
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_emulation_prevention_three_byte() {
        let input = [0x00, 0x00, 0x03, 0x01, 0x00, 0x00, 0x03, 0x00];
        assert_eq!(unescape_rbsp(&input), [0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn leaves_other_data_intact() {
        let input = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(unescape_rbsp(&input), [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn keeps_epb_like_sequence_followed_by_high_byte() {
        // 0x00 0x00 0x03 0xFF is not an EPB, so the 0x03 must be preserved.
        let input = [0x00, 0x00, 0x03, 0xff, 0x00, 0x00, 0x03, 0x03];
        assert_eq!(
            unescape_rbsp(&input),
            [0x00, 0x00, 0x03, 0xff, 0x00, 0x00, 0x03]
        );
    }

    #[test]
    fn keeps_trailing_zero_zero_three() {
        let input = [0x00, 0x00, 0x03];
        assert_eq!(unescape_rbsp(&input), [0x00, 0x00, 0x03]);
    }
}
