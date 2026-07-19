/**
 * Generic byte-stream downloader with pause/resume, progress and a pluggable sink.
 *
 * The downloader is transport-agnostic for HTTP(S): it uses `fetch` directly so
 * it can resume from the last received byte using a `Range` request. WebSocket
 * downloads are not supported; callers that need them should convert the stream
 * to a `ReadableStream` and use `pipeTo`.
 */

import { validateUrl, type TransportError, TransportErrorCode, makeError } from '../transport-common';

export interface DownloadProgress {
  readonly bytesWritten: number;
  readonly startedAt: number;
  readonly state: DownloadState;
}

export type DownloadState = 'idle' | 'running' | 'paused' | 'completed' | 'error';

export interface DownloadSink {
  write(chunk: Uint8Array): Promise<void> | void;
  close(): Promise<void> | void;
}

export interface DownloadOptions {
  readonly url: string;
  readonly headers?: Record<string, string>;
  readonly credentials?: RequestCredentials;
  readonly method?: 'GET' | 'POST';
  readonly body?: BodyInit | null;
  readonly sink: DownloadSink;
  readonly transform?: ((chunk: Uint8Array) => Uint8Array | undefined) | undefined;
  readonly onProgress?: (progress: DownloadProgress) => void;
  readonly onError?: (error: TransportError) => void;
  readonly onComplete?: () => void;
  /** Per-request timeout in milliseconds (default 30s). */
  readonly timeoutMs?: number;
}

export interface DownloadResult {
  readonly bytesWritten: number;
  readonly durationMs: number;
}

export class StreamDownloader {
  private state: DownloadState = 'idle';
  private controller?: AbortController;
  private bytesWritten = 0;
  private bytesReceived = 0;
  private startedAt = 0;
  private completedAt = 0;
  private currentSink: DownloadSink | undefined;

  get progress(): DownloadProgress {
    return {
      bytesWritten: this.bytesWritten,
      startedAt: this.startedAt,
      state: this.state,
    };
  }

  async start(options: DownloadOptions): Promise<DownloadResult> {
    const urlError = validateUrl(options.url);
    if (urlError) {
      this.report(options, urlError);
      throw urlError;
    }
    if (this.state === 'running') {
      const err = makeError(TransportErrorCode.Canceled, 'Download already running', false);
      this.report(options, err);
      throw err;
    }

    this.state = 'running';
    this.bytesWritten = 0;
    this.bytesReceived = 0;
    this.startedAt = performance.now();
    this.completedAt = 0;
    this.currentSink = options.sink;

    try {
      await this.run(options, false);
      this.completedAt = performance.now();
      this.state = 'completed';
      options.onComplete?.();
      return { bytesWritten: this.bytesWritten, durationMs: this.completedAt - this.startedAt };
    } catch (err) {
      const error = this.toError(err);
      const state = this.state as DownloadState;
      if (state === 'paused') {
        this.completedAt = performance.now();
        return { bytesWritten: this.bytesWritten, durationMs: this.completedAt - this.startedAt };
      }
      if (state !== 'idle') {
        this.state = 'error';
        this.report(options, error);
      }
      throw error;
    } finally {
      const finalState = this.state as DownloadState;
      if (finalState !== 'paused') {
        await this.closeSink(this.currentSink);
        this.currentSink = undefined;
      }
    }
  }

  pause(): void {
    if (this.state !== 'running') return;
    this.state = 'paused';
    this.controller?.abort();
  }

  async resume(options: DownloadOptions): Promise<DownloadResult> {
    if (this.state !== 'paused') {
      const err = makeError(TransportErrorCode.Canceled, 'Download is not paused', false);
      this.report(options, err);
      throw err;
    }

    this.state = 'running';
    this.currentSink = options.sink;
    try {
      await this.run(options, true);
      this.completedAt = performance.now();
      this.state = 'completed';
      options.onComplete?.();
      return { bytesWritten: this.bytesWritten, durationMs: this.completedAt - this.startedAt };
    } catch (err) {
      const error = this.toError(err);
      const state = this.state as DownloadState;
      if (state === 'paused') {
        this.completedAt = performance.now();
        return { bytesWritten: this.bytesWritten, durationMs: this.completedAt - this.startedAt };
      }
      if (state !== 'idle') {
        this.state = 'error';
        this.report(options, error);
      }
      throw error;
    } finally {
      const finalState = this.state as DownloadState;
      if (finalState !== 'paused') {
        await this.closeSink(this.currentSink);
        this.currentSink = undefined;
      }
    }
  }

  async stop(): Promise<void> {
    if (this.state === 'paused') {
      this.state = 'idle';
      await this.closeSink(this.currentSink);
      this.currentSink = undefined;
      return;
    }
    if (this.state === 'running') {
      this.state = 'idle';
      this.controller?.abort();
      // The pending start()/resume() finally will close the sink.
      return;
    }
  }

  private async run(options: DownloadOptions, isResume: boolean): Promise<void> {
    this.controller = new AbortController();
    const signal = this.controller.signal;

    let timer: ReturnType<typeof setTimeout> | undefined;
    const timeoutMs = options.timeoutMs ?? 30000;
    if (timeoutMs > 0 && timeoutMs !== Infinity) {
      timer = setTimeout(() => {
        this.controller?.abort();
      }, timeoutMs);
    }

    const headers = new Headers(options.headers ?? {});
    if (isResume && this.bytesReceived > 0) {
      headers.set('Range', `bytes=${this.bytesReceived}-`);
    }

    const init: RequestInit = {
      method: options.method ?? 'GET',
      headers,
      credentials: options.credentials ?? 'same-origin',
      signal,
    };
    if (options.body !== undefined && options.body !== null) {
      init.body = options.body;
    }
    try {
      const response = await fetch(options.url, init);

      if (!response.ok) {
        throw makeError(
          TransportErrorCode.HttpStatus,
          `HTTP ${response.status} ${response.statusText}`,
          response.status >= 500 || response.status === 429,
        );
      }

      if (isResume && this.bytesReceived > 0 && response.status !== 206) {
        throw makeError(TransportErrorCode.HttpStatus, 'Server does not support byte-range resume', false);
      }

      const reader = response.body?.getReader();
      if (!reader) {
        throw makeError(TransportErrorCode.Network, 'Response body is not readable', false);
      }

      try {
        while (true) {
          if (this.state !== 'running') {
            throw makeError(TransportErrorCode.Canceled, 'Download canceled', false);
          }
          const { done, value } = await reader.read();
          if (this.state !== 'running') {
            throw makeError(TransportErrorCode.Canceled, 'Download canceled', false);
          }
          if (done || !value) break;
          this.bytesReceived += value.length;
          const chunk = options.transform ? options.transform(value) : value;
          if (chunk) {
            await options.sink.write(chunk);
            this.bytesWritten += chunk.length;
          }
          options.onProgress?.(this.progress);
        }
      } finally {
        reader.releaseLock();
      }
    } finally {
      if (timer) clearTimeout(timer);
    }
  }

  private async closeSink(sink: DownloadSink | undefined): Promise<void> {
    if (!sink) return;
    try {
      await Promise.resolve(sink.close()).catch(() => undefined);
    } catch {
      // User close handler must not break the downloader lifecycle.
    }
  }

  private report(options: DownloadOptions, error: TransportError): void {
    try {
      options.onError?.(error);
    } catch {
      // User error handler must not break the downloader.
    }
  }

  private toError(err: unknown): TransportError {
    if (err && typeof err === 'object' && 'code' in err && 'stage' in err) {
      return err as TransportError;
    }
    const message = err instanceof Error ? err.message : String(err);
    if (this.isAbortError(err, message)) {
      return makeError(TransportErrorCode.Canceled, 'Download canceled', false);
    }
    return makeError(TransportErrorCode.Network, message, true);
  }

  private isAbortError(err: unknown, message: string): boolean {
    if (err instanceof DOMException && err.name === 'AbortError') return true;
    if (err instanceof Error && err.name === 'AbortError') return true;
    const lower = message.toLowerCase();
    return lower.includes('aborted') || lower.includes('abort');
  }
}

export class BlobSink implements DownloadSink {
  private chunks: Uint8Array[] = [];

  write(chunk: Uint8Array): void {
    this.chunks.push(chunk);
  }

  close(): void {
    // no-op
  }

  getBlob(): Blob {
    return new Blob(this.chunks as unknown as BlobPart[]);
  }
}
