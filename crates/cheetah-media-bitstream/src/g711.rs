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
}
