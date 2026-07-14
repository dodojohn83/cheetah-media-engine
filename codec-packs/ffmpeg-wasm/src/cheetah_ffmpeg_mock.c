/*
 * Mock codec pack for the build-system PR.
 *
 * This file implements the stable cheetah-ffmpeg-wasm ABI with every decode
 * operation returning CHEETAH_UNSUPPORTED. It is used to exercise the loader,
 * manifest/hash checks and the runtime fallback chain before the real FFmpeg
 * decoder shim lands.
 */

#include "cheetah_ffmpeg.h"

#include <stdint.h>

int cheetah_pack_abi_version(void) {
  return (CHEETAH_PACK_ABI_MAJOR << 16) | CHEETAH_PACK_ABI_MINOR;
}

int cheetah_pack_init(uint32_t max_memory_mb, uint32_t flags) {
  (void)max_memory_mb;
  (void)flags;
  return CHEETAH_OK;
}

int cheetah_pack_configure_track(uint32_t track_index, uint8_t codec,
                                 const uint8_t* config, uint32_t config_len) {
  (void)track_index;
  (void)codec;
  (void)config;
  (void)config_len;
  return CHEETAH_UNSUPPORTED;
}

int cheetah_pack_send_packet(const CheetahPacketDescriptor* packet) {
  (void)packet;
  return CHEETAH_UNSUPPORTED;
}

int cheetah_pack_receive_frame(uint32_t track_index, CheetahFrameDescriptor* out) {
  (void)track_index;
  (void)out;
  return CHEETAH_EOF;
}

int cheetah_pack_flush(uint32_t track_index) {
  (void)track_index;
  return CHEETAH_OK;
}

int cheetah_pack_close(void) {
  return CHEETAH_OK;
}
