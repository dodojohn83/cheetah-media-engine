/**
 * Unified network transport for media byte streams.
 *
 * The transport layer is agnostic to container formats; it only delivers
 * byte chunks and metadata to the caller. It supports HTTP(S) fetch,
 * WebSocket binary streaming, WebTransport unidirectional streams and
 * WebRTC data channels, with backpressure and bounded retry policies.
 */

import {
  type Chunk,
  makeError,
  type Transport,
  TransportErrorCode,
  type TransportConfig,
  type TransportError,
  validateUrl,
  validateTransportConfig,
} from './transport-common';

import { WebRtcTransport } from './webrtc';
import { WebTransportTransport } from './webtransport';
export { WebRtcTransport } from './webrtc';
export { WebTransportTransport } from './webtransport';
export {
  type Chunk,
  type Transport,
  type TransportConfig,
  type TransportError,
  type TransportStats,
  TransportErrorCode,
  makeError,
  validateUrl,
} from './transport-common';

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
  private timedOut = false;

  constructor(config: TransportConfig) {
    this.config = config;
  }

  start(onChunk: (chunk: Chunk) => void, onError: (error: TransportError) => void, onEnd: () => void): void {
    if (this.started) return;
    this.started = true;

    const configError = validateTransportConfig(this.config);
    if (configError) {
      onError(configError);
      onEnd();
      return;
    }

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
    const connectTimeoutMs = Math.min(timeoutMs, 5000);
    const maxBytes = this.config.maxBytes ?? Number.MAX_SAFE_INTEGER;
    const method = this.config.method ?? 'GET';
    const headers = this.config.headers ?? {};

    while (this.retries <= maxRetries) {
      this.timedOut = false;
      this.controller = new AbortController();
      let timer: ReturnType<typeof setTimeout> | undefined;
      const startIdleTimer = (duration = timeoutMs) => {
        if (timer !== undefined) {
          clearTimeout(timer);
        }
        timer = setTimeout(() => {
          this.timedOut = true;
          this.controller?.abort();
        }, duration);
      };
      startIdleTimer(connectTimeoutMs);

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
        startIdleTimer();
        while (!done) {
          const { value, done: d } = await reader.read();
          done = d;
          if (value) {
            if (this.bytesRead + value.byteLength > maxBytes) {
              this.controller?.abort();
              clearTimeout(timer);
              onError(makeError(TransportErrorCode.MaxBytesExceeded, 'Max response size exceeded', false));
              onEnd();
              return;
            }
            this.bytesRead += value.byteLength;
            onChunk({ bytes: new Uint8Array(value), timestamp: performance.now() });
            startIdleTimer();
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
      if (this.timedOut) {
        return makeError(TransportErrorCode.Timeout, 'Request timed out', true);
      }
      return makeError(TransportErrorCode.Canceled, 'Transport stopped', false);
    }
    const message = err instanceof Error ? err.message : String(err);
    return makeError(TransportErrorCode.Network, message, true);
  }

  stop(): void {
    this.timedOut = false;
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
  private reconnectTimer: ReturnType<typeof setTimeout> | undefined = undefined;
  private idleTimer: ReturnType<typeof setTimeout> | undefined = undefined;
  private started = false;
  private ended = false;
  private bytesRead = 0;
  private maxBytes: number;
  private timeoutMs: number;
  private onError?: (error: TransportError) => void;
  private onEnd?: () => void;

  constructor(config: WebSocketConfig) {
    this.config = config;
    this.maxBytes = config?.maxBytes ?? Number.MAX_SAFE_INTEGER;
    this.timeoutMs = config?.timeoutMs ?? 30000;
  }

  start(onChunk: (chunk: Chunk) => void, onError: (error: TransportError) => void, onEnd: () => void): void {
    if (this.started) return;
    this.started = true;
    this.onError = onError;
    this.onEnd = onEnd;

    const configError = validateTransportConfig(this.config);
    if (configError) {
      this.finish(configError);
      return;
    }

    const urlError = validateUrl(this.config.url);
    if (urlError) {
      this.finish(urlError);
      return;
    }

    this.controller = new AbortController();
    this.connect(onChunk).catch((err) => {
      this.finish(this.toError(err));
    });
  }

  private finish(error?: TransportError): void {
    if (this.ended) return;
    this.ended = true;
    this.clearIdleTimer();
    this.clearReconnectTimer();
    if (error) {
      this.onError?.(error);
    }
    this.onEnd?.();
  }

  private async connect(onChunk: (chunk: Chunk) => void): Promise<void> {
    const socket = new WebSocket(this.config.url);
    socket.binaryType = this.config.binaryType ?? 'arraybuffer';
    this.startIdleTimer();

    return new Promise<void>((resolve, reject) => {
      const onOpen = () => {
        this.reconnectAttempts = 0;
        this.startIdleTimer();
      };

      const deliver = (bytes: Uint8Array): void => {
        if (this.ended) return;
        if (this.bytesRead + bytes.length > this.maxBytes) {
          socket.close();
          this.finish(makeError(TransportErrorCode.MaxBytesExceeded, 'Max response size exceeded', false));
          reject(new Error('Max bytes exceeded'));
          return;
        }
        this.bytesRead += bytes.length;
        onChunk({ bytes, timestamp: performance.now() });
        this.startIdleTimer();
      };

      const onMessage = (event: MessageEvent) => {
        if (this.ended || this.controller?.signal.aborted) {
          return;
        }
        this.startIdleTimer();
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
        this.clearIdleTimer();
        if (this.ended) {
          resolve();
          return;
        }
        if (this.controller?.signal.aborted) {
          this.finish(makeError(TransportErrorCode.Canceled, 'Transport stopped', false));
          resolve();
          return;
        }
        if (event.code === 1000) {
          // Normal server-side end of stream.
          this.finish();
          resolve();
          return;
        }
        const maxRetries = this.config.maxRetries ?? 0;
        if (this.reconnectAttempts < maxRetries) {
          this.reconnectAttempts += 1;
          const delay = Math.min(1000 * 2 ** this.reconnectAttempts, 30000) + Math.random() * 1000;
          this.reconnectTimer = setTimeout(() => {
            this.reconnectTimer = undefined;
            this.connect(onChunk).catch((err) => {
              this.finish(this.toError(err));
            });
          }, delay);
          resolve();
        } else {
          this.finish(makeError(TransportErrorCode.WebSocketClosed, `Closed ${event.code}: ${event.reason}`, false));
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
    this.clearIdleTimer();
    this.clearReconnectTimer();
    if (!this.ended) {
      this.finish(makeError(TransportErrorCode.Canceled, 'Transport stopped', false));
    }
    this.controller?.abort();
    this.socket?.close();
  }

  private startIdleTimer(): void {
    if (this.idleTimer !== undefined) {
      clearTimeout(this.idleTimer);
    }
    this.idleTimer = setTimeout(() => {
      this.idleTimer = undefined;
      this.finish(makeError(TransportErrorCode.Timeout, 'WebSocket idle/connect timeout', true));
      this.socket?.close();
    }, this.timeoutMs);
  }

  private clearIdleTimer(): void {
    if (this.idleTimer !== undefined) {
      clearTimeout(this.idleTimer);
      this.idleTimer = undefined;
    }
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer !== undefined) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = undefined;
    }
  }
}

/**
 * Create a transport based on URL scheme or an explicit mode hint.
 */
export function createTransport(
  config: TransportConfig,
  mode?: 'fetch' | 'websocket' | 'webtransport' | 'webrtc',
): Transport {
  if (mode === 'webtransport') {
    return new WebTransportTransport(config);
  }
  if (mode === 'webrtc') {
    return new WebRtcTransport(config);
  }
  if (mode === 'websocket') {
    return new WebSocketTransport(config as WebSocketConfig);
  }
  if (mode === 'fetch') {
    return new FetchTransport(config);
  }

  let url: URL;
  try {
    url = new URL(config.url);
  } catch {
    // Defer validation to start() so errors are reported via onError.
    return new FetchTransport(config);
  }
  if (url.protocol === 'ws:' || url.protocol === 'wss:') {
    return new WebSocketTransport(config as WebSocketConfig);
  }
  return new FetchTransport(config);
}
