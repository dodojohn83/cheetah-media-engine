//! G.711 A-law and mu-law PCM conversion.

/// G.711 decoder kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum G711Kind {
    ALaw,
    MuLaw,
}

/// PCM format description for G.711 output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PcmFormat {
    pub sample_rate: u32,
    pub channel_count: u8,
    pub bits_per_sample: u8,
    pub duration_ms_per_sample: f64,
}

impl PcmFormat {
    pub const fn new_u8(sample_rate: u32, channels: u8) -> Self {
        Self {
            sample_rate,
            channel_count: channels,
            bits_per_sample: 16,
            duration_ms_per_sample: 1000.0 / sample_rate as f64,
        }
    }
}

/// Decode a mu-law sample (8-bit) to 16-bit signed PCM.
pub fn ulaw_to_pcm(sample: u8) -> i16 {
    const EXP_LUT: [i16; 8] = [0, 132, 396, 924, 1980, 4092, 8316, 16764];
    let ulawbyte = !sample;
    let sign = (ulawbyte & 0x80) != 0;
    let exponent = ((ulawbyte >> 4) & 0x07) as usize;
    let mantissa = (ulawbyte & 0x0f) as i16;
    let mut linear = EXP_LUT[exponent] + (mantissa << (exponent + 3));
    if sign {
        linear = -linear;
    }
    linear
}

/// Decode an A-law sample (8-bit) to 16-bit signed PCM.
pub fn alaw_to_pcm(sample: u8) -> i16 {
    let a = (sample ^ 0x55) as i32;
    let sign = (a & 0x80) != 0;
    let t = a & 0x7f;
    let value = if t < 16 {
        (t << 4) + 8
    } else {
        let seg = (t >> 4) & 0x07;
        (((t & 0x0f) << 4) + 0x108) << (seg - 1)
    };
    if sign { value as i16 } else { (-value) as i16 }
}

/// Decode a G.711 byte to 16-bit PCM.
pub fn decode(kind: G711Kind, sample: u8) -> i16 {
    match kind {
        G711Kind::ALaw => alaw_to_pcm(sample),
        G711Kind::MuLaw => ulaw_to_pcm(sample),
    }
}

/// Decode a buffer of G.711 samples in place into `output`.
pub fn decode_buffer(kind: G711Kind, input: &[u8], output: &mut [i16]) {
    let f = match kind {
        G711Kind::ALaw => alaw_to_pcm,
        G711Kind::MuLaw => ulaw_to_pcm,
    };
    let n = input.len().min(output.len());
    for i in 0..n {
        output[i] = f(input[i]);
    }
}

/// Encode a single 16-bit PCM sample to G.711.
pub fn encode(kind: G711Kind, sample: i16) -> u8 {
    match kind {
        G711Kind::ALaw => alaw_from_pcm(sample),
        G711Kind::MuLaw => ulaw_from_pcm(sample),
    }
}

/// Encode a buffer of 16-bit PCM samples into `output`.
pub fn encode_buffer(kind: G711Kind, input: &[i16], output: &mut [u8]) {
    let f = match kind {
        G711Kind::ALaw => alaw_from_pcm,
        G711Kind::MuLaw => ulaw_from_pcm,
    };
    let n = input.len().min(output.len());
    for i in 0..n {
        output[i] = f(input[i]);
    }
}

/// Encode a single 32-bit float PCM sample (nominal range [-1.0, 1.0]) to G.711.
pub fn encode_f32(kind: G711Kind, sample: f32) -> u8 {
    let clipped = if sample > 1.0 {
        i16::MAX
    } else if sample < -1.0 {
        i16::MIN
    } else {
        (sample * i16::MAX as f32) as i16
    };
    encode(kind, clipped)
}

/// Encode a buffer of 32-bit float PCM samples (nominal range [-1.0, 1.0]) into `output`.
pub fn encode_buffer_f32(kind: G711Kind, input: &[f32], output: &mut [u8]) {
    let f = match kind {
        G711Kind::ALaw => alaw_from_pcm,
        G711Kind::MuLaw => ulaw_from_pcm,
    };
    let n = input.len().min(output.len());
    for i in 0..n {
        let sample = input[i];
        let clipped = if sample > 1.0 {
            i16::MAX
        } else if sample < -1.0 {
            i16::MIN
        } else {
            (sample * i16::MAX as f32) as i16
        };
        output[i] = f(clipped);
    }
}

/// A-law segment end values used during encoding.
///
/// A-law encodes 16-bit PCM after an arithmetic right-shift of 3, producing
/// a 13-bit magnitude. `seg_aend` gives the inclusive upper bound of each
/// segment in that 13-bit range.
const A_LAW_SEG_AEND: [i32; 8] = [0x1F, 0x3F, 0x7F, 0xFF, 0x1FF, 0x3FF, 0x7FF, 0xFFF];

/// Find the smallest `i` such that `val <= table[i]`.
fn search_segment(val: i32, table: &[i32]) -> usize {
    for (i, &end) in table.iter().enumerate() {
        if val <= end {
            return i;
        }
    }
    table.len()
}

/// Encode a 16-bit linear PCM sample to 8-bit A-law.
fn alaw_from_pcm(sample: i16) -> u8 {
    let mask: u8;
    // A-law encoding first reduces the 16-bit input to 13 bits.
    let mut pcm_val: i32 = (i32::from(sample)) >> 3;

    if pcm_val >= 0 {
        mask = 0xD5; /* sign (7th) bit = 1 */
    } else {
        mask = 0x55; /* sign bit = 0 */
        pcm_val = -pcm_val - 1;
    }

    let seg = search_segment(pcm_val, &A_LAW_SEG_AEND);

    let aval: u8 = if seg >= 8 {
        /* Out of range, return maximum value. */
        0x7F
    } else {
        let mant = if seg < 2 {
            (pcm_val >> 1) & 0x0F
        } else {
            (pcm_val >> seg) & 0x0F
        };
        ((seg as u8) << 4) | mant as u8
    };

    aval ^ mask
}

/// Mu-law exponent lookup table for the top 9 bits of the biased magnitude.
const MU_LAW_EXP_LUT: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let v = i as u8;
        table[i] = if v == 0 {
            0
        } else {
            7 - v.leading_zeros() as u8
        };
        i += 1;
    }
    table
};

/// Encode a 16-bit linear PCM sample to 8-bit mu-law.
fn ulaw_from_pcm(sample: i16) -> u8 {
    const BIAS: i32 = 0x84; // 132

    let sign: u8;
    let mut magnitude: i32 = i32::from(sample);
    if magnitude < 0 {
        sign = 0x80;
        magnitude = -magnitude;
    } else {
        sign = 0;
    }

    magnitude += BIAS;
    if magnitude > 0x7FFF {
        magnitude = 0x7FFF;
    }

    let index = ((magnitude >> 7) & 0xFF) as usize;
    let exponent = MU_LAW_EXP_LUT[index] as i32;
    let mantissa = (magnitude >> (exponent + 3)) & 0x0F;

    let internal = sign as i32 | (exponent << 4) | mantissa;
    !(internal as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_and_sign() {
        // 0xff is mu-law silence.
        assert_eq!(ulaw_to_pcm(0xff), 0);
        // A-law zero encodes to a symmetric pair around 0 (±8).
        assert_eq!(alaw_to_pcm(0xd5), 8);
        assert_eq!(alaw_to_pcm(0x55), -8);
    }

    #[test]
    fn decode_buffer_preserves_length() {
        let input = [0xff, 0xff, 0xff];
        let mut output = [0i16; 3];
        decode_buffer(G711Kind::MuLaw, &input, &mut output);
        assert_eq!(output, [0, 0, 0]);
    }

    #[test]
    fn encode_silence() {
        assert_eq!(encode(G711Kind::MuLaw, 0), 0xff);
        assert_eq!(encode(G711Kind::ALaw, 0), 0xd5);
        assert_eq!(encode(G711Kind::ALaw, -8), 0x55);
    }

    #[test]
    fn encode_buffer_fills_output() {
        let input = [0i16, 100, -100, 1000, -1000];
        let mut output = [0u8; 5];
        encode_buffer(G711Kind::MuLaw, &input, &mut output);
        assert_eq!(output[0], 0xff);
        assert_ne!(output[1], output[2]);
        assert_ne!(output[3], output[4]);
    }

    #[test]
    fn encode_f32_silence_and_extremes() {
        assert_eq!(encode_f32(G711Kind::MuLaw, 0.0), 0xff);
        assert_eq!(encode_f32(G711Kind::ALaw, 0.0), 0xd5);
        assert_eq!(encode_f32(G711Kind::ALaw, 1.0), 0xaa); // largest positive
        assert_eq!(encode_f32(G711Kind::ALaw, -1.0), 0x2a); // largest negative
    }

    #[test]
    fn encode_f32_buffer_length() {
        let input = [0.0f32, 0.5, -0.5];
        let mut output = [0u8; 2];
        encode_buffer_f32(G711Kind::MuLaw, &input, &mut output);
        assert_eq!(output[0], 0xff);
        // output.len() is shorter than input, so only two samples are written.
        assert_eq!(output.len(), 2);
    }

    #[test]
    fn round_trip_is_within_tolerance() {
        // A-law and mu-law are lossy; ensure the round-trip is within the
        // expected quantization error for a representative set of samples.
        let samples: &[i16] = &[
            0, 1, 8, -8, 100, -100, 500, -500, 2000, -2000, 8000, -8000, 16000, -16000,
        ];

        for &s in samples {
            let mu = encode(G711Kind::MuLaw, s);
            let mu_back = decode(G711Kind::MuLaw, mu);
            let mu_error = (i32::from(mu_back) - i32::from(s)).abs();
            assert!(
                mu_error <= 256,
                "mu-law round-trip failed for {s}: {mu_back}"
            );

            let a = encode(G711Kind::ALaw, s);
            let a_back = decode(G711Kind::ALaw, a);
            let a_error = (i32::from(a_back) - i32::from(s)).abs();
            assert!(a_error <= 256, "a-law round-trip failed for {s}: {a_back}");
        }
    }
}
