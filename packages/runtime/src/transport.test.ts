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

  beforeEach(() => {
    originalFetch = globalThis.fetch;
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
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
});

describe('WebSocketTransport', () => {
  let originalWebSocket: typeof WebSocket;

  beforeEach(() => {
    originalWebSocket = globalThis.WebSocket as typeof WebSocket;
  });

  afterEach(() => {
    globalThis.WebSocket = originalWebSocket;
  });

  it('delivers binary messages as chunks', async () => {
    class MockSocket extends EventTarget {
      public url: string;
      public binaryType: BinaryType = 'arraybuffer';
      public readyState: number = WebSocket.CONNECTING;
      constructor(url: string | URL) {
        super();
        this.url = String(url);
        setTimeout(() => {
          this.readyState = WebSocket.OPEN;
          this.dispatchEvent(new Event('open'));
          this.dispatchEvent(new MessageEvent('message', { data: new Uint8Array([0x01, 0x02]).buffer }));
          this.dispatchEvent(new CloseEvent('close', { code: 1000, reason: 'done' }));
        }, 0);
      }
      public close(): void {
        this.readyState = WebSocket.CLOSED;
      }
    }
    globalThis.WebSocket = MockSocket as unknown as typeof WebSocket;

    const transport = new WebSocketTransport({ url: 'wss://example.com/stream' });
    const chunks: Uint8Array[] = [];
    await new Promise<void>((resolve) => {
      transport.start(
        (chunk) => chunks.push(chunk.bytes),
        () => { /* no-op */ },
        () => resolve(),
      );
    });

    expect(chunks).toHaveLength(1);
    expect(chunks[0]).toEqual(new Uint8Array([0x01, 0x02]));
  });

  it('ignores text messages', async () => {
    class MockSocket extends EventTarget {
      public url: string;
      public binaryType: BinaryType = 'arraybuffer';
      public readyState: number = WebSocket.CONNECTING;
      constructor(url: string | URL) {
        super();
        this.url = String(url);
        setTimeout(() => {
          this.readyState = WebSocket.OPEN;
          this.dispatchEvent(new Event('open'));
          this.dispatchEvent(new MessageEvent('message', { data: 'control' }));
          this.dispatchEvent(new CloseEvent('close', { code: 1000, reason: 'done' }));
        }, 0);
      }
      public close(): void {
        this.readyState = WebSocket.CLOSED;
      }
    }
    globalThis.WebSocket = MockSocket as unknown as typeof WebSocket;

    const transport = new WebSocketTransport({ url: 'wss://example.com/stream' });
    const chunks: Uint8Array[] = [];
    await new Promise<void>((resolve) => {
      transport.start(
        (chunk) => chunks.push(chunk.bytes),
        () => { /* no-op */ },
        () => resolve(),
      );
    });

    expect(chunks).toHaveLength(0);
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
