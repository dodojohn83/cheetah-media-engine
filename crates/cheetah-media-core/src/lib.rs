//! Facade crate for the shared Cheetah media core.

#![cfg_attr(not(any(test, feature = "std")), no_std)]

pub use cheetah_container_flv as flv;
pub use cheetah_container_isobmff as isobmff;
pub use cheetah_container_mpegts as mpegts;
pub use cheetah_hls_client as hls;
pub use cheetah_media_abi as abi;
pub use cheetah_media_bitstream as bitstream;
pub use cheetah_media_pipeline_core as pipeline;
pub use cheetah_media_timeline as timeline;
pub use cheetah_media_types as types;

/// Core facade version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
