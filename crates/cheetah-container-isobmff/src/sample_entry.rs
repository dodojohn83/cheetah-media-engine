//! Sample entry parsing (avcC, hvcC, esds) into `TrackInfo` fields.

use cheetah_media_types::{
    AudioFormat, ChannelLayout, CodecConfig, CodecId, ColorSpace, PixelFormat, SampleFormat,
    TrackInfo, TrackKind, VideoFormat,
};

use crate::Mp4Error;
use crate::boxes::{iter_boxes, read_fullbox_header, types};

/// Parsed information from an `stsd` sample entry.
#[derive(Debug, Clone)]
pub struct SampleEntry {
    pub kind: TrackKind,
    pub codec: CodecId,
    pub width: u16,
    pub height: u16,
    pub codec_config: CodecConfig,
    pub audio_format: Option<AudioFormat>,
}

impl SampleEntry {
    pub fn apply(&self, info: &mut TrackInfo) {
        info.kind = self.kind;
        info.codec = self.codec;
        info.set_codec_config(self.codec_config.clone());
        if let Some(fmt) = self.audio_format {
            info.set_audio_format(fmt).ok();
        }
        if self.kind == TrackKind::Video {
            let format = VideoFormat {
                pixel_format: PixelFormat::Yuv420P,
                coded_width: u32::from(self.width),
                coded_height: u32::from(self.height),
                visible_width: u32::from(self.width),
                visible_height: u32::from(self.height),
                stride: u32::from(self.width),
                color_space: ColorSpace::Unspecified,
            };
            info.set_video_format(format).ok();
        }
    }
}

/// Parse a sample entry box body (after the box header) into `SampleEntry`.
pub fn parse_sample_entry(box_type: u32, body: &[u8]) -> Result<Option<SampleEntry>, Mp4Error> {
    match box_type {
        types::AVC1 | types::AVC3 => parse_visual_sample_entry(body, CodecId::H264),
        types::HVC1 | types::HEV1 => parse_visual_sample_entry(body, CodecId::H265),
        types::MP4A => parse_audio_sample_entry(body),
        _ => Ok(None),
    }
}

fn parse_visual_sample_entry(body: &[u8], codec: CodecId) -> Result<Option<SampleEntry>, Mp4Error> {
    // VisualSampleEntry prefix is 78 bytes; width at bytes 24-25, height at 26-27.
    if body.len() < 78 {
        return Err(Mp4Error::NeedMoreData);
    }
    let width = u16::from_be_bytes([body[24], body[25]]);
    let height = u16::from_be_bytes([body[26], body[27]]);

    for item in iter_boxes(&body[78..], 78_u64, 4)? {
        let (header, inner) = item?;
        match header.box_type {
            types::AVCC => {
                let cfg = cheetah_media_bitstream::H264CodecConfig::parse(inner)
                    .map_err(|_| Mp4Error::invalid_input(3101, Some("invalid avcC")))?;
                let (w, h) = if cfg.width > 0 && cfg.height > 0 {
                    (cfg.width as u16, cfg.height as u16)
                } else {
                    (width, height)
                };
                return Ok(Some(SampleEntry {
                    kind: TrackKind::Video,
                    codec,
                    width: w,
                    height: h,
                    codec_config: CodecConfig::AvcC(cfg.build()),
                    audio_format: None,
                }));
            }
            types::HVCC => {
                let cfg = cheetah_media_bitstream::H265CodecConfig::parse(inner)
                    .map_err(|_| Mp4Error::invalid_input(3102, Some("invalid hvcC")))?;
                return Ok(Some(SampleEntry {
                    kind: TrackKind::Video,
                    codec,
                    width,
                    height,
                    codec_config: CodecConfig::HevcC(cfg.build()),
                    audio_format: None,
                }));
            }
            _ => {}
        }
    }

    Ok(Some(SampleEntry {
        kind: TrackKind::Video,
        codec,
        width,
        height,
        codec_config: CodecConfig::None,
        audio_format: None,
    }))
}

fn parse_audio_sample_entry(body: &[u8]) -> Result<Option<SampleEntry>, Mp4Error> {
    // AudioSampleEntry prefix is 28 bytes:
    // 8 (SampleEntry) + 8 (reserved) + 2 channelcount + 2 samplesize + 2 pre_defined + 2 reserved + 4 samplerate.
    if body.len() < 28 {
        return Err(Mp4Error::NeedMoreData);
    }
    let channel_count = u16::from_be_bytes([body[16], body[17]]);
    let sample_rate_raw = u32::from_be_bytes([body[24], body[25], body[26], body[27]]);
    let sample_rate = sample_rate_raw >> 16;

    for item in iter_boxes(&body[28..], 28_u64, 4)? {
        let (header, inner) = item?;
        if header.box_type == types::ESDS
            && let Some(asc) = parse_esds(inner)?
        {
            let fmt = AudioFormat {
                sample_format: SampleFormat::S16,
                sample_rate: asc.sampling_frequency,
                channel_layout: if asc.channel_count == 1 {
                    ChannelLayout::Mono
                } else {
                    ChannelLayout::Stereo
                },
                sample_count: 1024,
            };
            return Ok(Some(SampleEntry {
                kind: TrackKind::Audio,
                codec: CodecId::Aac,
                width: 0,
                height: 0,
                codec_config: CodecConfig::AacAudioSpecificConfig(asc.build()),
                audio_format: Some(fmt),
            }));
        }
    }

    let fmt = AudioFormat {
        sample_format: SampleFormat::S16,
        sample_rate,
        channel_layout: if channel_count == 1 {
            ChannelLayout::Mono
        } else {
            ChannelLayout::Stereo
        },
        sample_count: 1024,
    };

    Ok(Some(SampleEntry {
        kind: TrackKind::Audio,
        codec: CodecId::Aac,
        width: 0,
        height: 0,
        codec_config: CodecConfig::None,
        audio_format: Some(fmt),
    }))
}

/// Parse an `esds` box body and return the `AudioSpecificConfig` inside.
fn parse_esds(
    data: &[u8],
) -> Result<Option<cheetah_media_bitstream::AudioSpecificConfig>, Mp4Error> {
    let (_, _, body) = read_fullbox_header(data)?;
    let (tag, es_body, _rest) = read_descriptor_tag_length(body)?;
    if tag != 0x03 {
        return Ok(None);
    }
    // ES_ID (2) + flags (1)
    if es_body.len() < 3 {
        return Ok(None);
    }
    let (tag, dcd_body, _rest) = read_descriptor_tag_length(&es_body[3..])?;
    if tag != 0x04 {
        return Ok(None);
    }
    // DecoderConfigDescriptor prefix is 13 bytes before DecoderSpecificInfo.
    if dcd_body.len() < 13 {
        return Ok(None);
    }
    let (tag, dsi_body, _rest) = read_descriptor_tag_length(&dcd_body[13..])?;
    if tag != 0x05 || dsi_body.len() < 2 {
        return Ok(None);
    }
    cheetah_media_bitstream::AudioSpecificConfig::parse(dsi_body)
        .ok()
        .map_or(Ok(None), |asc| Ok(Some(asc)))
}

fn read_descriptor_tag_length(data: &[u8]) -> Result<(u8, &[u8], &[u8]), Mp4Error> {
    if data.is_empty() {
        return Err(Mp4Error::NeedMoreData);
    }
    let tag = data[0];
    let mut len: usize = 0;
    let mut i = 1;
    while i < data.len() {
        let b = data[i];
        i += 1;
        len = len.checked_shl(7).ok_or(Mp4Error::LimitExceeded {
            limit: "descriptor length",
        })? | ((b & 0x7f) as usize);
        if b & 0x80 == 0 {
            break;
        }
    }
    if i + len > data.len() {
        return Err(Mp4Error::NeedMoreData);
    }
    Ok((tag, &data[i..i + len], &data[i + len..]))
}
