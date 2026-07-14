//! FLV video tag parsing and building.

use alloc::vec::Vec;

use cheetah_media_bitstream::{
    h264::{self, H264CodecConfig},
    h265::{self, H265CodecConfig, NalUnitType as H265NalUnitType},
};
use cheetah_media_types::{CodecConfig, CodecId, MediaError, TrackInfo, TrackKind};

use crate::FlvError;

/// Video codec identifiers in the first byte of an FLV video tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VideoCodecId {
    H264 = 7,
    H265 = 12,
    Other(u8),
}

impl VideoCodecId {
    pub const fn from_u8(v: u8) -> Self {
        match v {
            7 => Self::H264,
            12 => Self::H265,
            _ => Self::Other(v),
        }
    }

    pub const fn to_codec_id(self) -> Option<CodecId> {
        match self {
            Self::H264 => Some(CodecId::H264),
            Self::H265 => Some(CodecId::H265),
            Self::Other(_) => None,
        }
    }
}

/// FLV video frame types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    Keyframe = 1,
    Interframe = 2,
    Disposable = 3,
    Generated = 4,
    Info = 5,
    Unknown(u8),
}

impl FrameType {
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Keyframe,
            2 => Self::Interframe,
            3 => Self::Disposable,
            4 => Self::Generated,
            5 => Self::Info,
            _ => Self::Unknown(v),
        }
    }
}

/// Parsed FLV video tag header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoTagHeader {
    pub frame_type: FrameType,
    pub codec_id: VideoCodecId,
    /// AVC/HEVC packet type: 0 = sequence header, 1 = NALUs, 2 = end of sequence.
    pub packet_type: u8,
    /// Composition time offset in milliseconds (PTS - DTS).
    pub cts_ms: i32,
    pub header_size: usize,
}

impl VideoTagHeader {
    /// Parse a video tag header from the start of `data`.
    pub fn parse(data: &[u8]) -> Result<Self, FlvError> {
        if data.is_empty() {
            return Err(FlvError::NeedMoreData);
        }
        let b0 = data[0];
        let frame_type = FrameType::from_u8(b0 >> 4);
        let codec_id = VideoCodecId::from_u8(b0 & 0x0f);

        match codec_id {
            VideoCodecId::H264 | VideoCodecId::H265 => {
                if data.len() < 5 {
                    return Err(FlvError::NeedMoreData);
                }
                let packet_type = data[1];
                let cts = i32::from_be_bytes([0, data[2], data[3], data[4]]);
                // Sign-extend the 24-bit composition time.
                let cts_ms = if cts & 0x80_0000 != 0 {
                    cts | !0x00ff_ffff
                } else {
                    cts
                };
                Ok(Self {
                    frame_type,
                    codec_id,
                    packet_type,
                    cts_ms,
                    header_size: 5,
                })
            }
            VideoCodecId::Other(_) => Ok(Self {
                frame_type,
                codec_id,
                packet_type: 0,
                cts_ms: 0,
                header_size: 1,
            }),
        }
    }

    /// Build a 5-byte AVC/HEVC video tag header.
    pub fn build_avc_hevc(
        frame_type: FrameType,
        codec_id: VideoCodecId,
        packet_type: u8,
        cts_ms: i32,
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(5);
        let ft = match frame_type {
            FrameType::Keyframe => 1,
            FrameType::Interframe => 2,
            FrameType::Disposable => 3,
            FrameType::Generated => 4,
            FrameType::Info => 5,
            FrameType::Unknown(v) => v,
        };
        let cid = match codec_id {
            VideoCodecId::H264 => 7,
            VideoCodecId::H265 => 12,
            VideoCodecId::Other(v) => v & 0x0f,
        };
        out.push((ft << 4) | cid);
        out.push(packet_type);
        out.extend_from_slice(&[(cts_ms >> 16) as u8, (cts_ms >> 8) as u8, cts_ms as u8]);
        out
    }
}

/// Parse an AVC/HEVC sequence header and update `track`.
pub fn parse_video_config(
    track: &mut TrackInfo,
    data: &[u8],
    codec_id: VideoCodecId,
) -> Result<(), FlvError> {
    if track.kind != TrackKind::Video {
        return Err(FlvError::MalformedTag);
    }
    match codec_id {
        VideoCodecId::H264 => {
            let config = H264CodecConfig::parse(data).map_err(|_| FlvError::MalformedTag)?;
            track.codec = CodecId::H264;
            track.set_codec_config(CodecConfig::AvcC(config.build()));
            if config.width != 0 && config.height != 0 {
                let w = config.width;
                let h = config.height;
                let format = cheetah_media_types::VideoFormat {
                    pixel_format: cheetah_media_types::PixelFormat::Yuv420P,
                    coded_width: w,
                    coded_height: h,
                    visible_width: w,
                    visible_height: h,
                    stride: w,
                    color_space: cheetah_media_types::ColorSpace::Unspecified,
                };
                track.set_video_format(format).ok();
            }
            Ok(())
        }
        VideoCodecId::H265 => {
            let config = H265CodecConfig::parse(data).map_err(|_| FlvError::MalformedTag)?;
            track.codec = CodecId::H265;
            track.set_codec_config(CodecConfig::HevcC(config.build()));
            // H.265 config parsing does not currently expose visible dimensions;
            // they will be derived from the SPS in a later work package.
            Ok(())
        }
        _ => Err(FlvError::UnsupportedCodec),
    }
}

/// Determine whether an H.264/H.265 AVCC/HVCC payload contains a keyframe.
fn payload_is_keyframe(payload: &[u8], codec_id: VideoCodecId) -> bool {
    match codec_id {
        VideoCodecId::H264 => {
            if let Ok(nals) = h264::split_avcc(payload, 4) {
                for nal in nals {
                    if !nal.data.is_empty() {
                        let nal_type = nal.data[0] & 0x1f;
                        if nal_type == 5 {
                            return true;
                        }
                    }
                }
            }
        }
        VideoCodecId::H265 => {
            if let Ok(nals) = h265::split_hvcc(payload, 4) {
                for nal in nals {
                    if nal.data.len() >= 2 {
                        let header = u16::from_be_bytes([nal.data[0], nal.data[1]]);
                        let nal_type = ((header >> 1) & 0x3f) as u8;
                        let t = H265NalUnitType::from_u8(nal_type);
                        if t.is_irap() {
                            return true;
                        }
                    }
                }
            }
        }
        _ => {}
    }
    false
}

/// True if the payload should be treated as a keyframe given the tag header.
pub fn is_keyframe(payload: &[u8], header: &VideoTagHeader) -> bool {
    match header.frame_type {
        FrameType::Keyframe => {
            // Refine H.265 by inspecting NAL types; for H.264 trust the header
            // because the frame type already signals an IDR frame.
            match header.codec_id {
                VideoCodecId::H265 => payload_is_keyframe(payload, header.codec_id),
                VideoCodecId::H264 => payload_is_keyframe(payload, header.codec_id),
                _ => true,
            }
        }
        _ => false,
    }
}

/// Build a video tag body (header + payload) for AVC/HEVC.
pub fn build_video_frame(
    codec: CodecId,
    keyframe: bool,
    packet_type: u8,
    cts_ms: i32,
    payload: &[u8],
) -> Result<Vec<u8>, FlvError> {
    let codec_id = match codec {
        CodecId::H264 => VideoCodecId::H264,
        CodecId::H265 => VideoCodecId::H265,
        _ => return Err(FlvError::UnsupportedCodec),
    };
    let frame_type = if keyframe {
        FrameType::Keyframe
    } else {
        FrameType::Interframe
    };
    let mut out = VideoTagHeader::build_avc_hevc(frame_type, codec_id, packet_type, cts_ms);
    out.extend_from_slice(payload);
    Ok(out)
}

/// Build an AVC/HEVC sequence-header tag body from `track`.
pub fn build_video_config(track: &TrackInfo) -> Result<Vec<u8>, FlvError> {
    let (codec_id, payload) = match track.codec {
        CodecId::H264 => {
            let bytes = track.codec_config.bytes().ok_or(FlvError::MalformedTag)?;
            (VideoCodecId::H264, bytes.to_vec())
        }
        CodecId::H265 => {
            let bytes = track.codec_config.bytes().ok_or(FlvError::MalformedTag)?;
            (VideoCodecId::H265, bytes.to_vec())
        }
        _ => return Err(FlvError::UnsupportedCodec),
    };
    let mut out = VideoTagHeader::build_avc_hevc(FrameType::Keyframe, codec_id, 0, 0);
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Validate that a video track can be represented in FLV.
pub fn check_video_track(track: &TrackInfo) -> Result<(), MediaError> {
    match track.codec {
        CodecId::H264 | CodecId::H265 => Ok(()),
        _ => Err(MediaError::Unsupported {
            code: 2002,
            context: Some("video codec not supported by FLV muxer"),
        }),
    }
}
