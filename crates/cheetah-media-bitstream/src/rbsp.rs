//! Removal of H.264/H.265 emulation prevention bytes from RBSP payloads.

use alloc::vec::Vec;

/// Remove H.264/H.265 emulation prevention three bytes (`0x00 0x00 0x03`) from RBSP.
///
/// In both standards, a `0x03` byte inserted after `0x00 0x00` is not part of
/// the payload and must be discarded before bitstream parsing.
pub fn unescape_rbsp(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if i + 2 < data.len() && data[i] == 0x00 && data[i + 1] == 0x00 && data[i + 2] == 0x03 {
            out.push(0x00);
            out.push(0x00);
            i += 3;
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
}
