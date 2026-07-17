//! Bounded FLV recorder with backpressure via an external writer.

use alloc::vec::Vec;

use cheetah_media_types::{MediaPacket, TrackInfo};

use crate::{FlvError, muxer::FlvMuxer};

/// Writer sink for an FLV recorder.
///
/// The recorder calls `write_chunk` whenever its internal buffer exceeds the
/// configured threshold. A real implementation can, for example, append to a
/// `WritableStream`, a file, or an in-memory buffer.
pub trait FlvWriter {
    /// Write a chunk of FLV bytes.
    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), FlvError>;
}

impl<W> FlvWriter for W
where
    W: FnMut(&[u8]) -> Result<(), FlvError>,
{
    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), FlvError> {
        (self)(bytes)
    }
}

/// Recorder state returned by `FlvRecorder::cancel`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlvRecordingCancel {
    /// FLV bytes that had been produced but not yet flushed to the writer.
    pub partial_output: Vec<u8>,
    /// Number of packets still held in the reorder queue when cancel occurred.
    pub unflushed_packets: usize,
}

/// Streaming FLV recorder.
///
/// The recorder buffers a bounded number of packets in `FlvMuxer` and flushes
/// completed FLV bytes to the writer as soon as they exceed `flush_threshold`.
/// This prevents accumulating the entire recording in memory.
#[derive(Debug)]
pub struct FlvRecorder<W: FlvWriter> {
    muxer: FlvMuxer,
    writer: W,
    tracks: Vec<TrackInfo>,
    flush_threshold: usize,
}

impl<W: FlvWriter> FlvRecorder<W> {
    /// Create a recorder with the given writer.
    pub fn new(writer: W, file_mode: bool, max_queue_depth: usize, flush_threshold: usize) -> Self {
        Self {
            muxer: FlvMuxer::new(file_mode, max_queue_depth),
            writer,
            tracks: Vec::new(),
            flush_threshold,
        }
    }

    /// Register a track. Returns `UnsupportedCodec` if an incompatible track
    /// with the same ID is already registered.
    pub fn add_track(&mut self, track: &TrackInfo) -> Result<(), FlvError> {
        if let Some(existing) = self.tracks.iter().find(|t| t.id == track.id) {
            if existing.codec != track.codec || existing.codec_config != track.codec_config {
                return Err(FlvError::UnsupportedCodec);
            }
        } else {
            self.tracks.push(track.clone());
        }
        self.muxer.add_track(track)
    }

    /// Queue a packet for recording. Completed FLV bytes are flushed to the
    /// writer when the internal buffer grows beyond the threshold.
    pub fn push_packet(
        &mut self,
        packet: MediaPacket<'static>,
        track: &TrackInfo,
    ) -> Result<(), FlvError> {
        if !self.tracks.iter().any(|t| t.id == track.id) {
            return Err(FlvError::UnsupportedCodec);
        }
        self.muxer.push_packet(packet, track)?;
        if self.muxer.output().len() >= self.flush_threshold {
            self.flush()?;
        }
        Ok(())
    }

    /// Flush any completed FLV bytes to the writer.
    pub fn flush(&mut self) -> Result<(), FlvError> {
        let chunk = self.muxer.take_output();
        if !chunk.is_empty() {
            self.writer.write_chunk(&chunk)?;
        }
        Ok(())
    }

    /// Finalize the recording and flush all remaining bytes.
    pub fn stop(mut self) -> Result<(), FlvError> {
        self.flush()?;
        let final_bytes = self.muxer.finish()?;
        if !final_bytes.is_empty() {
            self.writer.write_chunk(&final_bytes)?;
        }
        Ok(())
    }

    /// Cancel the recording and return a partial state without finalizing the
    /// file footer or flushing the reorder queue.
    pub fn cancel(mut self) -> FlvRecordingCancel {
        let unflushed = self.muxer.pending_packet_count();
        FlvRecordingCancel {
            partial_output: self.muxer.take_output(),
            unflushed_packets: unflushed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use cheetah_media_bitstream::h264::H264CodecConfig;
    use cheetah_media_types::{
        BufferRef, CodecConfig, CodecId, MediaPacket, MediaTime, PixelFormat, SequenceNumber,
        StreamEpoch, TimeBase, TrackId, TrackInfo, TrackKind, VideoFormat,
    };

    fn make_video_track() -> TrackInfo {
        let sps = vec![0x67, 0x42, 0x00, 0x1e];
        let pps = vec![0x68, 0xce, 0x3c, 0x80];
        let config = H264CodecConfig {
            configuration_version: 1,
            avc_profile_indication: 0x42,
            profile_compatibility: 0x00,
            avc_level_indication: 0x1e,
            length_size_minus_one: 3,
            sps_list: vec![sps],
            pps_list: vec![pps],
            width: 320,
            height: 240,
            codec_string: alloc::string::String::new(),
        };
        let mut track = TrackInfo::new(
            TrackId::new(1).unwrap(),
            TrackKind::Video,
            CodecId::H264,
            TimeBase::DEFAULT,
        );
        track.set_codec_config(CodecConfig::AvcC(config.build()));
        track
            .set_video_format(VideoFormat {
                pixel_format: PixelFormat::Yuv420P,
                coded_width: 320,
                coded_height: 240,
                visible_width: 320,
                visible_height: 240,
                stride: 320,
                color_space: cheetah_media_types::ColorSpace::Unspecified,
            })
            .unwrap();
        track
    }

    #[test]
    fn recorder_flushes_incrementally_and_finalizes() {
        let track = make_video_track();
        let mut out = Vec::new();
        let mut recorder = FlvRecorder::new(
            |bytes: &[u8]| {
                out.extend_from_slice(bytes);
                Ok(())
            },
            true,
            32,
            1, // flush after every byte produced
        );

        recorder.add_track(&track).unwrap();
        let packet = MediaPacket::new(
            BufferRef::from_owned(vec![0x00, 0x00, 0x00, 0x02, 0x65, 0x88]),
            track.id,
            StreamEpoch::new(0),
            SequenceNumber::new(0),
            MediaTime::from_ticks(Some(100), Some(100), None, TimeBase::DEFAULT),
        )
        .with_keyframe();
        recorder.push_packet(packet, &track).unwrap();

        recorder.stop().unwrap();
        assert!(out.starts_with(b"FLV"));

        // Final bytes should contain a valid FLV file end.
        assert!(!out.is_empty());
    }

    #[test]
    fn recorder_cancel_returns_partial() {
        let track = make_video_track();
        let mut out = Vec::new();
        let mut recorder = FlvRecorder::new(
            |bytes: &[u8]| {
                out.extend_from_slice(bytes);
                Ok(())
            },
            true,
            32,
            64 * 1024, // large threshold so nothing flushes
        );

        recorder.add_track(&track).unwrap();
        let packet = MediaPacket::new(
            BufferRef::from_owned(vec![0x00, 0x00, 0x00, 0x02, 0x65, 0x88]),
            track.id,
            StreamEpoch::new(0),
            SequenceNumber::new(0),
            MediaTime::from_ticks(Some(100), Some(100), None, TimeBase::DEFAULT),
        )
        .with_keyframe();
        recorder.push_packet(packet, &track).unwrap();

        let cancel = recorder.cancel();
        // The muxer has already emitted the header and config tag, but the
        // media packet is still held in the bounded reorder queue.
        assert!(!cancel.partial_output.is_empty());
        assert_eq!(cancel.unflushed_packets, 1);
    }
}
