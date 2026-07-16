import { describe, it, expect } from 'vitest';
import { detectCapabilities, probeCapabilities, CapabilityCache, type ProbedCapabilityReport } from './capabilities';

describe('detectCapabilities()', () => {
  it('returns a report with a fingerprint and timestamp', () => {
    const report = detectCapabilities();
    expect(typeof report.secureContext).toBe('boolean');
    expect(typeof report.fingerprint).toBe('string');
    expect(report.timestamp).toBeGreaterThan(0);
    expect(report.confidence).toBe('low');
    expect(Array.isArray(report.reasons)).toBe(true);
  });

  it('reports wasm support when WebAssembly is available', () => {
    const report = detectCapabilities();
    expect(report.wasm).toBe(typeof globalThis.WebAssembly !== 'undefined');
  });

  it('reports webTransport support when the API is present', () => {
    const original = (globalThis as unknown as { WebTransport?: unknown }).WebTransport;
    (globalThis as unknown as { WebTransport?: unknown }).WebTransport = class {};
    const report = detectCapabilities();
    expect(report.webTransport).toBe(true);
    expect(report.reasons).toContain('webtransport-api');
    (globalThis as unknown as { WebTransport?: unknown }).WebTransport = original;
  });
});

describe('probeCapabilities()', () => {
  it('returns a high/medium confidence report with probe details', async () => {
    const report = await probeCapabilities();
    expect(report.details).toBeDefined();
    expect(report.details.wasm).toBeDefined();
    expect(report.details.renderer).toBeDefined();
    expect(report.confidence === 'high' || report.confidence === 'medium').toBe(true);
  });

  it('does not throw in a node/vitest environment', async () => {
    await expect(probeCapabilities()).resolves.toBeDefined();
  });

  it('probes webTransport details without network', async () => {
    const original = (globalThis as unknown as { WebTransport?: unknown }).WebTransport;
    function MockTransport() { /* no-op */ }
    Object.defineProperty(MockTransport.prototype, 'datagrams', {
      get() {
        return { readable: new ReadableStream<Uint8Array>() };
      },
    });
    Object.defineProperty(MockTransport.prototype, 'incomingUnidirectionalStreams', {
      get() {
        return new ReadableStream<ReadableStream<Uint8Array>>();
      },
    });
    (globalThis as unknown as { WebTransport?: unknown }).WebTransport = MockTransport as unknown as typeof globalThis.WebTransport;
    const report = await probeCapabilities();
    expect(report.details.webTransport.incomingUnidirectionalStreams).toBe(true);
    expect(report.details.webTransport.datagrams).toBe(true);
    expect(report.reasons).toContain('webtransport-supported');
    (globalThis as unknown as { WebTransport?: unknown }).WebTransport = original;
  });
});

describe('CapabilityCache', () => {
  it('probes and caches by fingerprint', async () => {
    const cache = new CapabilityCache(60000);
    cache.setEnvironment('v1', 'c1');

    const first = await cache.probe();
    const second = await cache.probe();
    expect(first.fingerprint).toBe(second.fingerprint);
    expect(first).toBe(second);
  });

  it('invalidates the cache when the environment version changes', async () => {
    const cache = new CapabilityCache(60000);
    cache.setEnvironment('v1', 'c1');

    const report = {
      ...detectCapabilities(),
      fingerprint: 'fp',
      timestamp: performance.now(),
      details: {
        webCodecs: {},
        mse: {},
        wasm: { simd: false, threads: false, sharedMemory: false, memoryLimitPages: 0 },
        renderer: { webgpu: false, webgl2: false, canvas2d: false, videoFrame: false, preferredPixelFormat: undefined },
        webTransport: { datagrams: false, incomingUnidirectionalStreams: false, incomingBidirectionalStreams: false, byob: false },
      },
    } satisfies ProbedCapabilityReport;

    cache.put(report);
    expect(cache.get('fp')).toBe(report);

    cache.setEnvironment('v2', 'c1');
    expect(cache.get('fp')).toBeUndefined();
  });

  it('expires cached entries after the TTL', async () => {
    const cache = new CapabilityCache(1);
    cache.setEnvironment('v1', 'c1');

    const report = {
      ...detectCapabilities(),
      fingerprint: 'fp',
      timestamp: performance.now(),
      details: {
        webCodecs: {},
        mse: {},
        wasm: { simd: false, threads: false, sharedMemory: false, memoryLimitPages: 0 },
        renderer: { webgpu: false, webgl2: false, canvas2d: false, videoFrame: false, preferredPixelFormat: undefined },
        webTransport: { datagrams: false, incomingUnidirectionalStreams: false, incomingBidirectionalStreams: false, byob: false },
      },
    } satisfies ProbedCapabilityReport;

    cache.put(report);
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(cache.get('fp')).toBeUndefined();
  });
});
