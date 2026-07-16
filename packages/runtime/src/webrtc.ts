/**
 * WebRTC data-channel transport skeleton.
 *
 * This is a skeleton implementation that sets up an `RTCPeerConnection` and an
 * `RTCDataChannel`, performs a minimal SDP offer/answer exchange via the
 * configured URL, and forwards received binary messages to the caller.
 *
 * It does not implement the browser `ontrack` decoded-media path; that will be
 * wired up in a later player-integration work package.
 */

import {
  type Chunk,
  makeError,
  type Transport,
  TransportErrorCode,
  type TransportConfig,
  type TransportError,
  validateUrl,
} from './transport-common';

interface RTCSessionDescriptionInit {
  type: 'offer' | 'answer' | 'pranswer' | 'rollback';
  sdp: string;
}

interface RTCDataChannel {
  readonly label: string;
  readonly readyState: 'connecting' | 'open' | 'closing' | 'closed';
  binaryType: BinaryType;
  onopen: ((this: RTCDataChannel, ev: Event) => void) | null;
  onclose: ((this: RTCDataChannel, ev: Event) => void) | null;
  onerror: ((this: RTCDataChannel, ev: Event) => void) | null;
  onmessage: ((this: RTCDataChannel, ev: { data: unknown }) => void) | null;
  close(): void;
}

interface RTCPeerConnection {
  readonly connectionState: 'new' | 'connecting' | 'connected' | 'disconnected' | 'failed' | 'closed';
  readonly iceConnectionState: 'new' | 'checking' | 'connected' | 'completed' | 'disconnected' | 'failed' | 'closed';
  onconnectionstatechange: ((this: RTCPeerConnection, ev: Event) => void) | null;
  oniceconnectionstatechange: ((this: RTCPeerConnection, ev: Event) => void) | null;
  createDataChannel(label: string, init?: { ordered?: boolean }): RTCDataChannel;
  createOffer(): Promise<RTCSessionDescriptionInit>;
  setLocalDescription(description?: RTCSessionDescriptionInit): Promise<void>;
  setRemoteDescription(description: RTCSessionDescriptionInit): Promise<void>;
  close(): void;
}

interface RTCPeerConnectionConstructor {
  new (configuration?: { iceServers?: readonly unknown[] }): RTCPeerConnection;
  readonly prototype: RTCPeerConnection;
}

function getRTCPeerConnectionConstructor(): RTCPeerConnectionConstructor | undefined {
  const g = globalThis as unknown as { RTCPeerConnection?: RTCPeerConnectionConstructor };
  return g.RTCPeerConnection;
}

class WebRtcError extends Error {
  constructor(
    readonly code: number,
    message: string,
  ) {
    super(message);
  }
}

export class WebRtcTransport implements Transport {
  public readonly config: TransportConfig;

  private pc: RTCPeerConnection | undefined;
  private channel: RTCDataChannel | undefined;
  private started = false;
  private stopped = false;
  private ended = false;
  private bytesRead = 0;
  private maxBytes: number;
  private onError?: (error: TransportError) => void;
  private onEnd?: () => void;
  private closeController: AbortController | undefined;
  private channelOpenWaiter?: { resolve: () => void; reject: (err: Error) => void } | undefined;
  private channelCloseWaiter?: { closeWith: (error?: TransportError) => void } | undefined;

  constructor(config: TransportConfig) {
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

    const urlError = validateUrl(this.config.url);
    if (urlError) {
      this.finish(urlError);
      return;
    }

    const Ctor = getRTCPeerConnectionConstructor();
    if (!Ctor) {
      this.finish(makeError(TransportErrorCode.WebRtcNotSupported, 'RTCPeerConnection API is not available', false));
      return;
    }

    this.run(Ctor, onChunk).catch((err) => {
      const error = this.toError(err);
      try {
        this.pc?.close();
      } catch {
        // close() can throw if already closed; ignore.
      }
      this.channelOpenWaiter?.resolve();
      this.channelCloseWaiter?.closeWith();
      this.channelOpenWaiter = undefined;
      this.channelCloseWaiter = undefined;
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

  private async run(
    Ctor: RTCPeerConnectionConstructor,
    onChunk: (chunk: Chunk) => void,
  ): Promise<void> {
    this.closeController = new AbortController();
    this.pc = new Ctor();
    this.channel = this.pc.createDataChannel('media', { ordered: true });
    this.channel.binaryType = 'arraybuffer';

    const opened = this.waitForChannelOpen(this.pc, this.channel);
    this.attachMessageHandler(this.channel, onChunk);

    if (this.stopped) {
      throw new WebRtcError(TransportErrorCode.Canceled, 'Transport stopped');
    }

    const offer = await this.pc.createOffer();
    await this.pc.setLocalDescription(offer);

    if (this.stopped) {
      throw new WebRtcError(TransportErrorCode.Canceled, 'Transport stopped');
    }

    const answer = await this.signal(offer);
    await this.pc.setRemoteDescription(answer);

    await opened;

    if (this.stopped) {
      throw new WebRtcError(TransportErrorCode.Canceled, 'Transport stopped');
    }

    const closeError = await this.waitForChannelClose(this.pc, this.channel);

    this.pc.close();
    this.finish(closeError);
  }

  private attachMessageHandler(
    channel: RTCDataChannel,
    onChunk: (chunk: Chunk) => void,
  ): void {
    channel.onmessage = (event) => {
      if (this.stopped) return;
      const data = event.data as ArrayBuffer | Blob;
      if (data instanceof ArrayBuffer) {
        this.deliver(new Uint8Array(data), onChunk);
        return;
      }
      if (typeof Blob !== 'undefined' && data instanceof Blob) {
        data.arrayBuffer().then((buffer) => {
          if (!this.stopped && !this.ended) {
            this.deliver(new Uint8Array(buffer), onChunk);
          }
        }).catch(() => {
          // Blob read failure is treated as a non-fatal payload skip.
        });
      }
    };
    channel.onerror = () => {
      if (!this.stopped && !this.ended) {
        this.finish(makeError(TransportErrorCode.WebRtcDataChannelFailed, 'Data channel error', true));
        this.stop();
      }
    };
  }

  private waitForChannelOpen(
    pc: RTCPeerConnection,
    channel: RTCDataChannel,
  ): Promise<void> {
    return new Promise((resolve, reject) => {
      this.channelOpenWaiter = { resolve, reject };
      let settled = false;
      const done = (fn: () => void) => {
        if (settled) return;
        settled = true;
        this.channelOpenWaiter = undefined;
        fn();
      };

      channel.onopen = () => {
        done(() => resolve());
      };
      channel.onclose = () => {
        done(() => {
          // A close that follows pc.close() or stop() should resolve so the
          // caller can report the underlying error; reject only when the data
          // channel closes unexpectedly while the peer connection is still up.
          if (this.stopped || pc.connectionState === 'closed' || pc.connectionState === 'failed') {
            resolve();
          } else {
            reject(new WebRtcError(TransportErrorCode.WebRtcDataChannelFailed, 'Data channel closed before open'));
          }
        });
      };
      channel.onerror = () => {
        done(() => reject(new WebRtcError(TransportErrorCode.WebRtcDataChannelFailed, 'Data channel error')));
      };
      pc.onconnectionstatechange = () => {
        if (pc.connectionState === 'failed') {
          done(() => reject(new WebRtcError(TransportErrorCode.WebRtcConnectionFailed, 'Peer connection failed')));
        }
        if (pc.connectionState === 'closed') {
          // Always resolve on a clean close; the caller (run) will report any
          // underlying error from the operation that caused the close.
          done(() => resolve());
        }
      };
    });
  }

  private waitForChannelClose(pc: RTCPeerConnection, channel: RTCDataChannel): Promise<TransportError | undefined> {
    return new Promise((resolve) => {
      let settled = false;
      const closeWith = (error?: TransportError) => {
        if (settled) return;
        settled = true;
        this.channelCloseWaiter = undefined;
        channel.onclose = null;
        pc.onconnectionstatechange = null;
        resolve(error);
      };

      this.channelCloseWaiter = { closeWith };

      if (this.stopped) {
        closeWith();
        return;
      }
      if (pc.connectionState === 'failed') {
        closeWith(makeError(TransportErrorCode.WebRtcConnectionFailed, 'Peer connection failed', true));
        return;
      }
      if (channel.readyState === 'closed' || pc.connectionState === 'closed') {
        closeWith();
        return;
      }

      const maybeClose = () => {
        if (this.stopped) {
          closeWith();
          return;
        }
        if (pc.connectionState === 'failed') {
          closeWith(makeError(TransportErrorCode.WebRtcConnectionFailed, 'Peer connection failed', true));
          return;
        }
        if (channel.readyState === 'closed' || pc.connectionState === 'closed') {
          closeWith();
          return;
        }
      };

      channel.onclose = () => {
        maybeClose();
      };
      pc.onconnectionstatechange = () => {
        maybeClose();
      };
    });
  }

  private async signal(offer: RTCSessionDescriptionInit): Promise<RTCSessionDescriptionInit> {
    const method = this.config.method ?? 'POST';
    const headers: Record<string, string> = {
      ...(this.config.headers ?? {}),
      'Content-Type': 'application/sdp',
    };

    const response = await fetch(this.config.url, {
      method,
      headers,
      body: offer.sdp,
      signal: this.closeController?.signal ?? null,
    });

    if (!response.ok) {
      throw new WebRtcError(
        TransportErrorCode.WebRtcSignalingFailed,
        `Signaling server returned ${response.status} ${response.statusText}`,
      );
    }

    const sdp = await response.text();
    if (!sdp) {
      throw new WebRtcError(TransportErrorCode.WebRtcSignalingFailed, 'Signaling server returned empty SDP');
    }

    return { type: 'answer', sdp };
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
    if (this.stopped) {
      return makeError(TransportErrorCode.Canceled, 'Transport stopped', false);
    }
    if (err instanceof DOMException && err.name === 'AbortError') {
      return makeError(TransportErrorCode.Canceled, 'Transport stopped', false);
    }
    if (err instanceof WebRtcError) {
      return makeError(err.code, err.message, err.code !== TransportErrorCode.WebRtcNotSupported);
    }
    const message = err instanceof Error ? err.message : String(err);
    return makeError(TransportErrorCode.WebRtcConnectionFailed, message, true);
  }

  stop(): void {
    this.stopped = true;
    this.closeController?.abort();
    try {
      this.pc?.close();
    } catch {
      // close() can throw if already closed; ignore.
    }
    // Real browsers do not always fire connectionstatechange/onclose when
    // RTCPeerConnection.close() is called locally, so resolve any pending
    // waiters directly to guarantee onEnd is emitted.
    this.channelOpenWaiter?.resolve();
    this.channelCloseWaiter?.closeWith();
    this.channelOpenWaiter = undefined;
    this.channelCloseWaiter = undefined;
  }
}
