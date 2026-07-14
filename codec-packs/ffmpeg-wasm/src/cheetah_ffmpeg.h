#ifndef CHEETAH_FFMPEG_H
#define CHEETAH_FFMPEG_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Stable ABI for the cheetah-ffmpeg-wasm codec pack.
 *
 * C structure layout intentionally matches the Rust/JS ABI descriptors in
 * cheetah-media-abi so that the JS loader can read and write them directly
 * inside the WASM linear memory.
 */

#define CHEETAH_PACK_ABI_MAJOR 0
#define CHEETAH_PACK_ABI_MINOR 1

/* Return codes */
#define CHEETAH_OK          0
#define CHEETAH_EAGAIN      1
#define CHEETAH_UNSUPPORTED 2
#define CHEETAH_EOF         3
#define CHEETAH_ERROR       4

/* Feature flags */
#define CHEETAH_FEATURE_THREADS       (1u << 0)
#define CHEETAH_FEATURE_SIMD          (1u << 1)
#define CHEETAH_FEATURE_SHARED_MEMORY (1u << 2)

/* Codec ids - keep in sync with JS/Rust enum order */
enum cheetah_codec_id {
  CHEETAH_CODEC_H264 = 0,
  CHEETAH_CODEC_H265 = 1,
  CHEETAH_CODEC_AAC  = 2,
  CHEETAH_CODEC_G711A = 3,
  CHEETAH_CODEC_G711U = 4,
  CHEETAH_CODEC_MP3  = 5,
};

/* Memory region descriptor (40 bytes, 8-byte aligned) */
typedef struct CheetahMemoryDescriptor {
  uint32_t region;      /* 0 */
  uint32_t _padding0;   /* 4 */
  uint64_t offset;      /* 8 */
  uint32_t length;      /* 16 */
  uint32_t capacity;    /* 20 */
  uint64_t generation; /* 24 */
  uint32_t flags;       /* 32 */
  uint32_t _padding1;   /* 36 */
} CheetahMemoryDescriptor;

/* Packet descriptor (128 bytes, 8-byte aligned) */
typedef struct CheetahPacketDescriptor {
  uint32_t track_index;                  /* 0 */
  uint32_t _padding0;                    /* 4 */
  CheetahMemoryDescriptor payload;       /* 8 */
  CheetahMemoryDescriptor side_data;     /* 48 */
  int64_t pts_ms;                        /* 88 */
  int64_t dts_ms;                        /* 96 */
  int64_t duration_ms;                   /* 104 */
  uint32_t flags;                        /* 112 */
  uint32_t _padding1;                    /* 116 */
  uint64_t epoch;                        /* 120 */
} CheetahPacketDescriptor;

/* Frame descriptor (288 bytes, 8-byte aligned) */
typedef struct CheetahFrameDescriptor {
  uint32_t track_index;                   /* 0 */
  uint32_t _padding0;                     /* 4 */
  CheetahMemoryDescriptor payload;        /* 8 */
  CheetahMemoryDescriptor planes[4];      /* 48 */
  CheetahMemoryDescriptor side_data;      /* 208 */
  uint32_t width;                         /* 248 */
  uint32_t height;                        /* 252 */
  int64_t pts_ms;                         /* 256 */
  int64_t duration_ms;                    /* 264 */
  uint32_t flags;                         /* 272 */
  uint32_t _padding1;                     /* 276 */
  uint64_t epoch;                         /* 280 */
} CheetahFrameDescriptor;

/* Pack lifecycle and control */
int cheetah_pack_abi_version(void);
int cheetah_pack_init(uint32_t max_memory_mb, uint32_t flags);
int cheetah_pack_configure_track(uint32_t track_index, uint8_t codec,
                                 const uint8_t* config, uint32_t config_len);
int cheetah_pack_send_packet(const CheetahPacketDescriptor* packet);
int cheetah_pack_receive_frame(uint32_t track_index, CheetahFrameDescriptor* out);
int cheetah_pack_flush(uint32_t track_index);
int cheetah_pack_close(void);

#ifdef __cplusplus
}
#endif

#endif /* CHEETAH_FFMPEG_H */
