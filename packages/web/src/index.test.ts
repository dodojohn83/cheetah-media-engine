import { describe, it, expect } from 'vitest';
import {
  createPlayer,
  createPlayerWithRuntime,
  CheetahMediaError,
  type CheetahPlayer,
  type CheetahPlayerEvent,
  type PlayerConfig,
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
    expect(player.state).toBe('loading');
    await player.stop();
    expect(player.state).toBe('idle');
    expect(states).toContain('loading');
    expect(states).toContain('idle');
  });

  it('play and pause change state', () => {
    const player = playerWithMock();
    player.play();
    expect(player.state).toBe('playing');
    player.pause();
    expect(player.state).toBe('paused');
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
});
