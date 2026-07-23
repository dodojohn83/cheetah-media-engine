import { describe, it, expect, vi } from 'vitest';
import {
  FfmpegPackImpl,
  FfmpegPackError,
  PackReturnCode,
  type FfmpegPackModule,
  type FfmpegPacket,
} from './ffmpeg-pack';

const PACK_ABI_VERSION_0_1 = (0 << 16) | 1;
const PACK_ABI_VERSION_0_0 = (0 << 16) | 0;
const PACK_ABI_VERSION_1_0 = (1 << 16) | 0;

function makeFakeHeap(size = 65536): Uint8Array {
  const buffer = new ArrayBuffer(size);
  return new Uint8Array(buffer);
}

function makeFakeModule(overrides?: {
  abiVersion?: number;
  initResult?: number;
  configureResult?: number;
  sendResult?: number;
  receiveResult?: number;
}): FfmpegPackModule {
  const heap = makeFakeHeap();
  const allocations = new Map<number, number>();
  let nextPtr = 8;

  const malloc = vi.fn((size: number): number => {
    const ptr = nextPtr;
    nextPtr += size + (8 - (size % 8));
    allocations.set(ptr, size);
    return ptr;
  });
  const free = vi.fn((ptr: number): void => {
    allocations.delete(ptr);
  });

  return {
    HEAPU8: heap,
    _malloc: malloc,
    _free: free,
    _cheetah_pack_abi_version: vi.fn(() => overrides?.abiVersion ?? PACK_ABI_VERSION_0_1),
    _cheetah_pack_init: vi.fn(() => overrides?.initResult ?? PackReturnCode.Ok),
    _cheetah_pack_configure_track: vi.fn(() => overrides?.configureResult ?? PackReturnCode.Ok),
    _cheetah_pack_send_packet: vi.fn(() => overrides?.sendResult ?? PackReturnCode.Ok),
    _cheetah_pack_receive_frame: vi.fn(() => overrides?.receiveResult ?? PackReturnCode.Eof),
    _cheetah_pack_flush: vi.fn(() => PackReturnCode.Ok),
    _cheetah_pack_close: vi.fn(() => PackReturnCode.Ok),
  };
}

describe('FfmpegPackImpl', () => {
  it('reads ABI version from the module', () => {
    const module = makeFakeModule();
    const pack = new FfmpegPackImpl(module);
    expect(pack.abiMajor).toBe(0);
    expect(pack.abiMinor).toBe(1);
    expect(module._cheetah_pack_abi_version).toHaveBeenCalled();
  });

  it('throws on ABI major mismatch', () => {
    const module = makeFakeModule({ abiVersion: PACK_ABI_VERSION_1_0 });
    expect(() => new FfmpegPackImpl(module)).toThrow(FfmpegPackError);
  });

  it('throws on ABI minor older than loader', () => {
    const module = makeFakeModule({ abiVersion: PACK_ABI_VERSION_0_0 });
    expect(() => new FfmpegPackImpl(module)).toThrow(FfmpegPackError);
  });

  it('initializes the pack once and then reuses state', async () => {
    const module = makeFakeModule();
    const pack = new FfmpegPackImpl(module);
    await pack.init(0);
    await pack.init(0);
    expect(module._cheetah_pack_init).toHaveBeenCalledTimes(1);
  });

  it('throws when init fails', async () => {
    const module = makeFakeModule({ initResult: PackReturnCode.Error });
    const pack = new FfmpegPackImpl(module);
    await expect(pack.init(0)).rejects.toThrow('pack init failed');
  });

  it('configures a track lazily initializing the pack', async () => {
    const module = makeFakeModule();
    const pack = new FfmpegPackImpl(module);
    await pack.configureTrack(0, 'h264');
    expect(module._cheetah_pack_init).toHaveBeenCalled();
    expect(module._cheetah_pack_configure_track).toHaveBeenCalledWith(0, 0, 0, 0);
  });

  it('throws unsupported when configure returns unsupported', async () => {
    const module = makeFakeModule({ configureResult: PackReturnCode.Unsupported });
    const pack = new FfmpegPackImpl(module);
    await expect(pack.configureTrack(1, 'h265')).rejects.toThrow(FfmpegPackError);
  });

  it('throws for unknown codec names', async () => {
    const module = makeFakeModule();
    const pack = new FfmpegPackImpl(module);
    await expect(pack.configureTrack(0, 'av1')).rejects.toThrow('av1');
  });

  it('sends a packet and frees all allocated buffers', async () => {
    const module = makeFakeModule();
    const pack = new FfmpegPackImpl(module);
    await pack.send({
      trackIndex: 0,
      payload: new Uint8Array([1, 2, 3]),
      sideData: new Uint8Array([4, 5]),
      ptsMs: 100n,
    });
    expect(module._cheetah_pack_send_packet).toHaveBeenCalled();
    expect(module._free).toHaveBeenCalled();
  });

  it('returns undefined when receive returns EOF', async () => {
    const module = makeFakeModule({ receiveResult: PackReturnCode.Eof });
    const pack = new FfmpegPackImpl(module);
    const frame = await pack.receive(0);
    expect(frame).toBeUndefined();
  });

  it('throws when receive fails with an error code', async () => {
    const module = makeFakeModule({ receiveResult: PackReturnCode.Error });
    const pack = new FfmpegPackImpl(module);
    await expect(pack.receive(0)).rejects.toThrow('receive failed');
  });

  it('flushes and closes cleanly', async () => {
    const module = makeFakeModule();
    const pack = new FfmpegPackImpl(module);
    await pack.configureTrack(0, 'aac');
    await pack.flush(0);
    await pack.close();
    expect(module._cheetah_pack_close).toHaveBeenCalled();
    await expect(pack.send({ trackIndex: 0, payload: new Uint8Array(0), ptsMs: 0n })).rejects.toThrow(
      'pack is closed',
    );
  });

  it('throws for invalid send packets', async () => {
    const module = makeFakeModule();
    const pack = new FfmpegPackImpl(module);
    const valid: FfmpegPacket = { trackIndex: 0, payload: new Uint8Array([1, 2, 3]), ptsMs: 100n };

    await expect(pack.send(null as unknown as FfmpegPacket)).rejects.toThrow('packet must be an object');
    await expect(pack.send({ ...valid, trackIndex: -1 })).rejects.toThrow('trackIndex');
    await expect(pack.send({ ...valid, payload: 'bytes' as unknown as Uint8Array })).rejects.toThrow(
      'payload',
    );
    await expect(pack.send({ ...valid, sideData: 'bytes' as unknown as Uint8Array })).rejects.toThrow(
      'sideData',
    );
    await expect(pack.send({ ...valid, ptsMs: 100 as unknown as bigint })).rejects.toThrow('ptsMs');
    await expect(pack.send({ ...valid, dtsMs: 100 as unknown as bigint })).rejects.toThrow('dtsMs');
    await expect(pack.send({ ...valid, durationMs: 100 as unknown as bigint })).rejects.toThrow(
      'durationMs',
    );
    await expect(pack.send({ ...valid, epoch: 100 as unknown as bigint })).rejects.toThrow('epoch');
    await expect(pack.send({ ...valid, flags: -1 })).rejects.toThrow('flags');
  });
});
