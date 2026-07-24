import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createRuntime, RUNTIME_VERSION, detectCapabilities, encodeEnvelope, decodeEnvelope } from './index';

describe('runtime', () => {
  it('reports version', () => {
    const runtime = createRuntime();
    expect(runtime.version).toBe(RUNTIME_VERSION);
  });
});

describe('capabilities', () => {
  it('returns a report without throwing', () => {
    const caps = detectCapabilities();
    expect(caps).toHaveProperty('secureContext');
    expect(caps).toHaveProperty('sharedArrayBuffer');
  });
});

describe('messages', () => {
  it('round-trips an envelope', () => {
    const envelope = {
      protocolVersion: 1,
      instance: 2,
      epoch: 3,
      sequence: 4,
      type: 'load' as const,
      payload: { url: 'http://example.com/test.flv', isLive: false },
    };
    expect(decodeEnvelope(encodeEnvelope(envelope))).toEqual(envelope);
  });

  it('rejects unsupported protocol versions', () => {
    const bad = { ...JSON.parse(encodeEnvelope({
      protocolVersion: 1,
      instance: 1,
      epoch: 1,
      sequence: 1,
      type: 'load' as const,
    })) };
    bad.protocolVersion = 2;
    expect(() => decodeEnvelope(JSON.stringify(bad))).toThrow();
  });

  it('round-trips frame-step and pause-display envelopes', () => {
    const frameStep = {
      protocolVersion: 1,
      instance: 2,
      epoch: 3,
      sequence: 4,
      type: 'frame-step' as const,
      payload: { direction: 'forward' as const, keyframeOnly: true },
    };
    const pauseDisplay = {
      protocolVersion: 1,
      instance: 2,
      epoch: 3,
      sequence: 5,
      type: 'pause-display' as const,
      payload: { keepConnection: false },
    };
    expect(decodeEnvelope(encodeEnvelope(frameStep))).toEqual(frameStep);
    expect(decodeEnvelope(encodeEnvelope(pauseDisplay))).toEqual(pauseDisplay);
  });

  it('rejects malformed envelope fields', () => {
    const base = {
      protocolVersion: 1,
      instance: 1,
      epoch: 1,
      sequence: 1,
      type: 'load',
    };
    expect(() => decodeEnvelope(JSON.stringify({ ...base, instance: NaN }))).toThrow();
    expect(() => decodeEnvelope(JSON.stringify({ ...base, epoch: -1 }))).toThrow();
    expect(() => decodeEnvelope(JSON.stringify({ ...base, sequence: Infinity }))).toThrow();
    expect(() => decodeEnvelope(JSON.stringify({ ...base, type: 'unknown' }))).toThrow();
  });

  it('rejects non-JSON envelope data', () => {
    expect(() => decodeEnvelope('not json')).toThrow('Malformed envelope');
    expect(() => decodeEnvelope('{invalid')).toThrow('Malformed envelope');
  });

  it('rejects malformed envelopes in encodeEnvelope', () => {
    const base = {
      protocolVersion: 1,
      instance: 1,
      epoch: 1,
      sequence: 1,
      type: 'load',
    } as const;
    expect(() => encodeEnvelope(undefined as unknown as typeof base)).toThrow('Envelope must be an object');
    expect(() => encodeEnvelope({ ...base, instance: NaN } as unknown as typeof base)).toThrow('Malformed envelope');
    expect(() => encodeEnvelope({ ...base, sequence: -1 } as unknown as typeof base)).toThrow('Malformed envelope');
    expect(() => encodeEnvelope({ ...base, type: 'unknown' } as unknown as typeof base)).toThrow('Malformed envelope');
    expect(() => encodeEnvelope({ ...base, protocolVersion: 2 } as unknown as typeof base)).toThrow(
      'Unsupported protocol version 2',
    );
  });
});

describe('createRuntime validation', () => {
  it('rejects non-string workerUrl', () => {
    expect(() => createRuntime({ workerUrl: 123 as unknown as string })).toThrow('workerUrl must be a non-empty string');
  });

  it('rejects dangerous workerUrl schemes', () => {
    expect(() => createRuntime({ workerUrl: 'data:text/javascript,alert(1)' })).toThrow('workerUrl must use http:');
    expect(() => createRuntime({ workerUrl: 'javascript:alert(1)' })).toThrow('workerUrl must use http:');
    expect(() => createRuntime({ workerUrl: 'vbscript:msgbox(1)' })).toThrow('workerUrl must use http:');
  });

  it('rejects dangerous wasmUrl schemes', () => {
    expect(() => createRuntime({ workerUrl: 'mock-worker.js', wasmUrl: 'data:text/javascript,alert(1)' })).toThrow('wasmUrl must use http:');
  });

  it('allows relative, http, https, and blob URLs', () => {
    expect(() => createRuntime({ workerUrl: 'mock-worker.js' })).not.toThrow();
    expect(() => createRuntime({ workerUrl: 'https://example.com/worker.js' })).not.toThrow();
    expect(() => createRuntime({ workerUrl: 'blob:https://example.com/uuid' })).not.toThrow();
  });
});

describe('createRuntime worker integration', () => {
  let originalWorker: typeof Worker | undefined;

  beforeEach(() => {
    originalWorker = globalThis.Worker as unknown as typeof Worker;

    class MockWorker extends EventTarget {
      public onerror?: (event: ErrorEvent) => void;
      public onmessage?: (event: MessageEvent<string>) => void;
      public onmessageerror?: (event: MessageEvent) => void;

      constructor(public url: string | URL) {
        super();
      }

      postMessage(data: string): void {
        let envelope: ReturnType<typeof decodeEnvelope>;
        try {
          envelope = decodeEnvelope(data);
        } catch {
          return;
        }
        // Echo the same envelope back as the command reply.
        const reply = {
          ...envelope,
          payload: { acknowledged: envelope.type },
        };
        this.onmessage?.(new MessageEvent('message', { data: encodeEnvelope(reply) }));
      }

      terminate(): void {
        // noop
      }
    }

    globalThis.Worker = MockWorker as unknown as typeof Worker;
  });

  afterEach(() => {
    if (originalWorker) {
      globalThis.Worker = originalWorker;
    }
  });

  it('load resolves after worker reply', async () => {
    const runtime = createRuntime({ workerUrl: 'mock-worker.js' });
    await expect(runtime.load('http://example.com/test.flv')).resolves.toBeUndefined();
  });

  it('play sends a control command', async () => {
    const runtime = createRuntime({ workerUrl: 'mock-worker.js' });
    await runtime.load('http://example.com/test.flv');
    runtime.play();
  });

  it('seek resolves after worker reply', async () => {
    const runtime = createRuntime({ workerUrl: 'mock-worker.js' });
    await runtime.load('http://example.com/test.flv');
    await expect(runtime.seek(12345)).resolves.toBeUndefined();
  });

  it('seek rejects invalid timeMs', async () => {
    const runtime = createRuntime({ workerUrl: 'mock-worker.js' });
    await runtime.load('http://example.com/test.flv');
    await expect(runtime.seek(-1)).rejects.toThrow('seek timeMs');
    await expect(runtime.seek(NaN)).rejects.toThrow('seek timeMs');
  });

  it('setPlaybackRate resolves after worker reply', async () => {
    const runtime = createRuntime({ workerUrl: 'mock-worker.js' });
    await runtime.load('http://example.com/test.flv');
    await expect(runtime.setPlaybackRate(2)).resolves.toBeUndefined();
  });

  it('setPlaybackRate rejects out of range rate', async () => {
    const runtime = createRuntime({ workerUrl: 'mock-worker.js' });
    await runtime.load('http://example.com/test.flv');
    await expect(runtime.setPlaybackRate(0.05)).rejects.toThrow('playback rate');
    await expect(runtime.setPlaybackRate(20)).rejects.toThrow('playback rate');
    await expect(runtime.setPlaybackRate(NaN)).rejects.toThrow('playback rate');
  });

  it('frameStep rejects non-boolean keyframeOnly', async () => {
    const runtime = createRuntime({ workerUrl: 'mock-worker.js' });
    await runtime.load('http://example.com/test.flv');
    await expect(runtime.frameStep('forward', 'true' as unknown as boolean)).rejects.toThrow('keyframeOnly');
  });

  it('pauseDisplay rejects non-boolean keepConnection', async () => {
    const runtime = createRuntime({ workerUrl: 'mock-worker.js' });
    await runtime.load('http://example.com/test.flv');
    await expect(runtime.pauseDisplay('true' as unknown as boolean)).rejects.toThrow('keepConnection');
  });

  it('request rejects invalid timeoutMs', async () => {
    const runtime = createRuntime({ workerUrl: 'mock-worker.js' });
    await runtime.load('http://example.com/test.flv');
    await expect(runtime.request('play', undefined, -1)).rejects.toThrow('timeoutMs');
    await expect(runtime.request('play', undefined, NaN)).rejects.toThrow('timeoutMs');
  });

  it('request rejects non-JSON-serializable payloads instead of throwing synchronously', async () => {
    const runtime = createRuntime({ workerUrl: 'mock-worker.js' });
    await runtime.load('http://example.com/test.flv');
    const payload: Record<string, unknown> = { a: 1 };
    payload.self = payload;
    await expect(runtime.request('metrics', payload)).rejects.toThrow();
  });

  it('destroy rejects pending commands', async () => {
    class NoReplyWorker extends EventTarget {
      public onmessage?: (event: MessageEvent<string>) => void;
      postMessage(): void {
        // never reply
      }
      terminate(): void {
        // noop
      }
    }
    globalThis.Worker = NoReplyWorker as unknown as typeof Worker;

    const runtime = createRuntime({ workerUrl: 'mock-worker.js' });
    const loadPromise = runtime.load('http://example.com/test.flv');
    runtime.destroy();
    await expect(loadPromise).rejects.toThrow('Runtime destroyed');
  });
});
