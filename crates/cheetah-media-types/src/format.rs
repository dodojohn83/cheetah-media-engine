//! Pixel, sample, color, and layout formats for decoded frames.

/// Video pixel formats supported by the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    Yuv420P,
    Yuv422P,
    Yuv444P,
    Nv12,
    Nv21,
    Rgba,
    Bgra,
    Rgb24,
    Bgr24,
    I420,
    Unknown(u32),
}

/// Color space / matrix coefficients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorSpace {
    Bt601,
    Bt709,
    Bt2020,
    Bt2020C,
    Smpte170M,
    Smpte240M,
    Unspecified,
}

/// Audio sample format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleFormat {
    U8,
    S16,
    S32,
    F32,
    F64,
    S16Planar,
    S32Planar,
    F32Planar,
    F64Planar,
    Unknown(u32),
}

/// Audio channel layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelLayout {
    Mono,
    Stereo,
    Surround21,
    Surround31,
    Surround40,
    Surround41,
    Surround50,
    Surround51,
    Surround61,
    Surround71,
    Unknown(u64),
}

impl ChannelLayout {
    /// Number of channels for well-known layouts.
    pub const fn channels(self) -> u32 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::Surround21 => 3,
            Self::Surround31 => 4,
            Self::Surround40 => 4,
            Self::Surround41 => 5,
            Self::Surround50 => 5,
            Self::Surround51 => 6,
            Self::Surround61 => 7,
            Self::Surround71 => 8,
            Self::Unknown(mask) => mask.count_ones(),
        }
    }

    /// Build a `ChannelLayout` from a raw channel count.
    ///
    /// Counts that do not match a named layout are stored as an `Unknown`
    /// bitmask with `count` low bits set so `ChannelLayout::channels()` still
    /// returns the original count.
    pub fn from_channel_count(count: u32) -> Self {
        match count {
            1 => Self::Mono,
            2 => Self::Stereo,
            0 => Self::Unknown(0),
            n if n >= 64 => Self::Unknown(u64::MAX),
            n => Self::Unknown((1u64 << n) - 1),
        }
    }
}

/// Video format descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoFormat {
    pub pixel_format: PixelFormat,
    pub coded_width: u32,
    pub coded_height: u32,
    pub visible_width: u32,
    pub visible_height: u32,
    /// Stride in bytes for the first plane; additional planes use `planes` in `VideoFrame`.
    pub stride: u32,
    pub color_space: ColorSpace,
}

impl VideoFormat {
    /// True if the visible size is within the coded size.
    pub const fn is_valid(self) -> bool {
        self.visible_width <= self.coded_width && self.visible_height <= self.coded_height
    }
}

/// Audio format descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioFormat {
    pub sample_format: SampleFormat,
    pub sample_rate: u32,
    pub channel_layout: ChannelLayout,
    /// Number of samples per channel in a single frame.
    pub sample_count: u32,
}

impl SampleFormat {
    /// Number of bytes per sample for the base element.
    pub const fn bytes_per_sample(self) -> u32 {
        match self {
            Self::U8 => 1,
            Self::S16 | Self::S16Planar => 2,
            Self::S32 | Self::F32 | Self::S32Planar | Self::F32Planar => 4,
            Self::F64 | Self::F64Planar => 8,
            Self::Unknown(_) => 0,
        }
    }
}

impl AudioFormat {
    /// Number of bytes per sample for the base element.
    pub const fn bytes_per_sample(self) -> u32 {
        self.sample_format.bytes_per_sample()
    }

    /// Total samples in the frame across all channels.
    pub const fn total_samples(self) -> u64 {
        (self.sample_count as u64).saturating_mul(self.channel_layout.channels() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_layout_counts_include_lfe() {
        assert_eq!(ChannelLayout::Surround40.channels(), 4);
        assert_eq!(ChannelLayout::Surround41.channels(), 5);
        assert_eq!(ChannelLayout::Surround50.channels(), 5);
        assert_eq!(ChannelLayout::Surround51.channels(), 6);
        assert_eq!(ChannelLayout::Surround61.channels(), 7);
        assert_eq!(ChannelLayout::Surround71.channels(), 8);
    }

    #[test]
    fn audio_format_total_samples() {
        let fmt = AudioFormat {
            sample_format: SampleFormat::S16,
            sample_rate: 48000,
            channel_layout: ChannelLayout::Surround51,
            sample_count: 1024,
        };
        assert_eq!(fmt.total_samples(), 1024 * 6);
    }
}
