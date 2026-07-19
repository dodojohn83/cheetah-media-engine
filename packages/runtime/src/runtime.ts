import {
  decodeEnvelope,
  encodeEnvelope,
  type BootstrapPayload,
  type Envelope,
  type EventPayload,
  type LoadPayload,
  type SeekPayload,
  type SetPlaybackRatePayload,
  type WorkerErrorPayload,
  type FrameStepPayload,
  type PauseDisplayPayload,
  PROTOCOL_VERSION,
} from './messages';
import { MemoryArenaView } from './memory';

export interface EngineRuntime {
  readonly version: string;
  readonly epoch: number;
  load(url: string, options?: { isLive?: boolean }): Promise<void>;
  play(): void;
  pause(): void;
  frameStep(direction: 'forward' | 'backward', keyframeOnly?: boolean): Promise<void>;
  pauseDisplay(keepConnection?: boolean): Promise<void>;
  seek(timeMs: number): Promise<void>;
  setPlaybackRate(rate: number): Promise<void>;
  stop(): Promise<void>;
  destroy(): Promise<void>;
  request(type: Envelope['type'], payload?: unknown, timeoutMs?: number): Promise<unknown>;
  onEvent?: ((event: string, details?: Record<string, unknown> | undefined) => void) | undefined;
  onError?: ((error: WorkerErrorPayload) => void) | undefined;
}

export interface RuntimeOptions {
  workerUrl?: string | undefined;
  wasmUrl?: string | undefined;
  maxPendingCommands?: number | undefined;
}

interface PendingCommand {
  resolve: (value: void | unknown) => void;
  reject: (reason: Error) => void;
  timer?: ReturnType<typeof setTimeout>;
}

/**
 * Create a runtime that runs the media engine inside a Web Worker.
 *
 * Commands are sent as JSON envelopes with a sequence number; replies are
 * matched by the same sequence. Stale replies (different epoch) are ignored.
 */
export function createRuntime(options: RuntimeOptions = {}): EngineRuntime {
  const { workerUrl, wasmUrl: _wasmUrl, maxPendingCommands = 64 } = options;
  if (!Number.isInteger(maxPendingCommands) || maxPendingCommands < 1) {
    throw new Error('maxPendingCommands must be a finite positive integer');
  }

  let worker: Worker | undefined;
  let instance = 0;
  let epoch = 0;
  let sequence = 0;
  let pending = new Map<number, PendingCommand>();
  let destroyed = false;
  let memoryView: MemoryArenaView | undefined;

  function ensureWorker(): Worker {
    if (destroyed) throw new Error('Runtime destroyed');
    if (worker) return worker;
    if (!workerUrl) throw new Error('No workerUrl provided');
    worker = new Worker(workerUrl, { type: 'module' });
    worker.onmessage = (event: MessageEvent<unknown>) => handleMessage(event.data);
    worker.onerror = (error: ErrorEvent) => {
      rejectAll(new Error(`Worker error: ${error.message}`));
    };
    worker.onmessageerror = () => {
      rejectAll(new Error('Worker message deserialization failed'));
    };
    return worker;
  }

  function handleMessage(data: unknown): void {
    if (typeof data !== 'string') return;
    let envelope: Envelope;
    try {
      envelope = decodeEnvelope(data);
    } catch {
      return;
    }
    if (envelope.instance !== instance || envelope.epoch !== epoch) {
      // Stale reply from a previous session; ignore.
      return;
    }
    if (envelope.type === 'event') {
      const payload = envelope.payload as EventPayload;
      runtime.onEvent?.(payload.event, payload.details);
      return;
    }
    if (envelope.type === 'error') {
      const payload = envelope.payload as WorkerErrorPayload;
      runtime.onError?.(payload);
      // Reject the matching command if any.
      const command = pending.get(envelope.sequence);
      if (command) {
        pending.delete(envelope.sequence);
        clearTimeout(command.timer);
        command.reject(new Error(`${payload.stage}:${payload.code} ${payload.message}`));
      }
      return;
    }
    if (envelope.type === 'memory-growth') {
      memoryView?.refresh();
      return;
    }
    const command = pending.get(envelope.sequence);
    if (command) {
      pending.delete(envelope.sequence);
      clearTimeout(command.timer);
      command.resolve(envelope.payload);
    }
  }

  function rejectAll(reason: Error): void {
    const copy = pending;
    pending = new Map();
    for (const command of copy.values()) {
      clearTimeout(command.timer);
      command.reject(reason);
    }
  }

  function post(type: Envelope['type'], payload?: unknown, timeoutMs = 10000): Promise<unknown> {
    if (destroyed) return Promise.reject(new Error('Runtime destroyed'));
    if (pending.size >= maxPendingCommands) {
      return Promise.reject(new Error('Too many pending commands'));
    }
    const w = ensureWorker();
    sequence += 1;
    const seq = sequence;
    const envelope: Envelope = {
      protocolVersion: PROTOCOL_VERSION,
      instance,
      epoch,
      sequence: seq,
      type,
      payload,
    };
    return new Promise<unknown>((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(seq);
        reject(new Error(`Command ${type} timed out`));
      }, timeoutMs);
      pending.set(seq, { resolve, reject, timer });
      w.postMessage(encodeEnvelope(envelope));
    });
  }

  function sendControl(type: Envelope['type']): void {
    if (destroyed) return;
    const w = ensureWorker();
    sequence += 1;
    const envelope: Envelope = {
      protocolVersion: PROTOCOL_VERSION,
      instance,
      epoch,
      sequence: sequence,
      type,
    };
    w.postMessage(encodeEnvelope(envelope));
  }

  const runtime: EngineRuntime = {
    version: '0.1.0',
    get epoch() {
      return epoch;
    },
    request: post,

    async load(url: string, options = {}): Promise<void> {
      if (destroyed) throw new Error('Runtime destroyed');
      instance += 1;
      epoch += 1;
      sequence = 0;
      rejectAll(new Error('New stream loaded'));
      pending = new Map();
      ensureWorker();
      if (_wasmUrl) {
        const bootstrap: BootstrapPayload = { wasmUrl: _wasmUrl };
        await post('bootstrap', bootstrap, 30000);
      }
      const payload: LoadPayload = { url, isLive: options.isLive ?? false };
      await post('load', payload, 30000);
    },

    play(): void {
      sendControl('play');
    },

    pause(): void {
      sendControl('pause');
    },

    async frameStep(direction: 'forward' | 'backward', keyframeOnly = false): Promise<void> {
      if (destroyed) throw new Error('Runtime destroyed');
      if (direction !== 'forward' && direction !== 'backward') {
        throw new Error('frameStep direction must be forward or backward');
      }
      const payload: FrameStepPayload = { direction, keyframeOnly };
      await post('frame-step', payload, 10000);
    },

    async pauseDisplay(keepConnection = true): Promise<void> {
      if (destroyed) throw new Error('Runtime destroyed');
      const payload: PauseDisplayPayload = { keepConnection };
      await post('pause-display', payload, 10000);
    },

    async seek(timeMs: number): Promise<void> {
      if (destroyed) throw new Error('Runtime destroyed');
      if (!Number.isFinite(timeMs) || timeMs < 0) {
        throw new Error('seek timeMs must be a finite non-negative number');
      }
      const payload: SeekPayload = { timeMs };
      await post('seek', payload, 10000);
    },

    async setPlaybackRate(rate: number): Promise<void> {
      if (destroyed) throw new Error('Runtime destroyed');
      if (!Number.isFinite(rate) || rate < 0.1 || rate > 16) {
        throw new Error('playback rate must be between 0.1 and 16');
      }
      const payload: SetPlaybackRatePayload = { rate };
      await post('set-playback-rate', payload, 10000);
    },

    async stop(): Promise<void> {
      if (destroyed) return;
      epoch += 1;
      rejectAll(new Error('Stream stopped'));
      await post('stop', undefined, 10000);
    },

    async destroy(): Promise<void> {
      if (destroyed) return;
      destroyed = true;
      rejectAll(new Error('Runtime destroyed'));
      if (worker) {
        worker.terminate();
        worker = undefined;
      }
    },
  };

  // Expose wasmUrl/workerUrl for worker bootstrap if needed.
  (runtime as unknown as Record<string, unknown>).__options = options;
  return runtime;
}
