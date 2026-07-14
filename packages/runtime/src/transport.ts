/**
 * Unified network transport for media byte streams.
 *
 * The transport layer is agnostic to container formats; it only delivers
 * byte chunks and metadata to the caller. It supports both HTTP(S) fetch
 * streaming and WebSocket binary streaming, with backpressure and bounded
 * retry policies.
 */

export interface TransportConfig {
  readonly url: string;
  readonly method?: 'GET' | 'POST';
  readonly headers?: Record<string, string>;
  readonly credentials?: RequestCredentials;
  readonly referrer?: string;
  readonly timeoutMs?: number;
  readonly maxBytes?: number;
  readonly maxRetries?: number;
  readonly redirect?: RequestRedirect;
}

export interface TransportStats {
  readonly bytesRead: number;
  readonly chunks: number;
  readonly startedAt: number;
  readonly endedAt?: number;
}

export interface Chunk {
  readonly bytes: Uint8Array;
  readonly timestamp: number;
}

export interface TransportError {
  readonly code: number;
  readonly stage: 'transport';
  readonly message: string;
  readonly retryable: boolean;
}

export const TransportErrorCode = {
  InvalidUrl: 7001,
  InsecureContent: 7002,
  Network: 7003,
  HttpStatus: 7004,
  Timeout: 7005,
  Canceled: 7006,
  ContentLengthMismatch: 7007,
  MaxBytesExceeded: 7008,
  WebSocketClosed: 7009,
} as const;

function makeError(
  code: number,
  message: string,
  retryable: boolean,
): TransportError {
  return { code, stage: 'transport', message, retryable };
}

function validateUrl(url: string): TransportError | undefined {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return makeError(TransportErrorCode.InvalidUrl, `Invalid URL: ${url}`, false);
  }
  if (parsed.username || parsed.password) {
    return makeError(
      TransportErrorCode.InvalidUrl,
      'URL must not contain user credentials',
      false,
    );
  }
  if (
    (parsed.protocol === 'http:' || parsed.protocol === 'ws:') &&
    typeof globalThis !== 'undefined' &&
    (globalThis as unknown as { isSecureContext?: boolean }).isSecureContext
  ) {
    return makeError(
      TransportErrorCode.InsecureContent,
      'Plain-text transport is not allowed from a secure context',
      false,
    );
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:' && parsed.protocol !== 'ws:' && parsed.protocol !== 'wss:') {
    return makeError(
      TransportErrorCode.InvalidUrl,
      `Unsupported transport scheme: ${parsed.protocol}`,
      false,
    );
  }
  return undefined;
}

function sanitizeHeaders(headers: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(headers)) {
    const lower = key.toLowerCase();
    if (lower === 'authorization' || lower === 'cookie') continue;
    out[key] = value;
  }
  return out;
}

export interface Transport {
  readonly config: TransportConfig;
  start(onChunk: (chunk: Chunk) => void, onError: (error: TransportError) => void, onEnd: () => void): void;
  stop(): void;
}

/**
 * Fetch-based HTTP byte stream transport.
 *
 * Reads the response body chunk by chunk and respects a maximum byte limit.
 * Retries are only attempted before any data has been produced so that the
 * caller receives a contiguous, appendable byte stream.
 */
export class FetchTransport implements Transport {
  public readonly config: TransportConfig;

  private controller?: AbortController;
  private started = false;
  private bytesRead = 0;
  private retries = 0;

  constructor(config: TransportConfig) {
    this.config = config;
  }

  start(onChunk: (chunk: Chunk) => void, onError: (error: TransportError) => void, onEnd: () => void): void {
    if (this.started) return;
    this.started = true;

    const urlError = validateUrl(this.config.url);
    if (urlError) {
      onError(urlError);
      onEnd();
      return;
    }

    this.run(onChunk, onError, onEnd).catch((err) => {
      onError(this.toError(err));
      onEnd();
    });
  }

  private async run(
    onChunk: (chunk: Chunk) => void,
    onError: (error: TransportError) => void,
    onEnd: () => void,
  ): Promise<void> {
    const maxRetries = this.config.maxRetries ?? 0;
    const timeoutMs = this.config.timeoutMs ?? 30000;
    const maxBytes = this.config.maxBytes ?? Number.MAX_SAFE_INTEGER;
    const method = this.config.method ?? 'GET';
    const headers = sanitizeHeaders(this.config.headers ?? {});

    while (this.retries <= maxRetries) {
      this.controller = new AbortController();
      const timer = setTimeout(() => this.controller?.abort(), timeoutMs);

      try {
        const init: RequestInit = {
          method,
          headers,
          credentials: this.config.credentials ?? 'same-origin',
          redirect: this.config.redirect ?? 'follow',
          signal: this.controller.signal,
        };
        if (this.config.referrer !== undefined) {
          init.referrer = this.config.referrer;
        }
        const response = await fetch(this.config.url, init);

        if (!response.ok || (response.status !== 200 && response.status !== 206)) {
          clearTimeout(timer);
          onError(makeError(TransportErrorCode.HttpStatus, `HTTP ${response.status}`, false));
          onEnd();
          return;
        }

        if (!response.body) {
          clearTimeout(timer);
          onEnd();
          return;
        }

        const reader = response.body.getReader();
        let done = false;
        while (!done) {
          const { value, done: d } = await reader.read();
          done = d;
          if (value) {
            if (this.bytesRead + value.byteLength > maxBytes) {
              clearTimeout(timer);
              reader.releaseLock();
              onError(makeError(TransportErrorCode.MaxBytesExceeded, 'Max response size exceeded', false));
              onEnd();
              return;
            }
            this.bytesRead += value.byteLength;
            onChunk({ bytes: new Uint8Array(value), timestamp: performance.now() });
          }
        }

        clearTimeout(timer);
        onEnd();
        return;
      } catch (err) {
        clearTimeout(timer);
        const transportError = this.toError(err);
        if (transportError.code === TransportErrorCode.Canceled) {
          onError(transportError);
          onEnd();
          return;
        }
        if (this.bytesRead > 0 || this.retries >= maxRetries) {
          onError(transportError);
          onEnd();
          return;
        }
        this.retries += 1;
      }
    }

    onError(makeError(TransportErrorCode.Network, 'Max retries exceeded', false));
    onEnd();
  }

  private toError(err: unknown): TransportError {
    if (err instanceof DOMException && err.name === 'AbortError') {
      return makeError(TransportErrorCode.Canceled, 'Transport stopped', false);
    }
    const message = err instanceof Error ? err.message : String(err);
    return makeError(TransportErrorCode.Network, message, true);
  }

  stop(): void {
    this.controller?.abort();
  }
}

interface WebSocketConfig extends TransportConfig {
  readonly binaryType?: 'arraybuffer' | 'blob';
}

/**
 * WebSocket binary transport.
 *
 * Each incoming binary message is treated as an opaque chunk; message
 * boundaries are not container boundaries. Reconnection uses bounded
 * exponential backoff with jitter and creates a fresh connection.
 */
export class WebSocketTransport implements Transport {
  public readonly config: WebSocketConfig;

  private socket?: WebSocket;
  private controller?: AbortController;
  private reconnectAttempts = 0;
  private started = false;
  private bytesRead = 0;
  private maxBytes: number;

  constructor(config: WebSocketConfig) {
    this.config = config;
    this.maxBytes = config.maxBytes ?? Number.MAX_SAFE_INTEGER;
  }

  start(onChunk: (chunk: Chunk) => void, onError: (error: TransportError) => void, onEnd: () => void): void {
    if (this.started) return;
    this.started = true;

    const urlError = validateUrl(this.config.url);
    if (urlError) {
      onError(urlError);
      onEnd();
      return;
    }

    this.controller = new AbortController();
    this.connect(onChunk, onError, onEnd).catch((err) => {
      onError(this.toError(err));
      onEnd();
    });
  }

  private async connect(
    onChunk: (chunk: Chunk) => void,
    onError: (error: TransportError) => void,
    onEnd: () => void,
  ): Promise<void> {
    const socket = new WebSocket(this.config.url);
    socket.binaryType = this.config.binaryType ?? 'arraybuffer';

    return new Promise<void>((resolve, reject) => {
      const onOpen = () => {
        this.reconnectAttempts = 0;
      };

      const deliver = (bytes: Uint8Array): void => {
        if (this.bytesRead + bytes.length > this.maxBytes) {
          socket.close();
          onError(makeError(TransportErrorCode.MaxBytesExceeded, 'Max response size exceeded', false));
          onEnd();
          reject(new Error('Max bytes exceeded'));
          return;
        }
        this.bytesRead += bytes.length;
        onChunk({ bytes, timestamp: performance.now() });
      };

      const onMessage = (event: MessageEvent) => {
        if (typeof event.data === 'string') {
          // Text messages are not media byte chunks. Discard silently.
          return;
        }
        const data = event.data as ArrayBuffer | Blob;
        if (data instanceof Blob) {
          data.arrayBuffer().then((buffer) => deliver(new Uint8Array(buffer))).catch(reject);
          return;
        }
        deliver(new Uint8Array(data as ArrayBuffer));
      };

      const onClose = (event: CloseEvent) => {
        if (this.controller?.signal.aborted) {
          onError(makeError(TransportErrorCode.Canceled, 'Transport stopped', false));
          onEnd();
          resolve();
          return;
        }
        if (event.code === 1000) {
          // Normal server-side end of stream.
          onEnd();
          resolve();
          return;
        }
        const maxRetries = this.config.maxRetries ?? 0;
        if (this.reconnectAttempts < maxRetries) {
          this.reconnectAttempts += 1;
          const delay = Math.min(1000 * 2 ** this.reconnectAttempts, 30000) + Math.random() * 1000;
          setTimeout(() => {
            this.connect(onChunk, onError, onEnd).catch(reject);
          }, delay);
        } else {
          onError(makeError(TransportErrorCode.WebSocketClosed, `Closed ${event.code}: ${event.reason}`, false));
          onEnd();
          resolve();
        }
      };

      const onErrorEvent = () => {
        // Do nothing here; the close event carries the final status.
      };

      socket.addEventListener('open', onOpen);
      socket.addEventListener('message', onMessage);
      socket.addEventListener('close', onClose);
      socket.addEventListener('error', onErrorEvent);

      this.socket = socket;
    });
  }

  private toError(err: unknown): TransportError {
    if (err instanceof DOMException && err.name === 'AbortError') {
      return makeError(TransportErrorCode.Canceled, 'Transport stopped', false);
    }
    const message = err instanceof Error ? err.message : String(err);
    return makeError(TransportErrorCode.Network, message, true);
  }

  stop(): void {
    this.controller?.abort();
    this.socket?.close();
  }
}

/**
 * Create a transport based on URL scheme.
 */
export function createTransport(config: TransportConfig): Transport {
  const url = new URL(config.url);
  if (url.protocol === 'ws:' || url.protocol === 'wss:') {
    return new WebSocketTransport(config as WebSocketConfig);
  }
  return new FetchTransport(config);
}
