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
  validateTransportConfig,
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
  private started = false;
  private stopped = false;
  private ended = false;
  private closed = false;
  private bytesRead = 0;
  private maxBytes: number;
  private timeoutMs = 30000;
  private timedOut = false;
  private onError?: (error: TransportError) => void;
  private onEnd?: () => void;
  private inFlight = new Set<Promise<void>>();

  constructor(config: TransportConfig) {
    if (!config || typeof config !== 'object' || typeof config.url !== 'string' || config.url.length === 0) {
      throw new Error('WebTransportTransport config requires a non-empty url string');
    }
    this.config = config;
    this.maxBytes = config.maxBytes ?? Number.MAX_SAFE_INTEGER;
  }

  start(
    onChunk: (chunk: Chunk) => void,
    onError: (error: TransportError) => void,
    onEnd: () => void,
  ): void {
    if (this.ended) {
      onError(makeError(TransportErrorCode.Canceled, 'Transport already ended', false));
      onEnd();
      return;
    }
    if (this.started) return;
    this.started = true;

    this.onError = onError;
    this.onEnd = onEnd;

    const configError = validateTransportConfig(this.config);
    if (configError) {
      this.finish(configError);
      return;
    }

    const urlError = validateWebTransportUrl(this.config.url);
    if (urlError) {
      this.finish(urlError);
      return;
    }
    this.timeoutMs = this.config.timeoutMs ?? 30000;
    this.timedOut = false;

    const Ctor = getWebTransportConstructor();
    if (!Ctor) {
      this.finish(makeError(TransportErrorCode.WebTransportNotSupported, 'WebTransport API is not available', false));
      return;
    }

    this.transport = new Ctor(this.config.url);
    this.run(this.transport, onChunk).catch((err) => {
      const error = this.toError(err);
      this.stop();
      this.finish(error);
    });
  }

  private finish(error?: TransportError): void {
    if (this.ended) return;
    this.ended = true;
    if (error) {
      this.onError?.(error);
    }
    this.onEnd?.();
  }

  private withTimeout<T>(promise: Promise<T>, message: string): Promise<T> {
    if (!Number.isFinite(this.timeoutMs) || this.timeoutMs <= 0) {
      return promise;
    }
    let settled = false;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        settled = true;
        this.timedOut = true;
        if (this.transport) {
          this.transport.close().catch(() => undefined);
        }
        reject(new Error(message));
      }, this.timeoutMs);

      promise.then(
        (value) => {
          if (settled) return;
          settled = true;
          clearTimeout(timer);
          resolve(value);
        },
        (reason) => {
          if (settled) return;
          settled = true;
          clearTimeout(timer);
          reject(reason);
        },
      );
    });
  }

  private async run(
    transport: WebTransportHandle,
    onChunk: (chunk: Chunk) => void,
  ): Promise<void> {
    await this.withTimeout(transport.ready, 'WebTransport connection timed out');

    const streams = transport.incomingUnidirectionalStreams;
    if (streams) {
      this.streamsReader = streams.getReader();
      while (!this.stopped) {
        const { value: receiveStream, done } = await this.streamsReader.read();
        if (done || this.stopped) break;
        if (receiveStream) {
          const promise = this.readStream(receiveStream, onChunk).catch((err) => {
            if (!this.stopped) {
              const failure = this.toError(err);
              this.stop();
              this.finish(failure);
            }
          });
          this.inFlight.add(promise);
          void promise.finally(() => {
            this.inFlight.delete(promise);
          });
        }
      }
      // Wait for any per-stream reads that are still draining buffered chunks
      // before signalling end-of-stream to the caller.
      await Promise.all(this.inFlight);
    } else if (transport.datagrams?.readable) {
      this.datagramReader = transport.datagrams.readable.getReader();
      while (!this.stopped) {
        const { value, done } = await this.datagramReader.read();
        if (done || this.stopped) break;
        if (value) this.deliver(value, onChunk);
      }
    }

    // The WebTransport session must be explicitly closed; reaching end-of-
    // stream on the incoming readable does not release the network session.
    await this.closeTransport();

    this.finish();
  }

  private async readStream(
    stream: WebTransportReceiveStream,
    onChunk: (chunk: Chunk) => void,
  ): Promise<void> {
    const reader = stream.getReader();
    try {
      while (!this.stopped) {
        const { value, done } = await reader.read();
        if (done || this.stopped) break;
        if (value) this.deliver(value, onChunk);
      }
    } finally {
      reader.releaseLock();
    }
  }

  private deliver(bytes: Uint8Array, onChunk: (chunk: Chunk) => void): boolean {
    if (this.bytesRead + bytes.length > this.maxBytes) {
      this.stop();
      this.finish(makeError(TransportErrorCode.MaxBytesExceeded, 'Max response size exceeded', false));
      return false;
    }
    this.bytesRead += bytes.length;
    onChunk({ bytes, timestamp: performance.now() });
    return true;
  }

  private toError(err: unknown): TransportError {
    if (this.timedOut) {
      return makeError(TransportErrorCode.Timeout, 'WebTransport operation timed out', true);
    }
    if (this.stopped) {
      return makeError(TransportErrorCode.Canceled, 'Transport stopped', false);
    }
    if (err instanceof DOMException && err.name === 'AbortError') {
      return makeError(TransportErrorCode.Canceled, 'Transport stopped', false);
    }
    const message = err instanceof Error ? err.message : String(err);
    return makeError(TransportErrorCode.WebTransportClosed, message, false);
  }

  stop(): void {
    this.stopped = true;
    // Closing the transport terminates pending stream reads cleanly; do not
    // release reader locks while a read is in flight, which would reject with
    // a TypeError and be reported as a transport failure.
    this.closeTransport().catch(() => {
      // close() can reject if already closed; ignore.
    });
  }

  private async closeTransport(): Promise<void> {
    if (!this.transport || this.closed) return;
    this.closed = true;
    await this.transport.close();
  }
}
