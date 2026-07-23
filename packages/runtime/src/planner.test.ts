import { describe, it, expect } from 'vitest';
import { plan, explain, type PlanRequest, type PlaybackPlan } from './planner';
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
    webTransport: true,
    webRtc: true,
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
      webTransport: {
        datagrams: false,
        incomingUnidirectionalStreams: true,
        incomingBidirectionalStreams: false,
        byob: false,
      },
      webRtc: {
        peerConnection: true,
        dataChannel: true,
        insertableStreams: false,
        getUserMedia: false,
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

  it('routes raw Annex-B H.264 to WebCodecs when supported', () => {
    const request: PlanRequest = {
      protocol: 'http-annexb',
      tracks: [{ kind: 'video', codec: 'h264', width: 640, height: 360 }],
      latencyTarget: 'normal',
    };
    const caps = makeCaps();
    const result = plan(request, caps);

    expect(result.primary.transport).toBe('fetch');
    expect(result.primary.videoBackend).toBe('webcodecs');
    expect(result.primary.renderer).toBe('webgpu');
  });

  it('uses websocket transport for ws-annexb', () => {
    const request: PlanRequest = {
      protocol: 'ws-annexb',
      tracks: [{ kind: 'video', codec: 'h265', width: 1920, height: 1080 }],
      latencyTarget: 'realtime',
      isolation: true,
    };
    const caps = makeCaps();
    const result = plan(request, caps);

    expect(result.primary.transport).toBe('websocket');
    expect(result.primary.videoBackend).toBe('webcodecs');
  });

  it('prefers WASM for MPEG-PS because WebCodecs and MSE cannot demux it', () => {
    const request: PlanRequest = {
      protocol: 'http-mpegps',
      tracks: [
        { kind: 'video', codec: 'h264', width: 640, height: 360 },
        { kind: 'audio', codec: 'aac', sampleRate: 48000, channels: 2 },
      ],
      latencyTarget: 'normal',
      isolation: true,
    };
    const caps = makeCaps();
    const result = plan(request, caps);

    expect(result.primary.transport).toBe('fetch');
    expect(result.primary.videoBackend).toBe('wasm-threads-simd');
    expect(result.primary.audioBackend).toBe('wasm-threads-simd');
    expect(result.unsupported.some((u) => u.backend === 'mse')).toBe(true);
    expect(result.unsupported.some((u) => u.backend === 'webcodecs')).toBe(true);
  });

  it('reports no route for MPEG-PS when WASM is unavailable', () => {
    const request: PlanRequest = {
      protocol: 'ws-mpegps',
      tracks: [{ kind: 'video', codec: 'h264', width: 640, height: 360 }],
      latencyTarget: 'normal',
    };
    const caps = makeCaps({ wasm: false, simd: false, threads: false });
    const result = plan(request, caps);

    expect(result.candidates).toHaveLength(0);
    expect(result.primary.reason).toContain('no supported');
  });

  it('selects webtransport mode for the webtransport protocol', () => {
    const request: PlanRequest = {
      protocol: 'webtransport',
      tracks: [{ kind: 'video', codec: 'h264', width: 640, height: 360 }],
      latencyTarget: 'realtime',
      isolation: true,
    };
    const result = plan(request, makeCaps());

    expect(result.primary.transport).toBe('webtransport');
    expect(result.primary.videoBackend).toBe('webcodecs');
  });

  it('selects webrtc mode for the webrtc protocol', () => {
    const request: PlanRequest = {
      protocol: 'webrtc',
      tracks: [
        { kind: 'video', codec: 'h264', width: 640, height: 360 },
        { kind: 'audio', codec: 'aac', sampleRate: 48000, channels: 2 },
      ],
      latencyTarget: 'realtime',
      isolation: true,
    };
    const result = plan(request, makeCaps());

    expect(result.primary.transport).toBe('webrtc');
    expect(result.primary.videoBackend).toBe('webcodecs');
    expect(result.primary.audioBackend).toBe('webcodecs');
    expect(result.candidates.every((c) => c.videoBackend !== 'mse' && c.audioBackend !== 'mse')).toBe(true);
  });

  it('rejects WASM routes for a non-positive maxWasmMemoryMB budget', () => {
    const request: PlanRequest = {
      protocol: 'http-flv',
      tracks: [{ kind: 'video', codec: 'h264', width: 640, height: 360 }],
      latencyTarget: 'normal',
      isolation: true,
      disabled: ['webcodecs', 'mse'],
      budget: { maxWasmMemoryMB: -1 },
    };
    const caps = makeCaps();
    const result = plan(request, caps);

    expect(result.candidates.every((c) => c.videoBackend !== 'wasm-threads-simd' && c.videoBackend !== 'wasm-simd' && c.videoBackend !== 'wasm-baseline')).toBe(true);
    expect(result.unsupported.some((u) => u.backend === 'wasm-threads-simd' && u.reason.includes('WASM memory budget'))).toBe(true);
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

  it('rejects invalid plan arguments', () => {
    expect(() => explain(null as unknown as PlaybackPlan)).toThrow('explain plan must be a valid PlaybackPlan');
    expect(() => explain({} as unknown as PlaybackPlan)).toThrow('explain plan must be a valid PlaybackPlan');
    expect(() =>
      explain({
        primary: { reason: 123 },
        candidates: [],
        unsupported: [],
      } as unknown as PlaybackPlan),
    ).toThrow('explain plan must be a valid PlaybackPlan');
  });
});
