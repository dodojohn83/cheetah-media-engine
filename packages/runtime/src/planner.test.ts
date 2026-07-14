import { describe, it, expect } from 'vitest';
import { plan, explain, type PlanRequest } from './planner';
import type { ProbedCapabilityReport } from './capabilities';

function makeCaps(overrides: Partial<ProbedCapabilityReport> = {}): ProbedCapabilityReport {
  const base: ProbedCapabilityReport = {
    secureContext: true,
    crossOriginIsolated: true,
    sharedArrayBuffer: true,
    atomics: true,
    simd: true,
    threads: true,
    webCodecs: true,
    mse: true,
    webAudio: true,
    offscreenCanvas: true,
    webgpu: true,
    webgl2: true,
    canvas2d: true,
    videoFrame: true,
    wasm: true,
    fingerprint: 'test',
    timestamp: 0,
    confidence: 'high',
    reasons: [],
    details: {
      webCodecs: {
        'avc1.42001e': true,
        'hvc1.1.6.l93.b0': true,
        'mp4a.40.2': true,
        alaw: true,
        ulaw: true,
        mp3: true,
      },
      mse: {
        'avc1.42001e': true,
        'hvc1.1.6.l93.b0': false,
        'mp4a.40.2': true,
        alaw: false,
        ulaw: false,
        mp3: true,
      },
      wasm: {
        simd: true,
        threads: true,
        sharedMemory: true,
        memoryLimitPages: 32767,
      },
      renderer: {
        webgpu: true,
        webgl2: true,
        canvas2d: true,
        videoFrame: true,
        preferredPixelFormat: 'I420',
      },
    },
  };
  // Merge overrides shallowly for the top-level and details fields.
  return {
    ...base,
    ...overrides,
    details: {
      ...base.details,
      ...(overrides.details ?? {}),
    },
  };
}

describe('plan()', () => {
  it('selects WebCodecs as primary for HTTP-FLV H.264/AAC with full capabilities', () => {
    const request: PlanRequest = {
      protocol: 'http-flv',
      tracks: [
        { kind: 'video', codec: 'h264', width: 640, height: 360 },
        { kind: 'audio', codec: 'aac', sampleRate: 48000, channels: 2 },
      ],
      latencyTarget: 'normal',
      isolation: true,
    };
    const caps = makeCaps();
    const result = plan(request, caps);

    expect(result.primary.videoBackend).toBe('webcodecs');
    expect(result.primary.audioBackend).toBe('webcodecs');
    expect(result.primary.renderer).toBe('webgpu');
    expect(result.primary.transport).toBe('fetch');
    expect(result.degraded).toBe(false);
    expect(result.fallback.length).toBeGreaterThan(0);
  });

  it('falls back to WASM when WebCodecs and MSE are disabled', () => {
    const request: PlanRequest = {
      protocol: 'http-flv',
      tracks: [
        { kind: 'video', codec: 'h264', width: 640, height: 360 },
        { kind: 'audio', codec: 'aac', sampleRate: 48000, channels: 2 },
      ],
      latencyTarget: 'normal',
      isolation: true,
      disabled: ['webcodecs', 'mse'],
    };
    const caps = makeCaps();
    const result = plan(request, caps);

    expect(result.primary.videoBackend).toBe('wasm-threads-simd');
    expect(result.primary.audioBackend).toBe('wasm-threads-simd');
    expect(result.primary.renderer).toBe('webgpu');
  });

  it('uses MSE for fMP4 H.264/AAC when WebCodecs is unavailable', () => {
    const request: PlanRequest = {
      protocol: 'http-fmp4',
      tracks: [
        { kind: 'video', codec: 'h264', width: 640, height: 360 },
        { kind: 'audio', codec: 'aac', sampleRate: 48000, channels: 2 },
      ],
      latencyTarget: 'normal',
    };
    const caps = makeCaps({ webCodecs: false });
    const result = plan(request, caps);

    expect(result.primary.videoBackend).toBe('mse');
    expect(result.primary.audioBackend).toBe('mse');
    expect(result.primary.renderer).toBeUndefined();
    expect(result.primary.transport).toBe('fetch');
  });

  it('excludes MSE for FLV even when MSE is present', () => {
    const request: PlanRequest = {
      protocol: 'ws-flv',
      tracks: [
        { kind: 'video', codec: 'h264', width: 640, height: 360 },
        { kind: 'audio', codec: 'aac', sampleRate: 48000, channels: 2 },
      ],
      latencyTarget: 'low',
      isolation: true,
    };
    const caps = makeCaps({
      webCodecs: false,
      wasm: false,
      simd: false,
      threads: false,
      sharedArrayBuffer: false,
    });
    const result = plan(request, caps);

    // No WASM baseline either, so the plan is empty.
    expect(result.candidates).toHaveLength(0);
    expect(result.primary.reason).toContain('no supported');
    expect(result.unsupported.some((u) => u.backend === 'mse')).toBe(true);
  });

  it('requires isolation for WASM threads path', () => {
    const request: PlanRequest = {
      protocol: 'http-flv',
      tracks: [
        { kind: 'video', codec: 'h264', width: 640, height: 360 },
        { kind: 'audio', codec: 'aac', sampleRate: 48000, channels: 2 },
      ],
      latencyTarget: 'normal',
      isolation: false,
      disabled: ['webcodecs', 'mse'],
    };
    const caps = makeCaps();
    const result = plan(request, caps);

    expect(result.primary.videoBackend).toBe('wasm-simd');
  });

  it('prefers websocket transport for ws-fmp4', () => {
    const request: PlanRequest = {
      protocol: 'ws-fmp4',
      tracks: [
        { kind: 'video', codec: 'h264', width: 640, height: 360 },
      ],
      latencyTarget: 'realtime',
    };
    const caps = makeCaps();
    const result = plan(request, caps);

    expect(result.primary.transport).toBe('websocket');
  });

  it('reports degraded when the primary is not the preferred WebCodecs path', () => {
    const request: PlanRequest = {
      protocol: 'http-fmp4',
      tracks: [{ kind: 'video', codec: 'h264', width: 640, height: 360 }],
      latencyTarget: 'normal',
    };
    const caps = makeCaps({ webCodecs: false });
    const result = plan(request, caps);

    expect(result.degraded).toBe(true);
    expect(result.primary.videoBackend).toBe('mse');
  });

  it('produces a deterministic golden order for hls/h264/aac in isolated context', () => {
    const request: PlanRequest = {
      protocol: 'hls',
      tracks: [
        { kind: 'video', codec: 'h264' },
        { kind: 'audio', codec: 'aac' },
      ],
      latencyTarget: 'normal',
      isolation: true,
    };
    const caps = makeCaps();
    const result = plan(request, caps);

    expect(result.primary.videoBackend).toBe('webcodecs');
    expect(result.primary.audioBackend).toBe('webcodecs');
    const uniqueVideoBackends = result.candidates
      .map((c) => c.videoBackend)
      .filter((v, i, a) => v !== undefined && a.indexOf(v) === i);
    expect(uniqueVideoBackends[0]).toBe('webcodecs');
    expect(uniqueVideoBackends[1]).toBe('mse');
    expect(uniqueVideoBackends).toContain('wasm-threads-simd');

    // Fallback audio path is independent: webcodecs video can pair with mse audio.
    const webcodecsVideo = result.candidates.find((c) => c.videoBackend === 'webcodecs');
    expect(webcodecsVideo).toBeDefined();
    expect(result.candidates.some((c) => c.videoBackend === 'webcodecs' && c.audioBackend === 'mse')).toBe(true);
  });
});

describe('explain()', () => {
  it('summarizes the plan', () => {
    const request: PlanRequest = {
      protocol: 'http-flv',
      tracks: [{ kind: 'video', codec: 'h264' }],
      latencyTarget: 'normal',
      isolation: true,
    };
    const caps = makeCaps();
    const result = plan(request, caps);
    const text = explain(result);
    expect(text).toContain('primary:');
    expect(text).toContain('webcodecs');
  });
});
