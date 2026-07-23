/**
 * JS loader for the cheetah-ffmpeg-wasm codec pack.
 *
 * The loader is intentionally thin: it only knows the stable pack ABI (version,
 * init/configure/send/receive/flush/close) and reads/writes the same
 * descriptor layout that the Rust engine uses.  FFmpeg internals never leak
 * past this file.
 */

import {
  ABI_VERSION_MAJOR,
  ABI_VERSION_MINOR,
  MEMORY_DESCRIPTOR_SIZE,
  MEMORY_DESCRIPTOR_OFFSET_REGION,
  MEMORY_DESCRIPTOR_OFFSET_OFFSET,
  MEMORY_DESCRIPTOR_OFFSET_LENGTH,
  MEMORY_DESCRIPTOR_OFFSET_CAPACITY,
  MEMORY_DESCRIPTOR_OFFSET_GENERATION,
  MEMORY_DESCRIPTOR_OFFSET_FLAGS,
  PACKET_DESCRIPTOR_SIZE,
  PACKET_DESCRIPTOR_OFFSET_TRACK_INDEX,
  PACKET_DESCRIPTOR_OFFSET_PAYLOAD,
  PACKET_DESCRIPTOR_OFFSET_SIDE_DATA,
  PACKET_DESCRIPTOR_OFFSET_PTS_MS,
  PACKET_DESCRIPTOR_OFFSET_DTS_MS,
  PACKET_DESCRIPTOR_OFFSET_DURATION_MS,
  PACKET_DESCRIPTOR_OFFSET_FLAGS,
  PACKET_DESCRIPTOR_OFFSET_EPOCH,
  FRAME_DESCRIPTOR_SIZE,
  FRAME_DESCRIPTOR_OFFSET_TRACK_INDEX,
  FRAME_DESCRIPTOR_OFFSET_PAYLOAD,
  FRAME_DESCRIPTOR_OFFSET_PLANES,
  FRAME_DESCRIPTOR_OFFSET_SIDE_DATA,
  FRAME_DESCRIPTOR_OFFSET_WIDTH,
  FRAME_DESCRIPTOR_OFFSET_HEIGHT,
  FRAME_DESCRIPTOR_OFFSET_PTS_MS,
  FRAME_DESCRIPTOR_OFFSET_DURATION_MS,
  FRAME_DESCRIPTOR_OFFSET_FLAGS,
  FRAME_DESCRIPTOR_OFFSET_EPOCH,
} from './abi-constants';

export type FfmpegPackErrorCode =
  | 'unsupported'
  | 'not-initialized'
  | 'memory'
  | 'abi-mismatch'
  | 'send-failed'
  | 'receive-failed'
  | 'closed';

export class FfmpegPackError extends Error {
  readonly code: FfmpegPackErrorCode;

  constructor(code: FfmpegPackErrorCode, message: string) {
    super(message);
    this.name = 'FfmpegPackError';
    this.code = code;
  }
}

export const enum PackReturnCode {
  Ok = 0,
  Eagain = 1,
  Unsupported = 2,
  Eof = 3,
  Error = 4,
}

export const enum PackFeatureFlags {
  Threads = 1 << 0,
  Simd = 1 << 1,
  SharedMemory = 1 << 2,
}

export const enum PackCodecId {
  H264 = 0,
  H265 = 1,
  Aac = 2,
  G711A = 3,
  G711U = 4,
  Mp3 = 5,
}

const CODEC_NAME_TO_ID: Record<string, PackCodecId | undefined> = {
  h264: PackCodecId.H264,
  h265: PackCodecId.H265,
  hevc: PackCodecId.H265,
  aac: PackCodecId.Aac,
  g711a: PackCodecId.G711A,
  alaw: PackCodecId.G711A,
  g711u: PackCodecId.G711U,
  ulaw: PackCodecId.G711U,
  mp3: PackCodecId.Mp3,
};

export interface MemoryDescriptor {
  readonly region: number;
  readonly offset: bigint;
  readonly length: number;
  readonly capacity: number;
  readonly generation: bigint;
  readonly flags: number;
}

export interface PacketDescriptor {
  readonly trackIndex: number;
  readonly payload: MemoryDescriptor;
  readonly sideData: MemoryDescriptor;
  readonly ptsMs: bigint;
  readonly dtsMs: bigint;
  readonly durationMs: bigint;
  readonly flags: number;
  readonly epoch: bigint;
}

export interface FrameDescriptor {
  readonly trackIndex: number;
  readonly payload: MemoryDescriptor;
  readonly planes: readonly MemoryDescriptor[];
  readonly sideData: MemoryDescriptor;
  readonly width: number;
  readonly height: number;
  readonly ptsMs: bigint;
  readonly durationMs: bigint;
  readonly flags: number;
  readonly epoch: bigint;
}

export interface FfmpegPackOptions {
  readonly maxMemoryMB?: number;
}

export interface FfmpegPacket {
  readonly trackIndex: number;
  readonly payload: Uint8Array;
  readonly sideData?: Uint8Array | undefined;
  readonly ptsMs: bigint;
  readonly dtsMs?: bigint | undefined;
  readonly durationMs?: bigint | undefined;
  readonly flags?: number | undefined;
  readonly epoch?: bigint | undefined;
}

export interface FfmpegPackModule {
  readonly HEAPU8: Uint8Array;
  _malloc(size: number): number;
  _free(ptr: number): void;
  _cheetah_pack_abi_version(): number;
  _cheetah_pack_init(maxMemoryMB: number, flags: number): number;
  _cheetah_pack_configure_track(
    trackIndex: number,
    codec: number,
    configPtr: number,
    configLen: number,
  ): number;
  _cheetah_pack_send_packet(packetPtr: number): number;
  _cheetah_pack_receive_frame(trackIndex: number, outPtr: number): number;
  _cheetah_pack_flush(trackIndex: number): number;
  _cheetah_pack_close(): number;
}

function getCodecId(name: string): PackCodecId {
  const id = CODEC_NAME_TO_ID[name.toLowerCase()];
  if (id === undefined) {
    throw new FfmpegPackError('unsupported', `codec ${name} not mapped to pack codec id`);
  }
  return id;
}

function dataViewAt(heap: Uint8Array, offset: number, size: number): DataView {
  return new DataView(heap.buffer, offset, size);
}

function writeMemoryDescriptor(heap: Uint8Array, base: number, desc: MemoryDescriptor): void {
  const view = dataViewAt(heap, base, MEMORY_DESCRIPTOR_SIZE);
  heap.fill(0, base, base + MEMORY_DESCRIPTOR_SIZE);
  view.setUint32(MEMORY_DESCRIPTOR_OFFSET_REGION, desc.region, true);
  view.setBigUint64(MEMORY_DESCRIPTOR_OFFSET_OFFSET, desc.offset, true);
  view.setUint32(MEMORY_DESCRIPTOR_OFFSET_LENGTH, desc.length, true);
  view.setUint32(MEMORY_DESCRIPTOR_OFFSET_CAPACITY, desc.capacity, true);
  view.setBigUint64(MEMORY_DESCRIPTOR_OFFSET_GENERATION, desc.generation, true);
  view.setUint32(MEMORY_DESCRIPTOR_OFFSET_FLAGS, desc.flags, true);
}

function readMemoryDescriptor(heap: Uint8Array, base: number): MemoryDescriptor {
  const view = dataViewAt(heap, base, MEMORY_DESCRIPTOR_SIZE);
  return {
    region: view.getUint32(MEMORY_DESCRIPTOR_OFFSET_REGION, true),
    offset: view.getBigUint64(MEMORY_DESCRIPTOR_OFFSET_OFFSET, true),
    length: view.getUint32(MEMORY_DESCRIPTOR_OFFSET_LENGTH, true),
    capacity: view.getUint32(MEMORY_DESCRIPTOR_OFFSET_CAPACITY, true),
    generation: view.getBigUint64(MEMORY_DESCRIPTOR_OFFSET_GENERATION, true),
    flags: view.getUint32(MEMORY_DESCRIPTOR_OFFSET_FLAGS, true),
  };
}

function writePacketDescriptor(heap: Uint8Array, base: number, packet: PacketDescriptor): void {
  const view = dataViewAt(heap, base, PACKET_DESCRIPTOR_SIZE);
  heap.fill(0, base, base + PACKET_DESCRIPTOR_SIZE);
  view.setUint32(PACKET_DESCRIPTOR_OFFSET_TRACK_INDEX, packet.trackIndex, true);
  writeMemoryDescriptor(heap, base + PACKET_DESCRIPTOR_OFFSET_PAYLOAD, packet.payload);
  writeMemoryDescriptor(heap, base + PACKET_DESCRIPTOR_OFFSET_SIDE_DATA, packet.sideData);
  view.setBigInt64(PACKET_DESCRIPTOR_OFFSET_PTS_MS, packet.ptsMs, true);
  view.setBigInt64(PACKET_DESCRIPTOR_OFFSET_DTS_MS, packet.dtsMs, true);
  view.setBigInt64(PACKET_DESCRIPTOR_OFFSET_DURATION_MS, packet.durationMs, true);
  view.setUint32(PACKET_DESCRIPTOR_OFFSET_FLAGS, packet.flags, true);
  view.setBigUint64(PACKET_DESCRIPTOR_OFFSET_EPOCH, packet.epoch, true);
}

function readFrameDescriptor(heap: Uint8Array, base: number): FrameDescriptor {
  const view = dataViewAt(heap, base, FRAME_DESCRIPTOR_SIZE);
  const planes: MemoryDescriptor[] = [];
  for (let i = 0; i < 4; i += 1) {
    const planeBase = base + FRAME_DESCRIPTOR_OFFSET_PLANES + i * MEMORY_DESCRIPTOR_SIZE;
    if (planeBase + MEMORY_DESCRIPTOR_SIZE <= heap.byteLength) {
      planes.push(readMemoryDescriptor(heap, planeBase));
    }
  }
  return {
    trackIndex: view.getUint32(FRAME_DESCRIPTOR_OFFSET_TRACK_INDEX, true),
    payload: readMemoryDescriptor(heap, base + FRAME_DESCRIPTOR_OFFSET_PAYLOAD),
    planes,
    sideData: readMemoryDescriptor(heap, base + FRAME_DESCRIPTOR_OFFSET_SIDE_DATA),
    width: view.getUint32(FRAME_DESCRIPTOR_OFFSET_WIDTH, true),
    height: view.getUint32(FRAME_DESCRIPTOR_OFFSET_HEIGHT, true),
    ptsMs: view.getBigInt64(FRAME_DESCRIPTOR_OFFSET_PTS_MS, true),
    durationMs: view.getBigInt64(FRAME_DESCRIPTOR_OFFSET_DURATION_MS, true),
    flags: view.getUint32(FRAME_DESCRIPTOR_OFFSET_FLAGS, true),
    epoch: view.getBigUint64(FRAME_DESCRIPTOR_OFFSET_EPOCH, true),
  };
}

function isUint8Array(value: unknown): value is Uint8Array {
  if (typeof Uint8Array !== 'undefined' && value instanceof Uint8Array) return true;
  return Object.prototype.toString.call(value) === '[object Uint8Array]';
}

function validateBigint(value: unknown, name: string): void {
  if (typeof value !== 'bigint') {
    throw new FfmpegPackError('send-failed', `${name} must be a bigint`);
  }
}

function validateFfmpegPacket(packet: unknown): asserts packet is FfmpegPacket {
  if (!packet || typeof packet !== 'object') {
    throw new FfmpegPackError('send-failed', 'packet must be an object');
  }
  const p = packet as Partial<FfmpegPacket>;
  if (typeof p.trackIndex !== 'number' || !Number.isInteger(p.trackIndex) || p.trackIndex < 0) {
    throw new FfmpegPackError('send-failed', 'packet.trackIndex must be a non-negative integer');
  }
  if (!isUint8Array(p.payload)) {
    throw new FfmpegPackError('send-failed', 'packet.payload must be a Uint8Array');
  }
  if (p.sideData !== undefined && !isUint8Array(p.sideData)) {
    throw new FfmpegPackError('send-failed', 'packet.sideData must be a Uint8Array');
  }
  if (p.ptsMs === undefined) {
    throw new FfmpegPackError('send-failed', 'packet.ptsMs is required');
  }
  validateBigint(p.ptsMs, 'packet.ptsMs');
  if (p.dtsMs !== undefined) {
    validateBigint(p.dtsMs, 'packet.dtsMs');
  }
  if (p.durationMs !== undefined) {
    validateBigint(p.durationMs, 'packet.durationMs');
  }
  if (p.epoch !== undefined) {
    validateBigint(p.epoch, 'packet.epoch');
  }
  if (p.flags !== undefined) {
    if (typeof p.flags !== 'number' || !Number.isInteger(p.flags) || p.flags < 0) {
      throw new FfmpegPackError('send-failed', 'packet.flags must be a non-negative integer');
    }
  }
}

export interface FfmpegPack {
  readonly abiMajor: number;
  readonly abiMinor: number;
  init(variantFlags: number): Promise<void>;
  configureTrack(trackIndex: number, codec: string, config?: Uint8Array | undefined): Promise<void>;
  send(packet: FfmpegPacket): Promise<void>;
  receive(trackIndex: number): Promise<FrameDescriptor | undefined>;
  flush(trackIndex: number): Promise<void>;
  close(): Promise<void>;
}

export class FfmpegPackImpl implements FfmpegPack {
  private readonly module: FfmpegPackModule;
  private readonly maxMemoryMB: number;
  private initialized = false;
  private closed = false;

  constructor(module: FfmpegPackModule, options: FfmpegPackOptions = {}) {
    this.module = module;
    this.maxMemoryMB = options.maxMemoryMB ?? 128;
    if (!Number.isInteger(this.maxMemoryMB) || this.maxMemoryMB < 1) {
      throw new FfmpegPackError('not-initialized', 'maxMemoryMB must be a finite positive integer');
    }
    const version = module._cheetah_pack_abi_version();
    this.abiMajor = (version >> 16) & 0xffff;
    this.abiMinor = version & 0xffff;
    if (this.abiMajor !== ABI_VERSION_MAJOR || this.abiMinor < ABI_VERSION_MINOR) {
      throw new FfmpegPackError(
        'abi-mismatch',
        `pack ABI ${this.abiMajor}.${this.abiMinor} is not compatible with loader ${ABI_VERSION_MAJOR}.${ABI_VERSION_MINOR}`,
      );
    }
  }

  readonly abiMajor: number;
  readonly abiMinor: number;

  get heap(): Uint8Array {
    return this.module.HEAPU8;
  }

  private checkNotClosed(): void {
    if (this.closed) throw new FfmpegPackError('closed', 'pack is closed');
  }

  async init(variantFlags: number): Promise<void> {
    this.checkNotClosed();
    if (this.initialized) return;
    const result = this.module._cheetah_pack_init(this.maxMemoryMB, variantFlags);
    if (result !== PackReturnCode.Ok) {
      throw new FfmpegPackError('not-initialized', `pack init failed with code ${result}`);
    }
    this.initialized = true;
  }

  async configureTrack(
    trackIndex: number,
    codec: string,
    config?: Uint8Array | undefined,
  ): Promise<void> {
    this.checkNotClosed();
    if (!this.initialized) await this.init(0);
    const codecId = getCodecId(codec);
    const cfg = config ?? new Uint8Array(0);
    let configPtr = 0;
    if (cfg.length > 0) {
      configPtr = this.module._malloc(cfg.length);
      if (!configPtr) throw new FfmpegPackError('memory', 'failed to allocate config buffer');
      this.heap.set(cfg, configPtr);
    }
    try {
      const result = this.module._cheetah_pack_configure_track(trackIndex, codecId, configPtr, cfg.length);
      if (result === PackReturnCode.Unsupported) {
        throw new FfmpegPackError('unsupported', `codec ${codec} is not supported by this pack`);
      }
      if (result !== PackReturnCode.Ok) {
        throw new FfmpegPackError('unsupported', `configureTrack failed with code ${result}`);
      }
    } finally {
      if (configPtr) this.module._free(configPtr);
    }
  }

  async send(packet: FfmpegPacket): Promise<void> {
    this.checkNotClosed();
    validateFfmpegPacket(packet);
    if (!this.initialized) await this.init(0);

    const payloadPtr = this.copyToHeap(packet.payload);
    const sideData = packet.sideData ?? new Uint8Array(0);
    const sideDataPtr = sideData.length > 0 ? this.copyToHeap(sideData) : 0;

    const descPtr = this.module._malloc(PACKET_DESCRIPTOR_SIZE);
    if (!descPtr) {
      if (payloadPtr) this.module._free(payloadPtr);
      if (sideDataPtr) this.module._free(sideDataPtr);
      throw new FfmpegPackError('memory', 'failed to allocate packet descriptor');
    }

    try {
      const now = 0n;
      writePacketDescriptor(this.heap, descPtr, {
        trackIndex: packet.trackIndex,
        payload: {
          region: 0,
          offset: BigInt(payloadPtr),
          length: packet.payload.length,
          capacity: packet.payload.length,
          generation: 0n,
          flags: 0,
        },
        sideData: {
          region: 0,
          offset: sideDataPtr ? BigInt(sideDataPtr) : 0n,
          length: sideData.length,
          capacity: sideData.length,
          generation: 0n,
          flags: 0,
        },
        ptsMs: packet.ptsMs,
        dtsMs: packet.dtsMs ?? packet.ptsMs,
        durationMs: packet.durationMs ?? now,
        flags: packet.flags ?? 0,
        epoch: packet.epoch ?? now,
      });

      const result = this.module._cheetah_pack_send_packet(descPtr);
      if (result === PackReturnCode.Unsupported) {
        throw new FfmpegPackError('unsupported', 'send packet is not supported by this pack');
      }
      if (result !== PackReturnCode.Ok && result !== PackReturnCode.Eagain) {
        throw new FfmpegPackError('send-failed', `send failed with code ${result}`);
      }
    } finally {
      this.module._free(descPtr);
      if (payloadPtr) this.module._free(payloadPtr);
      if (sideDataPtr) this.module._free(sideDataPtr);
    }
  }

  async receive(trackIndex: number): Promise<FrameDescriptor | undefined> {
    this.checkNotClosed();
    if (!this.initialized) await this.init(0);

    const outPtr = this.module._malloc(FRAME_DESCRIPTOR_SIZE);
    if (!outPtr) throw new FfmpegPackError('memory', 'failed to allocate frame descriptor');
    try {
      const result = this.module._cheetah_pack_receive_frame(trackIndex, outPtr);
      if (result === PackReturnCode.Eof) return undefined;
      if (result === PackReturnCode.Eagain) return undefined;
      if (result !== PackReturnCode.Ok) {
        throw new FfmpegPackError('receive-failed', `receive failed with code ${result}`);
      }
      return readFrameDescriptor(this.heap, outPtr);
    } finally {
      this.module._free(outPtr);
    }
  }

  async flush(trackIndex: number): Promise<void> {
    this.checkNotClosed();
    if (!this.initialized) return;
    const result = this.module._cheetah_pack_flush(trackIndex);
    if (result !== PackReturnCode.Ok) {
      throw new FfmpegPackError('receive-failed', `flush failed with code ${result}`);
    }
  }

  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    if (this.initialized) {
      this.module._cheetah_pack_close();
    }
  }

  private copyToHeap(data: Uint8Array): number {
    if (data.length === 0) return 0;
    const ptr = this.module._malloc(data.length);
    if (!ptr) throw new FfmpegPackError('memory', 'failed to allocate heap buffer');
    this.heap.set(data, ptr);
    return ptr;
  }
}
