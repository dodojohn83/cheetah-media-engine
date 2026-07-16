//! Start-code scanning helpers for MPEG-PS and PES payloads.

/// Find the next `0x000001` start code in `data` starting from `start`.
pub fn find_start_code(data: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i.saturating_add(2) < data.len() {
        if data[i] == 0x00 && data[i + 1] == 0x00 && data[i + 2] == 0x01 {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find the next PS boundary (a 3-byte or 4-byte start code followed by a
/// stream id >= 0xB9). NAL start codes inside PES payloads are followed by
/// bytes < 0x80, so they will not be mistaken for PS boundaries.
pub fn find_ps_boundary(data: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i.saturating_add(3) < data.len() {
        if data[i] == 0x00 && data[i + 1] == 0x00 {
            // 4-byte start code 00 00 00 01.
            if data[i + 2] == 0x00
                && i.saturating_add(4) < data.len()
                && data[i + 3] == 0x01
                && data[i + 4] >= 0xB9
            {
                return Some(i);
            }
            // 3-byte start code 00 00 01.
            if data[i + 2] == 0x01 && data[i + 3] >= 0xB9 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_start_code_basic() {
        let data = [0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x01, 0xE1];
        assert_eq!(find_start_code(&data, 0), Some(0));
        assert_eq!(find_start_code(&data, 3), Some(4));
    }

    #[test]
    fn find_ps_boundary_ignores_nal_headers() {
        // Simulate a buffer where the first six bytes (PES prefix + length) have
        // already been consumed. The payload contains a 4-byte NAL start code
        // followed by an audio PES boundary.
        let data = [
            0x00, 0x00, 0x01, 0xE0, 0x00, 0x10, // consumed prefix
            0x00, 0x00, 0x00, 0x01, 0x67, // 4-byte NAL start code
            0x00, 0x00, 0x01, 0xC0, // audio PES start code
        ];
        assert_eq!(find_ps_boundary(&data, 6), Some(11));
    }
}
