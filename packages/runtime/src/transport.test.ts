import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { createTransport, FetchTransport, WebSocketTransport, TransportErrorCode } from './transport';

class CloseEvent extends Event {
  public readonly code: number;
  public readonly reason: string;
  constructor(type: string, init: { code: number; reason: string }) {
    super(type);
    this.code = init.code;
    this.reason = init.reason;
  }
}

function buildChunks(values: number[][]): ReadableStream<Uint8Array> {
  return new ReadableStream({
    start(controller) {
      for (const value of values) {
        controller.enqueue(new Uint8Array(value));
      }
      controller.close();
    },
  });
}

describe('FetchTransport', () => {
  let originalFetch: typeof fetch;
  let originalIsSecureContext: PropertyDescriptor | undefined;

  beforeEach(() => {
    originalFetch = globalThis.fetch;
    originalIsSecureContext = Object.getOwnPropertyDescriptor(globalThis, 'isSecureContext');
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    if (originalIsSecureContext) {
      Object.defineProperty(globalThis, 'isSecureContext', originalIsSecureContext);
    } else {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      delete (globalThis as any).isSecureContext;
    }
  });

  it('delivers chunks for a 200 response', async () => {
    const chunks: Uint8Array[] = [];
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      body: buildChunks([[0x01, 0x02], [0x03, 0x04]]),
    } as unknown as Response);

    const transport = new FetchTransport({ url: 'https://example.com/stream' });
    await new Promise<void>((resolve, reject) => {
      transport.start(
        (chunk) => chunks.push(chunk.bytes),
        (err) => reject(new Error(err.message)),
        () => resolve(),
      );
    });

    expect(chunks).toHaveLength(2);
    expect(chunks[0]).toEqual(new Uint8Array([0x01, 0x02]));
    expect(chunks[1]).toEqual(new Uint8Array([0x03, 0x04]));
  });

  it('rejects HTTP 404', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 404,
      body: null,
    } as unknown as Response);

    const transport = new FetchTransport({ url: 'https://example.com/stream' });
    const err = await new Promise<{ code: number; message: string }>((resolve, reject) => {
      transport.start(
        () => reject(new Error('unexpected chunk')),
        (error) => resolve(error),
        () => reject(new Error('unexpected end')),
      );
    });

    expect(err.code).toBe(TransportErrorCode.HttpStatus);
  });

  it('enforces max bytes', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      body: buildChunks([[0x01, 0x02, 0x03, 0x04, 0x05]]),
    } as unknown as Response);

    const transport = new FetchTransport({ url: 'https://example.com/stream', maxBytes: 3 });
    const err = await new Promise<{ code: number }>((resolve, reject) => {
      transport.start(
        () => { /* no-op */ },
        (error) => resolve(error),
        () => reject(new Error('unexpected end')),
      );
    });

    expect(err.code).toBe(TransportErrorCode.MaxBytesExceeded);
  });

  it('rejects URLs with credentials', async () => {
    const transport = new FetchTransport({ url: 'https://user:pass@example.com/stream' });
    const err = await new Promise<{ code: number }>((resolve, reject) => {
      transport.start(
        () => reject(new Error('unexpected chunk')),
        (error) => resolve(error),
        () => reject(new Error('unexpected end')),
      );
    });

    expect(err.code).toBe(TransportErrorCode.InvalidUrl);
  });

  it('rejects plain HTTP from secure context', async () => {
    Object.defineProperty(globalThis, 'isSecureContext', { value: true, configurable: true });
    const transport = new FetchTransport({ url: 'http://example.com/stream' });
    const err = await new Promise<{ code: number }>((resolve, reject) => {
      transport.start(
        () => reject(new Error('unexpected chunk')),
        (error) => resolve(error),
        () => reject(new Error('unexpected end')),
      );
    });

    expect(err.code).toBe(TransportErrorCode.InsecureContent);
  });
});

describe('WebSocketTransport', () => {
  let originalWebSocket: typeof WebSocket;
  let originalIsSecureContext: PropertyDescriptor | undefined;

  beforeEach(() => {
    originalWebSocket = globalThis.WebSocket as typeof WebSocket;
    originalIsSecureContext = Object.getOwnPropertyDescriptor(globalThis, 'isSecureContext');
  });

  afterEach(() => {
    globalThis.WebSocket = originalWebSocket;
    if (originalIsSecureContext) {
      Object.defineProperty(globalThis, 'isSecureContext', originalIsSecureContext);
    } else {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      delete (globalThis as any).isSecureContext;
    }
  });

  function createMockSocket(messages: unknown[], closeCode = 1000, closeReason = 'done'): typeof WebSocket {
    class MockSocket extends EventTarget {
      public url: string;
      public binaryType: BinaryType = 'arraybuffer';
      public readyState: number = WebSocket.CONNECTING;
      constructor(url: string | URL) {
        super();
        this.url = String(url);
        setTimeout(async () => {
          this.readyState = WebSocket.OPEN;
          this.dispatchEvent(new Event('open'));
          for (const message of messages) {
            this.dispatchEvent(new MessageEvent('message', { data: message }));
          }
          // Yield so async Blob.arrayBuffer() microtasks can run before close.
          await Promise.resolve();
          this.dispatchEvent(new CloseEvent('close', { code: closeCode, reason: closeReason }));
        }, 0);
      }
      public close(): void {
        this.readyState = WebSocket.CLOSED;
      }
    }
    return MockSocket as unknown as typeof WebSocket;
  }

  it('delivers binary messages as chunks', async () => {
    globalThis.WebSocket = createMockSocket([new Uint8Array([0x01, 0x02]).buffer]);

    const transport = new WebSocketTransport({ url: 'wss://example.com/stream' });
    const chunks: Uint8Array[] = [];
    const errors: { code: number }[] = [];
    await new Promise<void>((resolve) => {
      transport.start(
        (chunk) => chunks.push(chunk.bytes),
        (error) => errors.push(error),
        () => resolve(),
      );
    });

    expect(chunks).toHaveLength(1);
    expect(chunks[0]).toEqual(new Uint8Array([0x01, 0x02]));
    expect(errors).toHaveLength(0);
  });

  it('delivers Blob messages as chunks', async () => {
    globalThis.WebSocket = createMockSocket([new Blob([new Uint8Array([0x03, 0x04])])]);

    const transport = new WebSocketTransport({ url: 'wss://example.com/stream' });
    const chunks: Uint8Array[] = [];
    await new Promise<void>((resolve, reject) => {
      transport.start(
        (chunk) => chunks.push(chunk.bytes),
        (error) => reject(new Error(error.message)),
        () => resolve(),
      );
    });

    expect(chunks).toHaveLength(1);
    expect(chunks[0]).toEqual(new Uint8Array([0x03, 0x04]));
  });

  it('ignores text messages', async () => {
    globalThis.WebSocket = createMockSocket(['control']);

    const transport = new WebSocketTransport({ url: 'wss://example.com/stream' });
    const chunks: Uint8Array[] = [];
    await new Promise<void>((resolve, reject) => {
      transport.start(
        (chunk) => chunks.push(chunk.bytes),
        (error) => reject(new Error(error.message)),
        () => resolve(),
      );
    });

    expect(chunks).toHaveLength(0);
  });

  it('reports abnormal close as error', async () => {
    globalThis.WebSocket = createMockSocket([], 1006, 'abnormal');

    const transport = new WebSocketTransport({ url: 'wss://example.com/stream' });
    const err = await new Promise<{ code: number }>((resolve, reject) => {
      transport.start(
        () => reject(new Error('unexpected chunk')),
        (error) => resolve(error),
        () => reject(new Error('unexpected end')),
      );
    });

    expect(err.code).toBe(TransportErrorCode.WebSocketClosed);
  });

  it('rejects plain ws: from secure context', async () => {
    Object.defineProperty(globalThis, 'isSecureContext', { value: true, configurable: true });
    const transport = new WebSocketTransport({ url: 'ws://example.com/stream' });
    const err = await new Promise<{ code: number }>((resolve, reject) => {
      transport.start(
        () => reject(new Error('unexpected chunk')),
        (error) => resolve(error),
        () => reject(new Error('unexpected end')),
      );
    });

    expect(err.code).toBe(TransportErrorCode.InsecureContent);
  });
});

describe('createTransport', () => {
  it('selects WebSocketTransport for ws://', () => {
    const transport = createTransport({ url: 'wss://example.com/stream' });
    expect(transport).toBeInstanceOf(WebSocketTransport);
  });

  it('selects FetchTransport for https://', () => {
    const transport = createTransport({ url: 'https://example.com/stream' });
    expect(transport).toBeInstanceOf(FetchTransport);
  });
});
