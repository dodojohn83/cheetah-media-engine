/**
 * Shared types and utilities for the media byte-stream transport layer.
 *
 * These live in a separate module so that `transport.ts`, `webtransport.ts`
 * and other future transports can import them without circular dependencies.
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
  WebTransportNotSupported: 7010,
  WebTransportClosed: 7011,
} as const;

export function makeError(
  code: number,
  message: string,
  retryable: boolean,
): TransportError {
  return { code, stage: 'transport', message, retryable };
}

export function validateUrl(url: string): TransportError | undefined {
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

export function validateWebTransportUrl(url: string): TransportError | undefined {
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
  if (parsed.protocol !== 'https:') {
    return makeError(
      TransportErrorCode.InvalidUrl,
      `WebTransport requires https:// scheme, got ${parsed.protocol}`,
      false,
    );
  }
  if (
    typeof globalThis !== 'undefined' &&
    !(globalThis as unknown as { isSecureContext?: boolean }).isSecureContext
  ) {
    return makeError(
      TransportErrorCode.InsecureContent,
      'WebTransport is only available in a secure context',
      false,
    );
  }
  return undefined;
}

/** Common transport interface implemented by Fetch, WebSocket and WebTransport. */
export interface Transport {
  readonly config: TransportConfig;
  start(
    onChunk: (chunk: Chunk) => void,
    onError: (error: TransportError) => void,
    onEnd: () => void,
  ): void;
  stop(): void;
}
