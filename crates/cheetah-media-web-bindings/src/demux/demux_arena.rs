//! Helpers for storing demuxer output in a `MemoryArena`.

use cheetah_media_abi::{AbiError, Handle, MemoryArena, MemoryDescriptor};
use cheetah_media_types::{CodecId, MediaPacket, MediaTime, TimeBase, TrackInfo};

use super::demux_event::{DemuxEvent, DemuxEventKind};

pub(crate) fn codec_to_u8(codec: CodecId) -> u8 {
    match codec {
        CodecId::H264 => 0,
        CodecId::H265 => 1,
        CodecId::Aac => 2,
        CodecId::G711A => 3,
        CodecId::G711U => 4,
        CodecId::Mp3 => 5,
        CodecId::Opus => 6,
        CodecId::PcmU8 => 7,
        CodecId::PcmS16 => 8,
        CodecId::Unknown(_) => 255,
    }
}

fn ticks_to_ms(time: MediaTime, ts: Option<cheetah_media_types::Timestamp>) -> i64 {
    let Some(ts) = ts else { return 0 };
    time.timebase
        .rescale_i64(ts.ticks(), TimeBase::DEFAULT)
        .unwrap_or(0)
}

fn store_or_empty(
    arena: &mut MemoryArena,
    data: &[u8],
) -> Result<(Option<Handle>, MemoryDescriptor), AbiError> {
    if data.is_empty() {
        Ok((
            None,
            MemoryDescriptor {
                region: 0,
                offset: 0,
                length: 0,
                capacity: 0,
                generation: 0,
                flags: 0,
            },
        ))
    } else {
        let (handle, desc) = arena.store(data)?;
        let instance_id = arena.instance_id();
        Ok((
            Some(Handle {
                instance_id,
                slot: handle.slot,
                generation: handle.generation,
            }),
            desc,
        ))
    }
}

pub(crate) fn write_track(
    arena: &mut MemoryArena,
    track: &TrackInfo,
) -> Result<(DemuxEvent, Option<Handle>), AbiError> {
    let (config_handle, config_desc) = if let Some(bytes) = track.codec_config.bytes() {
        store_or_empty(arena, bytes)?
    } else {
        store_or_empty(arena, &[])?
    };

    let (width, height) = track
        .video_format
        .map(|vf| (vf.visible_width, vf.visible_height))
        .unwrap_or((0, 0));
    let (sample_rate, channels) = track
        .audio_format
        .map(|af| (af.sample_rate, af.channel_layout.channels()))
        .unwrap_or((0, 0));

    let event = DemuxEvent {
        kind: DemuxEventKind::Track,
        track_id: track.id.get(),
        track_kind: match track.kind {
            cheetah_media_types::TrackKind::Video => 0,
            cheetah_media_types::TrackKind::Audio => 1,
            cheetah_media_types::TrackKind::Data => 2,
        },
        codec: codec_to_u8(track.codec),
        width,
        height,
        sample_rate,
        channels,
        config_slot: config_handle.as_ref().map_or(0, |h| h.slot),
        config_generation: config_handle.as_ref().map_or(0, |h| h.generation),
        config_offset: config_desc.offset,
        config_len: if config_handle.is_some() {
            config_desc.length
        } else {
            0
        },
        data_slot: 0,
        data_generation: 0,
        data_offset: 0,
        data_len: 0,
        pts_ms: 0,
        dts_ms: 0,
        duration_ms: 0,
        flags: 0,
        error_code: 0,
        error_message: String::new(),
    };
    Ok((event, config_handle))
}

pub(crate) fn write_packet(
    arena: &mut MemoryArena,
    packet: &MediaPacket<'static>,
) -> Result<(DemuxEvent, Option<Handle>), AbiError> {
    let payload = packet.payload.as_ref();
    let (data_handle, data_desc) = store_or_empty(arena, payload)?;

    let event = DemuxEvent {
        kind: DemuxEventKind::Packet,
        track_id: packet.track_id.get(),
        track_kind: 0,
        codec: 255,
        width: 0,
        height: 0,
        sample_rate: 0,
        channels: 0,
        config_slot: 0,
        config_generation: 0,
        config_offset: 0,
        config_len: 0,
        data_slot: data_handle.as_ref().map_or(0, |h| h.slot),
        data_generation: data_handle.as_ref().map_or(0, |h| h.generation),
        data_offset: data_desc.offset,
        data_len: if data_handle.is_some() {
            data_desc.length
        } else {
            0
        },
        pts_ms: ticks_to_ms(packet.time, packet.time.pts),
        dts_ms: ticks_to_ms(packet.time, packet.time.dts),
        duration_ms: ticks_to_ms(packet.time, packet.time.duration),
        flags: if packet.flags.is_keyframe { 1 } else { 0 },
        error_code: 0,
        error_message: String::new(),
    };
    Ok((event, data_handle))
}

pub(crate) fn track_event(
    arena: &mut MemoryArena,
    track: TrackInfo,
    last_config: &mut Option<Handle>,
) -> Result<DemuxEvent, AbiError> {
    if let Some(h) = last_config.take() {
        let _ = arena.release(h);
    }
    let (event, handle) = write_track(arena, &track)?;
    *last_config = handle;
    Ok(event)
}

pub(crate) fn packet_event(
    arena: &mut MemoryArena,
    packet: MediaPacket<'static>,
    last_data: &mut Option<Handle>,
) -> Result<DemuxEvent, AbiError> {
    if let Some(h) = last_data.take() {
        let _ = arena.release(h);
    }
    let (event, handle) = write_packet(arena, &packet)?;
    *last_data = handle;
    Ok(event)
}
