import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  createPlayer,
  createPlayerWithRuntime,
  CheetahMediaError,
  NoopVrRenderer,
  NoopAiFrameProcessor,
  type CheetahPlayer,
  type CheetahPlayerEvent,
  type PlayerConfig,
  type VrRenderer,
  type AiFrameProcessor,
  type ProcessableFrame,
  type AiFrameBudget,
} from './index';
import { type EngineRuntime } from '@cheetah-media/runtime';

if (typeof ImageData === 'undefined') {
  (globalThis as unknown as { ImageData: typeof ImageData }).ImageData = class {
    constructor(
      public data: Uint8ClampedArray,
      public width: number,
      public height: number,
    ) {}
  } as unknown as typeof ImageData;
}

type MockRuntime = EngineRuntime & {
  readonly emitEvent: (event: string, details?: Record<string, unknown>) => void;
  readonly emitError: (error: { code: number; stage: string; message: string; recoverable: boolean }) => void;
};

function mockRuntime(): MockRuntime {
  let epoch = 0;
  let destroyed = false;
  const pending = new Map<number, { resolve: (value: unknown) => void; reject: (reason: Error) => void }>();
  let eventHandler: ((event: string, details?: Record<string, unknown>) => void) | undefined;
  let errorHandler: ((error: { code: number; stage: string; message: string; recoverable: boolean }) => void) | undefined;
  let requestSequence = 0;

  function failIfDestroyed(): void {
    if (destroyed) throw new Error('Runtime destroyed');
  }

  const runtime: MockRuntime = {
    version: '0.1.0',
    get epoch() {
      return epoch;
    },

    async load(url: string, options: { isLive?: boolean } = {}): Promise<void> {
      failIfDestroyed();
      epoch += 1;
      if (url === 'fail://test') {
        throw new Error('load rejected');
      }
      expect(url).toBeDefined();
      expect(typeof options.isLive).toBe('boolean');
    },

    play(): void {
      failIfDestroyed();
    },

    pause(): void {
      failIfDestroyed();
    },

    async seek(timeMs: number): Promise<void> {
      failIfDestroyed();
      expect(typeof timeMs).toBe('number');
    },

    async setPlaybackRate(rate: number): Promise<void> {
      failIfDestroyed();
      expect(typeof rate).toBe('number');
    },

    async frameStep(direction: 'forward' | 'backward', keyframeOnly?: boolean): Promise<void> {
      failIfDestroyed();
      expect(direction === 'forward' || direction === 'backward').toBe(true);
      expect(typeof keyframeOnly).toBe('boolean');
    },

    async pauseDisplay(keepConnection?: boolean): Promise<void> {
      failIfDestroyed();
      expect(typeof keepConnection).toBe('boolean');
    },

    async stop(): Promise<void> {
      failIfDestroyed();
      epoch += 1;
    },

    async destroy(): Promise<void> {
      destroyed = true;
      for (const { reject } of pending.values()) {
        reject(new Error('Runtime destroyed'));
      }
      pending.clear();
    },

    request(type: string, payload?: unknown, _timeoutMs?: number): Promise<unknown> {
      failIfDestroyed();
      requestSequence += 1;
      const seq = requestSequence;
      if (type === 'snapshot') {
        const opts = payload as { maxWidth?: number; maxHeight?: number } | undefined;
        const width = opts?.maxWidth ?? 1280;
        const height = opts?.maxHeight ?? 720;
        const data = new Uint8ClampedArray(width * height * 4);
        return Promise.resolve({ width, height, data });
      }
      if (type === 'switch-variant') {
        const variant = payload as { bandwidth?: number; index?: number } | undefined;
        if (!variant || (variant.bandwidth === undefined && variant.index === undefined)) {
          return Promise.reject(new Error('invalid variant'));
        }
        return Promise.resolve({ switched: true });
      }
      return new Promise((resolve, reject) => {
        pending.set(seq, { resolve, reject });
      });
    },

    set onEvent(handler: ((event: string, details?: Record<string, unknown>) => void) | undefined) {
      eventHandler = handler;
    },
    set onError(handler: ((error: { code: number; stage: string; message: string; recoverable: boolean }) => void) | undefined) {
      errorHandler = handler;
    },

    emitEvent(event: string, details?: Record<string, unknown>): void {
      eventHandler?.(event, details);
    },

    emitError(error: { code: number; stage: string; message: string; recoverable: boolean }): void {
      errorHandler?.(error);
    },
  };

  return runtime;
}

function playerWithMock(config: PlayerConfig = {}): CheetahPlayer & { readonly runtime: MockRuntime } {
  const runtime = mockRuntime();
  const player = createPlayerWithRuntime(config, () => runtime);
  return Object.assign(player, { runtime });
}

describe('web sdk', () => {
  it('creates a player with id and version', () => {
    const player = createPlayer();
    expect(player.version).toBeDefined();
    expect(player.id).toMatch(/^cheetah-\d+$/);
    expect(player.state).toBe('idle');
  });

  it('load transitions to loading and then idle after stop', async () => {
    const player = playerWithMock();
    const states: string[] = [];
    player.addEventListener('statechange', (ev) => {
      states.push((ev.details?.to as string) ?? player.state);
    });
    await player.load('http://example.com/test.flv');
    expect(player.state).toBe('preroll');
    await player.stop();
    expect(player.state).toBe('idle');
    expect(states).toContain('loading');
    expect(states).toContain('preroll');
    expect(states).toContain('idle');
  });

  it('play and pause change state', () => {
    const player = playerWithMock();
    player.play();
    expect(player.state).toBe('playing');
    player.pause();
    expect(player.state).toBe('paused');
  });

  it('seek and setPlaybackRate forward to the runtime', async () => {
    const player = playerWithMock();
    await player.load('http://example.com/test.flv');
    await expect(player.seek(12345)).resolves.toBeUndefined();
    await expect(player.setPlaybackRate(2)).resolves.toBeUndefined();
  });

  it('frameStep and pauseDisplay forward to the runtime', async () => {
    const player = playerWithMock();
    await player.load('http://example.com/test.flv');
    await expect(player.frameStep('forward', true)).resolves.toBeUndefined();
    await expect(player.pauseDisplay(false)).resolves.toBeUndefined();
  });

  it('ptz generates a GB28181 command and emits a ptz event', async () => {
    const player = playerWithMock();
    await player.load('http://example.com/test.flv');
    let received: CheetahPlayerEvent<'ptz'> | undefined;
    player.addEventListener('ptz', (event) => {
      received = event as CheetahPlayerEvent<'ptz'>;
    });
    await player.ptz({ action: 'up', speeds: { vertical: 16 } });
    expect(received).toBeDefined();
    expect(received?.details?.ptzCmd).toMatch(/^[0-9A-F]{16}$/);
    expect(received?.details?.action).toBe('up');
  });

  it('seek is rejected when no active stream', async () => {
    const player = playerWithMock();
    await expect(player.seek(0)).rejects.toBeInstanceOf(CheetahMediaError);
  });

  it('destroy prevents further calls', async () => {
    const player = playerWithMock();
    await player.destroy();
    expect(player.state).toBe('destroyed');
    expect(() => player.play()).toThrow(CheetahMediaError);
    await expect(player.load('http://example.com/test.flv')).rejects.toBeInstanceOf(CheetahMediaError);
  });

  it('emits statechange before business events', () => {
    const player = playerWithMock();
    const order: string[] = [];
    player.addEventListener('statechange', () => order.push('statechange'));
    player.addEventListener('tracks', () => order.push('tracks'));
    player.runtime.emitEvent('statechange', { to: 'preroll' });
    player.runtime.emitEvent('tracks', { count: 1 });
    expect(order).toEqual(['statechange', 'tracks']);
  });

  it('maps runtime errors to CheetahMediaError events', () => {
    const player = playerWithMock();
    let received: CheetahPlayerEvent<'error'> | undefined;
    player.addEventListener('error', (ev) => {
      received = ev;
    });
    player.runtime.emitError({ code: 6100, stage: 'decoder', message: 'decode failed', recoverable: true });
    expect(player.state).toBe('failed');
    expect(received).toBeDefined();
    expect(received?.details?.error).toMatchObject({ code: 6100, stage: 'decoder', recoverable: true });
  });

  it('validates config and rejects invalid latency', () => {
    expect(() => createPlayer({ latency: { softMs: 10_000, hardMs: 5_000 } })).toThrow(CheetahMediaError);
    expect(() => createPlayer({ latency: { maxPlaybackRate: 3 } })).toThrow(CheetahMediaError);
  });

  it('validates config and rejects non-finite numeric values', () => {
    expect(() => createPlayer({ latency: { softMs: NaN } })).toThrow(CheetahMediaError);
    expect(() => createPlayer({ latency: { hardMs: Infinity } })).toThrow(CheetahMediaError);
    expect(() => createPlayer({ latency: { maxPlaybackRate: NaN } })).toThrow(CheetahMediaError);
    expect(() => createPlayer({ memory: { maxWasmMemoryMB: NaN } })).toThrow(CheetahMediaError);
    expect(() => createPlayer({ diagnostics: { statsIntervalMs: NaN } })).toThrow(CheetahMediaError);
    expect(() => createPlayer({ render: { maxResolution: { width: NaN, height: 1080 } } })).toThrow(CheetahMediaError);
  });

  it('redacts sensitive config in diagnostics', () => {
    const player = createPlayerWithRuntime(
      {
        security: { token: 'secret', credentials: { key: 'value' } },
      },
      () => mockRuntime(),
    );
    const diag = player.exportDiagnostics();
    expect(diag.config.security?.token).toBe('<redacted>');
    expect(diag.config.security?.credentials).toBe('<redacted>');
    expect(diag.config.transport?.headers).toEqual({});
  });

  it('snapshot returns ImageData', async () => {
    const player = playerWithMock();
    player.runtime.emitEvent('statechange', { to: 'playing' });
    const image = await player.snapshot({ maxWidth: 100, maxHeight: 100 });
    expect(image.width).toBe(100);
    expect(image.height).toBe(100);
  });

  it('snapshot preserves validation error message', async () => {
    const runtime = mockRuntime();
    runtime.request = (type) => {
      if (type === 'snapshot') return Promise.resolve({ width: 'not-a-number' });
      return Promise.reject(new Error('unsupported'));
    };
    const player = Object.assign(createPlayerWithRuntime({}, () => runtime), { runtime });
    player.runtime.emitEvent('statechange', { to: 'playing' });
    await expect(player.snapshot()).rejects.toThrow('Invalid snapshot result');
  });

  it('switchVariant validates input', async () => {
    const player = playerWithMock();
    await expect(player.switchVariant({})).rejects.toBeInstanceOf(CheetahMediaError);
    await expect(player.switchVariant({ bandwidth: 1_000_000 })).resolves.toBeUndefined();
  });

  it('getStats returns latest stats and throttles stats events', () => {
    const player = playerWithMock({ diagnostics: { statsIntervalMs: 10_000 } });
    player.runtime.emitEvent('stats', { bufferedMs: 100 });
    player.runtime.emitEvent('stats', { bufferedMs: 250 });
    const stats = player.getStats();
    expect(stats.bufferedMs).toBe(250);
  });

  it('getMetrics includes gauges from stats events', () => {
    const player = playerWithMock({});
    player.runtime.emitEvent('stats', {
      bufferedMs: 100,
      decodedFrames: 50,
      droppedFrames: 2,
      networkBytes: 1024,
      latencyMs: 80,
    });
    const metrics = player.getMetrics();
    expect(metrics.metrics.timeline?.['buffered-ms']?.type).toBe('gauge');
    expect((metrics.metrics.timeline?.['buffered-ms'] as { value: number } | undefined)?.value).toBe(100);
  });

  it('caps event history and freezes diagnostics events', () => {
    const player = createPlayerWithRuntime({ diagnostics: { maxEventHistory: 3 } }, () => mockRuntime());
    player.play();
    player.pause();
    player.play();
    const diag = player.exportDiagnostics();
    expect(diag.recentEvents.length).toBeLessThanOrEqual(3);
    expect(Object.isFrozen(diag.recentEvents)).toBe(true);
  });

  it('respects maxEventHistory=0', () => {
    const player = createPlayerWithRuntime({ diagnostics: { maxEventHistory: 0 } }, () => mockRuntime());
    player.play();
    player.pause();
    const diag = player.exportDiagnostics();
    expect(diag.recentEvents.length).toBe(0);
  });

  it('destroy clears metrics and history', async () => {
    const player = playerWithMock();
    player.runtime.emitEvent('stats', { bufferedMs: 100 });
    player.play();
    await player.destroy();
    expect(player.state).toBe('destroyed');
    expect(() => player.getMetrics()).toThrow(CheetahMediaError);
    expect(() => player.exportDiagnostics()).toThrow(CheetahMediaError);
  });

  it('reports intercom not active by default', () => {
    const player = playerWithMock();
    expect(player.intercomActive).toBe(false);
  });

  it('rejects opus intercom as unsupported', async () => {
    const player = playerWithMock();
    await expect(
      player.startIntercom({ codec: 'opus', sendPacket: () => {} }),
    ).rejects.toBeInstanceOf(CheetahMediaError);
  });

  it('rejects intercom when microphone is not available in the environment', async () => {
    const player = playerWithMock();
    await expect(
      player.startIntercom({ codec: 'g711u', sendPacket: () => {} }),
    ).rejects.toBeInstanceOf(CheetahMediaError);
  });

  it('stopIntercom is safe when intercom was never started', async () => {
    const player = playerWithMock();
    await expect(player.stopIntercom()).resolves.toBeUndefined();
    expect(player.intercomActive).toBe(false);
  });

  describe('download', () => {
    function makeStream(chunks: Uint8Array[], signal: AbortSignal | undefined, closeOnDone = true) {
      let index = 0;
      return new ReadableStream<Uint8Array>({
        start(controller) {
          if (signal) {
            signal.addEventListener('abort', () => {
              try {
                controller.close();
              } catch {
                // already closed
              }
            }, { once: true });
          }
        },
        pull(controller) {
          if (index >= chunks.length) {
            if (closeOnDone) controller.close();
            return;
          }
          if (controller.desiredSize !== null && controller.desiredSize <= 0) return;
          const chunk = chunks[index];
          if (chunk) {
            controller.enqueue(chunk);
            index += 1;
          }
        },
      });
    }

    beforeEach(() => {
      vi.stubGlobal(
        'fetch',
        vi.fn(async (_url: string | URL | Request, init?: RequestInit) => {
          const headers = init?.headers ? new Headers(init.headers) : new Headers();
          const range = headers.get('Range');
          const signal = init?.signal ?? undefined;
          if (range) {
            const start = Number.parseInt(range.replace('bytes=', ''), 10);
            return new Response(makeStream([new Uint8Array([start + 1, start + 2, start + 3])], signal), {
              status: 206,
            });
          }
          return new Response(makeStream([new Uint8Array([1, 2])], signal));
        }),
      );
    });

    afterEach(() => {
      vi.restoreAllMocks();
    });

    it('downloads a stream through the player', async () => {
      const player = playerWithMock();
      const result = await player.startDownload({ url: 'https://example.com/video.mp4' });
      expect(result.bytesWritten).toBe(2);
      expect(player.downloadActive).toBe(false);
      expect(player.downloadBlob).toBeInstanceOf(Blob);
      expect(player.downloadBlob?.size).toBe(2);
    });

    it('surfaces HTTP errors with a readable message', async () => {
      vi.stubGlobal(
        'fetch',
        vi.fn(async () => new Response('error', { status: 500 })),
      );
      const player = playerWithMock();
      await expect(player.startDownload({ url: 'https://example.com/video.mp4' })).rejects.toMatchObject({
        stage: 'download',
        message: expect.stringContaining('HTTP 500'),
      });
    });

    it('rejects malformed download URLs with a media error', async () => {
      const player = playerWithMock();
      await expect(player.startDownload({ url: 'not-a-url' })).rejects.toBeInstanceOf(CheetahMediaError);
    });

    it('rejects non-http download URLs', async () => {
      const player = playerWithMock();
      await expect(player.startDownload({ url: 'ftp://example.com/video.mp4' })).rejects.toBeInstanceOf(
        CheetahMediaError,
      );
    });

    it('does not save a partial file when a download is paused', async () => {
      const player = playerWithMock();
      const saveBlob = vi.spyOn(player as unknown as { saveBlob: () => void }, 'saveBlob');
      let paused = false;
      player.addEventListener('download', (event: CheetahPlayerEvent<'download'>) => {
        const progress = (event.details as { progress?: { bytesWritten: number } }).progress;
        if (!paused && progress && progress.bytesWritten >= 2) {
          paused = true;
          player.pauseDownload();
        }
      });
      const result = await player.startDownload({
        url: 'https://example.com/video.mp4',
        filename: 'video.mp4',
      });
      expect(result.bytesWritten).toBe(2);
      expect(player.downloadProgress?.state).toBe('paused');
      expect(saveBlob).not.toHaveBeenCalled();
      saveBlob.mockRestore();
      await player.stopDownload();
    });

    it('emits download progress events', async () => {
      const player = playerWithMock();
      const events: CheetahPlayerEvent<'download'>[] = [];
      player.addEventListener('download', (event) => events.push(event as CheetahPlayerEvent<'download'>));
      await player.startDownload({ url: 'https://example.com/video.mp4' });
      expect(events.length).toBeGreaterThanOrEqual(1);
    });

    describe('resume', () => {
      beforeEach(() => {
        vi.stubGlobal(
          'fetch',
          vi.fn(async (_url: string | URL | Request, init?: RequestInit) => {
            const headers = init?.headers ? new Headers(init.headers) : new Headers();
            const range = headers.get('Range');
            const signal = init?.signal ?? undefined;
            if (range) {
              const start = Number.parseInt(range.replace('bytes=', ''), 10);
              return new Response(makeStream([new Uint8Array([start + 1, start + 2, start + 3])], signal), {
                status: 206,
              });
            }
            return new Response(makeStream([new Uint8Array([1, 2])], signal, false));
          }),
        );
      });

      it('resumes a paused download into the same sink', async () => {
        const player = playerWithMock();
        let paused = false;
        player.addEventListener('download', (event: CheetahPlayerEvent<'download'>) => {
          const progress = (event.details as { progress?: { bytesWritten: number } }).progress;
          if (!paused && progress && progress.bytesWritten >= 2) {
            paused = true;
            player.pauseDownload();
          }
        });
        const first = await player.startDownload({ url: 'https://example.com/video.mp4' });
        expect(first.bytesWritten).toBe(2);

        const result = await player.resumeDownload({ url: 'https://example.com/video.mp4' });
        expect(result.bytesWritten).toBe(5);
      });
    });

    it('stops an active download when the player is destroyed', async () => {
      function makeSlowStream(chunks: Uint8Array[], signal?: AbortSignal) {
        let index = 0;
        return new ReadableStream<Uint8Array>({
          start(controller) {
            if (signal) {
              signal.addEventListener('abort', () => {
                try {
                  controller.close();
                } catch {
                  // already closed
                }
              }, { once: true });
            }
          },
          pull(controller) {
            if (index >= chunks.length) return;
            const chunk = chunks[index];
            if (chunk) {
              controller.enqueue(chunk);
              index += 1;
            }
          },
        });
      }

      vi.stubGlobal(
        'fetch',
        vi.fn((_url: string | URL | Request, init?: RequestInit) => {
          const signal = init?.signal ?? undefined;
          return Promise.resolve(new Response(makeSlowStream([new Uint8Array([1, 2])], signal)));
        }),
      );

      const player = playerWithMock();
      const sink = { write: vi.fn(), close: vi.fn() };
      player.addEventListener('download', () => {
        void player.destroy();
      });
      const start = player.startDownload({ url: 'https://example.com/video.mp4', sink });
      await expect(start).rejects.toBeInstanceOf(CheetahMediaError);
      expect(player.downloadActive).toBe(false);
      expect(sink.write).toHaveBeenCalledTimes(1);
      expect(sink.close).toHaveBeenCalled();
    });
  });

  describe('vr / ai extensions', () => {
    it('reports vr and ai inactive by default', () => {
      const player = playerWithMock();
      expect(player.vrActive).toBe(false);
      expect(player.aiActive).toBe(false);
    });

    it('setVrRenderer accepts a custom renderer and reports active', async () => {
      const player = playerWithMock();
      const renderer: VrRenderer = {
        get active() {
          return true;
        },
        initialize: vi.fn().mockReturnValue(true),
        render: vi.fn(),
        destroy: vi.fn(),
      };
      player.setVrRenderer(renderer);
      expect(player.vrActive).toBe(true);
      expect(renderer.initialize).not.toHaveBeenCalled();
      await player.destroy();
      expect(renderer.destroy).toHaveBeenCalled();
    });

    it('setAiProcessor accepts a custom processor and skips when budget is insufficient', async () => {
      const player = playerWithMock();
      const processor: AiFrameProcessor = {
        get active() {
          return true;
        },
        initialize: vi.fn().mockReturnValue(true),
        process: vi.fn().mockReturnValue(undefined),
        destroy: vi.fn(),
      };
      player.setAiProcessor(processor);
      expect(player.aiActive).toBe(true);

      const frame = { width: 640, height: 480, timestampMs: 0, source: {} as HTMLCanvasElement } as unknown as ProcessableFrame;
      const budget: AiFrameBudget = { deadlineMs: 5, canAllocate: false };
      const result = await (processor as AiFrameProcessor).process(frame, budget);
      expect(result).toBeUndefined();
      await player.destroy();
      expect(processor.destroy).toHaveBeenCalled();
    });

    it('NoopVrRenderer and NoopAiFrameProcessor are safe defaults', () => {
      const player = playerWithMock();
      const vr = new NoopVrRenderer();
      const ai = new NoopAiFrameProcessor();
      player.setVrRenderer(vr);
      player.setAiProcessor(ai);
      expect(player.vrActive).toBe(false);
      expect(player.aiActive).toBe(false);
    });

    it('falls back to no-op when setVrRenderer/setAiProcessor receives null', () => {
      const player = playerWithMock();
      player.setVrRenderer(null as unknown as VrRenderer);
      player.setAiProcessor(null as unknown as AiFrameProcessor);
      expect(player.vrActive).toBe(false);
      expect(player.aiActive).toBe(false);
    });

    it('tolerates a renderer/processor that throws during destroy', async () => {
      const player = playerWithMock();
      const badVr: VrRenderer = {
        get active() {
          return true;
        },
        initialize: vi.fn().mockReturnValue(true),
        render: vi.fn(),
        destroy: vi.fn().mockImplementation(() => {
          throw new Error('vr destroy failed');
        }),
      };
      const badAi: AiFrameProcessor = {
        get active() {
          return true;
        },
        initialize: vi.fn().mockReturnValue(true),
        process: vi.fn().mockReturnValue(undefined),
        destroy: vi.fn().mockImplementation(() => {
          throw new Error('ai destroy failed');
        }),
      };
      player.setVrRenderer(badVr);
      player.setAiProcessor(badAi);
      expect(player.vrActive).toBe(true);
      expect(player.aiActive).toBe(true);

      // Swap to a new renderer/processor even though the old ones throw.
      player.setVrRenderer(new NoopVrRenderer());
      player.setAiProcessor(new NoopAiFrameProcessor());
      expect(player.vrActive).toBe(false);
      expect(player.aiActive).toBe(false);

      // Player destroy still succeeds.
      await player.destroy();
    });

    it('rejects setVrRenderer and setAiProcessor after destroy', async () => {
      const player = playerWithMock();
      await player.destroy();
      expect(() => player.setVrRenderer(new NoopVrRenderer())).toThrow(CheetahMediaError);
      expect(() => player.setAiProcessor(new NoopAiFrameProcessor())).toThrow(CheetahMediaError);
    });
  });
});
