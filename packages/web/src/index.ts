import { createRuntime, type EngineRuntime } from '@cheetah-media/runtime';

export {
  ABI_VERSION_MAJOR,
  ABI_VERSION_MINOR,
  ABI_VERSION,
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

export const enum AbiFeatureFlags {
  Threads = 1 << 0,
  Simd = 1 << 1,
  SharedArrayBuffer = 1 << 2,
}

export interface Player {
  load(url: string, options?: { isLive?: boolean }): Promise<void>;
  play(): void;
  pause(): void;
  stop(): Promise<void>;
  destroy(): Promise<void>;
}

export interface PlayerOptions {
  workerUrl?: string | undefined;
  wasmUrl?: string | undefined;
}

export function createPlayer(options: PlayerOptions = {}): Player & EngineRuntime {
  const runtimeOptions: import('@cheetah-media/runtime').RuntimeOptions = {};
  if (options.workerUrl !== undefined) runtimeOptions.workerUrl = options.workerUrl;
  if (options.wasmUrl !== undefined) runtimeOptions.wasmUrl = options.wasmUrl;
  const runtime = createRuntime(runtimeOptions);
  const player: Player & EngineRuntime = {
    get version() {
      return runtime.version;
    },
    load: runtime.load.bind(runtime),
    play: runtime.play.bind(runtime),
    pause: runtime.pause.bind(runtime),
    stop: runtime.stop.bind(runtime),
    destroy: runtime.destroy.bind(runtime),
  };
  runtime.onEvent = (event, details) => player.onEvent?.(event, details);
  runtime.onError = (error) => player.onError?.(error);
  return player;
}
