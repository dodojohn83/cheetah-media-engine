//! FLV container format parser, muxer, and recorder utilities.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

pub mod amf;
pub mod audio;
pub mod demuxer;
pub mod error;
pub mod header;
pub mod muxer;
pub mod recorder;
pub mod video;

pub use amf::{AmfLimits, AmfValue, FlvScriptData, parse_script_data};
pub use audio::{AudioTagHeader, SoundFormat};
pub use demuxer::{FlvDemuxer, FlvEvent, FlvMode};
pub use error::FlvError;
pub use header::{FlvHeader, FlvTagHeader, TagType};
pub use muxer::FlvMuxer;
pub use recorder::{FlvRecorder, FlvRecordingCancel, FlvWriter};
pub use video::{FrameType, VideoCodecId, VideoTagHeader};
