import { describe, it, expect, vi } from 'vitest';
import { FallbackController, type MediaBackend, type BackendContext, type FallbackEvent } from './fallback';
import type { PlaybackPlan, PlanCandidate } from './planner';

function makePlan(candidates: PlanCandidate[]): PlaybackPlan {
  return {
    candidates,
    primary: candidates[0] ?? {
      rank: 0,
      videoBackend: undefined,
      audioBackend: undefined,
      renderer: undefined,
      transport: 'fetch',
      reason: 'empty',
      isLive: true,
    },
    fallback: candidates.slice(1),
    unsupported: [],
    degraded: false,
    reasonChain: ['test'],
  };
}

function candidate(video: string, audio: string): PlanCandidate {
  return {
    rank: 1,
    videoBackend: video as PlanCandidate['videoBackend'],
    audioBackend: audio as PlanCandidate['audioBackend'],
    renderer: undefined,
    transport: 'fetch',
    reason: `video=${video}, audio=${audio}`,
    isLive: true,
  };
}

function fakeBackend(ctx: BackendContext, fail = false, supportsSeek = false): MediaBackend {
  const identity = ctx.candidate.videoBackend ?? 'wasm-baseline';
  const backend: MediaBackend = {
    identity,
    configure: fail
      ? () => Promise.reject(new Error('configure failed'))
      : () => Promise.resolve(),
    stop: () => Promise.resolve(),
  };
  if (supportsSeek) {
    backend.seek = (timeMs: number) => {
      expect(typeof timeMs).toBe('number');
      return Promise.resolve();
    };
    backend.setPlaybackRate = (rate: number) => {
      expect(typeof rate).toBe('number');
      return Promise.resolve();
    };
    backend.frameStep = (direction: 'forward' | 'backward', keyframeOnly?: boolean) => {
      expect(direction === 'forward' || direction === 'backward').toBe(true);
      expect(typeof keyframeOnly).toBe('boolean');
      return Promise.resolve();
    };
    backend.pauseDisplay = (keepConnection?: boolean) => {
      expect(typeof keepConnection).toBe('boolean');
      return Promise.resolve();
    };
  }
  return backend;
}

describe('FallbackController', () => {
  it('activates the first candidate when configureNext is called', async () => {
    const plan = makePlan([
      candidate('webcodecs', 'webcodecs'),
      candidate('mse', 'mse'),
    ]);
    const factory = vi.fn(fakeBackend);
    const controller = new FallbackController({ plan, factory });

    const backend = await controller.configureNext('initial');

    expect(backend?.identity).toBe('webcodecs');
    expect(factory).toHaveBeenCalledTimes(1);
  });

  it('emits backendchange events on transitions', async () => {
    const plan = makePlan([
      candidate('webcodecs', 'webcodecs'),
      candidate('mse', 'mse'),
    ]);
    const events: FallbackEvent[] = [];
    const controller = new FallbackController({
      plan,
      factory: (ctx) => fakeBackend(ctx),
      onEvent: (e) => events.push(e),
    });

    await controller.configureNext('start');
    await controller.reportFailure('decode error');

    const changes = events.filter((e) => e.type === 'backendchange') as { type: 'backendchange'; payload: { to: string } }[];
    expect(changes.length).toBeGreaterThanOrEqual(1);
    expect(changes[changes.length - 1]?.payload.to).toBe('mse');
  });

  it('falls back to the next candidate when the first configure fails', async () => {
    const plan = makePlan([
      candidate('webcodecs', 'webcodecs'),
      candidate('mse', 'mse'),
      candidate('wasm-simd', 'wasm-simd'),
    ]);
    const factory = vi.fn((ctx: BackendContext) => fakeBackend(ctx, ctx.candidate.videoBackend === 'webcodecs'));
    const controller = new FallbackController({ plan, factory });

    const first = await controller.configureNext('initial');
    // webcodecs fails during configure, so the first *successful* backend is mse.
    expect(first?.identity).toBe('mse');

    const second = await controller.reportFailure('mse decode failed');
    expect(second?.identity).toBe('wasm-simd');
  });

  it('does not loop back to an already-tried backend in the same epoch', async () => {
    const plan = makePlan([
      candidate('webcodecs', 'webcodecs'),
      candidate('mse', 'mse'),
    ]);
    const factory = vi.fn((ctx: BackendContext) => fakeBackend(ctx, true));
    const controller = new FallbackController({ plan, factory });

    const first = await controller.configureNext('initial');
    expect(first).toBeUndefined();
    expect(factory).toHaveBeenCalledTimes(2);
  });

  it('reports unsupported after all candidates fail', async () => {
    const plan = makePlan([
      candidate('webcodecs', 'webcodecs'),
      candidate('mse', 'mse'),
    ]);
    const events: FallbackEvent[] = [];
    const controller = new FallbackController({
      plan,
      factory: (ctx) => fakeBackend(ctx, true),
      onEvent: (e) => events.push(e),
    });

    await controller.configureNext('initial');

    const unsupported = events.find((e) => e.type === 'unsupported') as { type: 'unsupported'; payload: { attemptChain: { backend: string; reason: string }[] } } | undefined;
    expect(unsupported).toBeDefined();
    const chain = unsupported?.payload.attemptChain ?? [];
    expect(chain.map((a) => a.backend)).toContain('webcodecs');
    expect(chain.map((a) => a.backend)).toContain('mse');
    expect(chain.every((a) => typeof a.reason === 'string' && a.reason.length > 0)).toBe(true);
  });

  it('stops accepting new work after stop()', async () => {
    const plan = makePlan([candidate('webcodecs', 'webcodecs')]);
    const controller = new FallbackController({
      plan,
      factory: (ctx) => fakeBackend(ctx),
    });

    await controller.stop();
    const backend = await controller.configureNext('initial');
    expect(backend).toBeUndefined();
  });

  it('newEpoch clears tried set so backends can be retried', async () => {
    const plan = makePlan([
      candidate('webcodecs', 'webcodecs'),
      candidate('mse', 'mse'),
    ]);
    const factory = vi.fn((ctx: BackendContext) => fakeBackend(ctx, ctx.candidate.videoBackend === 'webcodecs'));
    const controller = new FallbackController({ plan, factory });

    const first = await controller.configureNext('initial');
    expect(first?.identity).toBe('mse');

    await controller.reportFailure('mse decode failed');
    controller.newEpoch();

    const retry = await controller.configureNext('retry');
    // webcodecs still fails, but because the epoch was reset it is attempted again before mse.
    expect(factory).toHaveBeenCalledWith(expect.objectContaining({ candidate: expect.objectContaining({ videoBackend: 'webcodecs' }) }));
    expect(retry?.identity).toBe('mse');
  });

  it('does not recurse infinitely on a candidate with no backends', async () => {
    const plan = makePlan([
      {
        rank: 1,
        videoBackend: undefined,
        audioBackend: undefined,
        renderer: undefined,
        transport: 'fetch',
        reason: 'empty',
        isLive: true,
      },
    ]);
    const factory = vi.fn(() => fakeBackend({ candidate: plan.candidates[0]!, reason: 'test' }, true));
    const events: FallbackEvent[] = [];
    const controller = new FallbackController({
      plan,
      factory,
      onEvent: (e) => events.push(e),
    });

    const backend = await controller.configureNext('initial');
    expect(backend).toBeUndefined();
    expect(factory).toHaveBeenCalledTimes(1);
    expect(events.some((e) => e.type === 'unsupported')).toBe(true);
  });

  it('setPlan replaces the candidate list and resets tried state', async () => {
    const plan = makePlan([candidate('webcodecs', 'webcodecs')]);
    const controller = new FallbackController({
      plan,
      factory: (ctx) => fakeBackend(ctx, ctx.candidate.videoBackend === 'webcodecs'),
    });

    await controller.configureNext('initial');
    controller.setPlan(makePlan([candidate('mse', 'mse')]));

    const backend = await controller.configureNext('new plan');
    expect(backend?.identity).toBe('mse');
  });

  it('stops the previous backend before switching', async () => {
    const plan = makePlan([
      candidate('webcodecs', 'webcodecs'),
      candidate('mse', 'mse'),
    ]);
    const stop = vi.fn(() => Promise.resolve());
    const factory = vi.fn((ctx: BackendContext): MediaBackend => ({
      identity: ctx.candidate.videoBackend ?? 'wasm-baseline',
      configure: () => Promise.resolve(),
      stop,
    }));
    const controller = new FallbackController({ plan, factory });

    await controller.configureNext('initial');
    await controller.reportFailure('decode failure');

    expect(stop).toHaveBeenCalled();
  });

  it('forwards seek and setPlaybackRate to the current backend', async () => {
    const plan = makePlan([candidate('mse', 'mse')]);
    const backend = fakeBackend({ candidate: plan.candidates[0]!, reason: 'test' }, false, true);
    const seekSpy = vi.spyOn(backend, 'seek');
    const rateSpy = vi.spyOn(backend, 'setPlaybackRate');
    const controller = new FallbackController({
      plan,
      factory: () => backend,
    });

    await controller.configureNext('initial');
    await controller.seek(5000);
    await controller.setPlaybackRate(2);

    expect(seekSpy).toHaveBeenCalledWith(5000);
    expect(rateSpy).toHaveBeenCalledWith(2);
  });

  it('seek and setPlaybackRate throw when backend does not support them', async () => {
    const plan = makePlan([candidate('webcodecs', 'webcodecs')]);
    const controller = new FallbackController({
      plan,
      factory: (ctx) => fakeBackend(ctx, ctx.candidate.videoBackend === 'webcodecs'),
    });

    await controller.configureNext('initial');
    await expect(controller.seek(1000)).rejects.toThrow('does not support seek');
    await expect(controller.setPlaybackRate(2)).rejects.toThrow('does not support playback rate');
  });

  it('forwards frameStep and pauseDisplay to the current backend', async () => {
    const plan = makePlan([candidate('mse', 'mse')]);
    const backend = fakeBackend({ candidate: plan.candidates[0]!, reason: 'test' }, false, true);
    const stepSpy = vi.spyOn(backend, 'frameStep');
    const pauseSpy = vi.spyOn(backend, 'pauseDisplay');
    const controller = new FallbackController({
      plan,
      factory: () => backend,
    });

    await controller.configureNext('initial');
    await controller.frameStep('forward', true);
    await controller.pauseDisplay(false);

    expect(stepSpy).toHaveBeenCalledWith('forward', true);
    expect(pauseSpy).toHaveBeenCalledWith(false);
  });

  it('frameStep and pauseDisplay throw when backend does not support them', async () => {
    const plan = makePlan([candidate('webcodecs', 'webcodecs')]);
    const controller = new FallbackController({
      plan,
      factory: (ctx) => fakeBackend(ctx, ctx.candidate.videoBackend === 'webcodecs'),
    });

    await controller.configureNext('initial');
    await expect(controller.frameStep('forward')).rejects.toThrow('does not support frame step');
    await expect(controller.pauseDisplay()).rejects.toThrow('does not support pause display');
  });
});
