import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { WebTransportTransport, TransportErrorCode } from './transport';

describe('WebTransportTransport', () => {
  let originalWebTransport: unknown;
  let originalIsSecureContext: PropertyDescriptor | undefined;

  beforeEach(() => {
    originalWebTransport = (globalThis as unknown as { WebTransport?: unknown }).WebTransport;
    originalIsSecureContext = Object.getOwnPropertyDescriptor(globalThis, 'isSecureContext');
    Object.defineProperty(globalThis, 'isSecureContext', { value: true, configurable: true });
  });

  afterEach(() => {
    (globalThis as unknown as { WebTransport?: unknown }).WebTransport = originalWebTransport;
    if (originalIsSecureContext) {
      Object.defineProperty(globalThis, 'isSecureContext', originalIsSecureContext);
    } else {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      delete (globalThis as any).isSecureContext;
    }
  });

  function buildByteStream(values: number[][]): ReadableStream<Uint8Array> {
    return new ReadableStream({
      start(controller) {
        for (const value of values) {
          controller.enqueue(new Uint8Array(value));
        }
        controller.close();
      },
    });
  }

  function buildStreamOfStreams(streams: ReadableStream<Uint8Array>[]): ReadableStream<ReadableStream<Uint8Array>> {
    return new ReadableStream({
      start(controller) {
        for (const stream of streams) {
          controller.enqueue(stream);
        }
        controller.close();
      },
    });
  }

  it('delivers chunks from incoming unidirectional streams', async () => {
    const chunks: Uint8Array[] = [];
    let closeCalled = false;

    class MockTransport {
      public readonly url: string;
      public readonly ready = Promise.resolve();
      public readonly incomingUnidirectionalStreams = buildStreamOfStreams([
        buildByteStream([[0x01]]),
        buildByteStream([[0x02, 0x03]]),
      ]);
      constructor(url: string) {
        this.url = url;
      }
      public async close(): Promise<void> {
        closeCalled = true;
      }
    }

    (globalThis as unknown as { WebTransport: new (url: string) => unknown }).WebTransport = MockTransport as unknown as new (url: string) => unknown;

    const transport = new WebTransportTransport({ url: 'https://example.com/stream' });
    await new Promise<void>((resolve) => {
      transport.start(
        (chunk) => chunks.push(chunk.bytes),
        (err) => { throw new Error(err.message); },
        () => resolve(),
      );
    });

    expect(chunks).toHaveLength(2);
    expect(chunks[0]).toEqual(new Uint8Array([0x01]));
    expect(chunks[1]).toEqual(new Uint8Array([0x02, 0x03]));
    // The transport is explicitly closed when the incoming streams finish.
    expect(closeCalled).toBe(true);
  });

  it('reports an error when WebTransport is unavailable', async () => {
    (globalThis as unknown as { WebTransport?: unknown }).WebTransport = undefined;

    const transport = new WebTransportTransport({ url: 'https://example.com/stream' });
    const err = await new Promise<{ code: number }>((resolve, reject) => {
      transport.start(
        () => reject(new Error('unexpected chunk')),
        (error) => resolve(error),
        () => reject(new Error('unexpected end')),
      );
    });

    expect(err.code).toBe(TransportErrorCode.WebTransportNotSupported);
  });

  it('rejects non-https URLs', async () => {
    class MockTransport {
      public readonly ready = Promise.resolve();
    }
    (globalThis as unknown as { WebTransport: new (url: string) => unknown }).WebTransport = MockTransport as unknown as new (url: string) => unknown;

    const transport = new WebTransportTransport({ url: 'http://example.com/stream' });
    const err = await new Promise<{ code: number }>((resolve, reject) => {
      transport.start(
        () => reject(new Error('unexpected chunk')),
        (error) => resolve(error),
        () => reject(new Error('unexpected end')),
      );
    });

    expect(err.code).toBe(TransportErrorCode.InvalidUrl);
  });

  it('stops and closes the transport', async () => {
    let closeCalled = false;
    class MockTransport {
      public readonly ready = Promise.resolve();
      public readonly incomingUnidirectionalStreams = new ReadableStream<ReadableStream<Uint8Array>>({
        start(controller) {
          controller.close();
        },
      });
      public async close(): Promise<void> {
        closeCalled = true;
      }
    }
    (globalThis as unknown as { WebTransport: new (url: string) => unknown }).WebTransport = MockTransport as unknown as new (url: string) => unknown;

    const transport = new WebTransportTransport({ url: 'https://example.com/stream' });
    transport.start(() => { /* no-op */ }, () => { /* no-op */ }, () => { /* no-op */ });
    await new Promise((resolve) => setTimeout(resolve, 10));
    transport.stop();
    expect(closeCalled).toBe(true);
  });

  it('times out when the unidirectional stream accept stalls', async () => {
    class MockTransport {
      public readonly ready = Promise.resolve();
      public readonly incomingUnidirectionalStreams = new ReadableStream<ReadableStream<Uint8Array>>({
        start() {
          // Never enqueue or close; simulates a stalled server.
        },
      });
      public async close(): Promise<void> {}
    }
    (globalThis as unknown as { WebTransport: new (url: string) => unknown }).WebTransport =
      MockTransport as unknown as new (url: string) => unknown;

    const transport = new WebTransportTransport({
      url: 'https://example.com/stream',
      timeoutMs: 50,
    });
    const err = await new Promise<{ code: number }>((resolve, reject) => {
      transport.start(
        () => reject(new Error('unexpected chunk')),
        (error) => resolve(error),
        () => {},
      );
    });
    expect(err.code).toBe(TransportErrorCode.Timeout);
  });
});
