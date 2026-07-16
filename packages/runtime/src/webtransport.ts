/**
 * WebTransport byte-stream transport.
 *
 * This is a skeleton implementation that reads incoming unidirectional streams
 * (or datagrams, if unidirectional streams are unavailable) and forwards the
 * chunks to the caller. Full media pipeline integration happens in later work
 * packages.
 */

import {
  type Chunk,
  makeError,
  type Transport,
  TransportErrorCode,
  type TransportConfig,
  type TransportError,
  validateWebTransportUrl,
} from './transport-common';

interface WebTransportCloseInfo {
  readonly closeCode?: number;
  readonly reason?: string;
}

/** A unidirectional receive stream is a `ReadableStream` of `Uint8Array`. */
type WebTransportReceiveStream = ReadableStream<Uint8Array>;

interface WebTransportDatagramDuplexStream {
  readonly readable: ReadableStream<Uint8Array>;
}

interface WebTransportHandle {
  readonly ready: Promise<void>;
  readonly closed: Promise<WebTransportCloseInfo>;
  readonly incomingUnidirectionalStreams: ReadableStream<WebTransportReceiveStream>;
  readonly datagrams: WebTransportDatagramDuplexStream;
  close(): Promise<void>;
}

interface WebTransportConstructor {
  new (url: string): WebTransportHandle;
  readonly prototype: WebTransportHandle;
}

function getWebTransportConstructor(): WebTransportConstructor | undefined {
  const g = globalThis as unknown as { WebTransport?: WebTransportConstructor };
  return g.WebTransport;
}

export class WebTransportTransport implements Transport {
  public readonly config: TransportConfig;

  private transport: WebTransportHandle | undefined;
  private streamsReader: ReadableStreamDefaultReader<WebTransportReceiveStream> | undefined;
  private datagramReader: ReadableStreamDefaultReader<Uint8Array> | undefined;
  private stopped = false;
  private bytesRead = 0;
  private maxBytes: number;

  constructor(config: TransportConfig) {
    this.config = config;
    this.maxBytes = config.maxBytes ?? Number.MAX_SAFE_INTEGER;
  }

  start(
    onChunk: (chunk: Chunk) => void,
    onError: (error: TransportError) => void,
    onEnd: () => void,
  ): void {
    if (this.stopped) {
      onError(makeError(TransportErrorCode.Canceled, 'Transport already stopped', false));
      onEnd();
      return;
    }

    const urlError = validateWebTransportUrl(this.config.url);
    if (urlError) {
      onError(urlError);
      onEnd();
      return;
    }

    const Ctor = getWebTransportConstructor();
    if (!Ctor) {
      onError(makeError(TransportErrorCode.WebTransportNotSupported, 'WebTransport API is not available', false));
      onEnd();
      return;
    }

    this.transport = new Ctor(this.config.url);
    this.run(this.transport, onChunk, onError, onEnd).catch((err) => {
      onError(this.toError(err));
      onEnd();
    });
  }

  private async run(
    transport: WebTransportHandle,
    onChunk: (chunk: Chunk) => void,
    onError: (error: TransportError) => void,
    onEnd: () => void,
  ): Promise<void> {
    await transport.ready;

    const streams = transport.incomingUnidirectionalStreams;
    if (streams) {
      this.streamsReader = streams.getReader();
      while (!this.stopped) {
        const { value: receiveStream, done } = await this.streamsReader.read();
        if (done || this.stopped) break;
        if (receiveStream) {
          this.readStream(receiveStream, onChunk, onError).catch((err) => {
            onError(this.toError(err));
          });
        }
      }
    } else if (transport.datagrams?.readable) {
      this.datagramReader = transport.datagrams.readable.getReader();
      while (!this.stopped) {
        const { value, done } = await this.datagramReader.read();
        if (done || this.stopped) break;
        if (value) this.deliver(value, onChunk, onError);
      }
    }

    onEnd();
  }

  private async readStream(
    stream: WebTransportReceiveStream,
    onChunk: (chunk: Chunk) => void,
    onError: (error: TransportError) => void,
  ): Promise<void> {
    const reader = stream.getReader();
    try {
      while (!this.stopped) {
        const { value, done } = await reader.read();
        if (done || this.stopped) break;
        if (value) this.deliver(value, onChunk, onError);
      }
    } finally {
      reader.releaseLock();
    }
  }

  private deliver(
    bytes: Uint8Array,
    onChunk: (chunk: Chunk) => void,
    onError: (error: TransportError) => void,
  ): boolean {
    if (this.bytesRead + bytes.length > this.maxBytes) {
      this.stop();
      onError(makeError(TransportErrorCode.MaxBytesExceeded, 'Max response size exceeded', false));
      return false;
    }
    this.bytesRead += bytes.length;
    onChunk({ bytes, timestamp: performance.now() });
    return true;
  }

  private toError(err: unknown): TransportError {
    if (err instanceof DOMException && err.name === 'AbortError') {
      return makeError(TransportErrorCode.Canceled, 'Transport stopped', false);
    }
    const message = err instanceof Error ? err.message : String(err);
    return makeError(TransportErrorCode.WebTransportClosed, message, false);
  }

  stop(): void {
    this.stopped = true;
    try {
      this.streamsReader?.releaseLock();
    } catch {
      // Reader may already be released if the stream closed.
    }
    this.streamsReader = undefined;
    try {
      this.datagramReader?.releaseLock();
    } catch {
      // Ignore release errors on closed streams.
    }
    this.datagramReader = undefined;
    this.transport?.close().catch(() => {
      // close() can reject if the transport is already closed; ignore.
    });
  }
}
