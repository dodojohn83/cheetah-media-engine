import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { WebRtcTransport, TransportErrorCode } from './transport';

class MockRTCDataChannel {
  public label = 'media';
  public ordered = true;
  public readyState: 'connecting' | 'open' | 'closing' | 'closed' = 'connecting';
  public onopen: ((this: unknown, ev: Event) => void) | null = null;
  public onclose: ((this: unknown, ev: Event) => void) | null = null;
  public onerror: ((this: unknown, ev: Event) => void) | null = null;
  public onmessage: ((this: unknown, ev: { data: unknown }) => void) | null = null;
  public close = vi.fn(() => {
    this.readyState = 'closed';
  });

  public triggerOpen(): void {
    this.readyState = 'open';
    this.onopen?.call(this, new Event('open'));
  }

  public triggerClose(): void {
    this.readyState = 'closed';
    this.onclose?.call(this, new Event('close'));
  }

  public triggerError(): void {
    this.onerror?.call(this, new Event('error'));
  }

  public triggerMessage(data: ArrayBuffer): void {
    this.onmessage?.call(this, { data });
  }
}

class MockRTCPeerConnection {
  public connectionState: 'new' | 'connecting' | 'connected' | 'disconnected' | 'failed' | 'closed' = 'new';
  public onconnectionstatechange: ((this: unknown, ev: Event) => void) | null = null;
  public oniceconnectionstatechange: ((this: unknown, ev: Event) => void) | null = null;
  public channel: MockRTCDataChannel | undefined;
  public shouldOpenChannel = true;
  public createDataChannel = vi.fn((_label: string, _init?: { ordered?: boolean }) => {
    this.channel = new MockRTCDataChannel();
    return this.channel;
  });
  public createOffer = vi.fn(async () => ({ type: 'offer' as const, sdp: 'offer-sdp' }));
  public setLocalDescription = vi.fn(async () => { /* no-op */ });
  public setRemoteDescription = vi.fn(async () => {
    this.connectionState = 'connected';
    this.onconnectionstatechange?.call(this, new Event('connectionstatechange'));
    if (this.shouldOpenChannel) {
      // Open the data channel on the next microtask so listeners are attached.
      Promise.resolve().then(() => {
        this.channel?.triggerOpen();
      });
    }
  });
  public close = vi.fn(() => {
    this.connectionState = 'closed';
    this.onconnectionstatechange?.call(this, new Event('connectionstatechange'));
    this.channel?.triggerClose();
  });
}

describe('WebRtcTransport', () => {
  let originalRTCPeerConnection: unknown;
  let originalFetch: typeof fetch;
  let currentPc: MockRTCPeerConnection | undefined;

  beforeEach(() => {
    originalRTCPeerConnection = (globalThis as unknown as { RTCPeerConnection?: unknown }).RTCPeerConnection;
    originalFetch = globalThis.fetch;
    currentPc = undefined;
  });

  afterEach(() => {
    (globalThis as unknown as { RTCPeerConnection?: unknown }).RTCPeerConnection = originalRTCPeerConnection;
    globalThis.fetch = originalFetch;
  });

  function installMocks(options: { answer?: string; status?: number; openChannel?: boolean } = {}): MockRTCPeerConnection {
    const Ctor = function (this: unknown) {
      const pc = new MockRTCPeerConnection();
      pc.shouldOpenChannel = options.openChannel ?? true;
      currentPc = pc;
      return pc;
    } as unknown as typeof globalThis.RTCPeerConnection;

    (globalThis as unknown as { RTCPeerConnection?: unknown }).RTCPeerConnection = Ctor;

    globalThis.fetch = vi.fn(async () => {
      const status = options.status ?? 200;
      return {
        ok: status >= 200 && status < 300,
        status,
        statusText: status === 200 ? 'OK' : 'Not Found',
        text: async () => options.answer ?? 'answer-sdp',
      } as unknown as Response;
    });

    return currentPc!;
  }

  it('performs signaling, opens the data channel and delivers chunks', async () => {
    installMocks();
    const chunks: Uint8Array[] = [];
    const errors: { code: number }[] = [];
    let endCount = 0;

    const transport = new WebRtcTransport({ url: 'https://example.com/webrtc' });
    transport.start(
      (chunk) => chunks.push(chunk.bytes),
      (error) => errors.push(error),
      () => { endCount += 1; },
    );

    await vi.waitFor(() => {
      expect(currentPc).toBeDefined();
      expect(currentPc!.createDataChannel).toHaveBeenCalledWith('media', { ordered: true });
      expect(currentPc!.createOffer).toHaveBeenCalled();
      expect(currentPc!.setLocalDescription).toHaveBeenCalledWith({ type: 'offer', sdp: 'offer-sdp' });
      expect(globalThis.fetch).toHaveBeenCalledWith(
        'https://example.com/webrtc',
        expect.objectContaining({
          method: 'POST',
          headers: expect.objectContaining({ 'Content-Type': 'application/sdp' }),
          body: 'offer-sdp',
        }),
      );
      expect(currentPc!.setRemoteDescription).toHaveBeenCalledWith({ type: 'answer', sdp: 'answer-sdp' });
      expect(currentPc!.channel?.readyState).toBe('open');
    });

    const bytes = new Uint8Array([0x00, 0x00, 0x00, 0x01, 0x09, 0x10]);
    currentPc!.channel?.triggerMessage(bytes.buffer);

    await vi.waitFor(() => {
      expect(chunks).toHaveLength(1);
      expect(chunks[0]).toEqual(bytes);
    });

    transport.stop();

    await vi.waitFor(() => {
      expect(currentPc!.close).toHaveBeenCalled();
      expect(endCount).toBe(1);
      expect(errors).toHaveLength(0);
    });
  });

  it('reports an unsupported API when RTCPeerConnection is missing', async () => {
    (globalThis as unknown as { RTCPeerConnection?: unknown }).RTCPeerConnection = undefined;

    const transport = new WebRtcTransport({ url: 'https://example.com/webrtc' });
    const error = await new Promise<{ code: number }>((resolve) => {
      transport.start(
        () => { /* no-op */ },
        (err) => resolve(err),
        () => { /* no-op */ },
      );
    });

    expect(error.code).toBe(TransportErrorCode.WebRtcNotSupported);
  });

  it('reports an invalid URL', async () => {
    const transport = new WebRtcTransport({ url: 'ftp://example.com/webrtc' });
    const error = await new Promise<{ code: number }>((resolve) => {
      transport.start(
        () => { /* no-op */ },
        (err) => resolve(err),
        () => { /* no-op */ },
      );
    });

    expect(error.code).toBe(TransportErrorCode.InvalidUrl);
  });

  it('reports a signaling failure when the server returns an error', async () => {
    installMocks({ status: 404 });

    const transport = new WebRtcTransport({ url: 'https://example.com/webrtc' });
    const error = await new Promise<{ code: number }>((resolve) => {
      transport.start(
        () => { /* no-op */ },
        (err) => resolve(err),
        () => { /* no-op */ },
      );
    });

    expect(error.code).toBe(TransportErrorCode.WebRtcSignalingFailed);
  });

  it('reports a data channel error and ends', async () => {
    installMocks();
    const errors: { code: number }[] = [];
    let endCount = 0;

    const transport = new WebRtcTransport({ url: 'https://example.com/webrtc' });
    transport.start(
      () => { /* no-op */ },
      (error) => { errors.push(error); },
      () => { endCount += 1; },
    );

    await vi.waitFor(() => expect(currentPc!.channel?.readyState).toBe('open'));

    currentPc!.channel?.triggerError();

    await vi.waitFor(() => {
      expect(errors).toHaveLength(1);
      expect(errors[0]?.code).toBe(TransportErrorCode.WebRtcDataChannelFailed);
      expect(endCount).toBe(1);
    });
  });

  it('reports a peer connection failure mid-stream', async () => {
    installMocks();
    const errors: { code: number }[] = [];
    let endCount = 0;

    const transport = new WebRtcTransport({ url: 'https://example.com/webrtc' });
    transport.start(
      () => { /* no-op */ },
      (error) => { errors.push(error); },
      () => { endCount += 1; },
    );

    await vi.waitFor(() => expect(currentPc!.channel?.readyState).toBe('open'));

    currentPc!.connectionState = 'failed';
    currentPc!.onconnectionstatechange?.call(currentPc, new Event('connectionstatechange'));

    await vi.waitFor(() => {
      expect(errors).toHaveLength(1);
      expect(errors[0]?.code).toBe(TransportErrorCode.WebRtcConnectionFailed);
      expect(endCount).toBe(1);
    });
  });

  it('can be stopped before negotiation completes', async () => {
    installMocks({ openChannel: false });
    const errors: { code: number }[] = [];
    let endCount = 0;

    const transport = new WebRtcTransport({ url: 'https://example.com/webrtc' });
    transport.start(
      () => { /* no-op */ },
      (error) => { errors.push(error); },
      () => { endCount += 1; },
    );

    await vi.waitFor(() => expect(currentPc!.createOffer).toHaveBeenCalled());

    transport.stop();

    await vi.waitFor(() => {
      expect(currentPc!.close).toHaveBeenCalled();
      expect(endCount).toBe(1);
      expect(errors[0]?.code).toBe(TransportErrorCode.Canceled);
    });
  });
});
