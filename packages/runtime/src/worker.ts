/**
 * Media worker entry point.
 *
 * The worker dynamically imports the wasm-bindgen module, initializes the
 * engine, and dispatches commands received as envelopes from the main thread.
 * It keeps no volatile TypedArray views across await boundaries.
 */

import {
  decodeEnvelope,
  encodeEnvelope,
  type Envelope,
  type EventPayload,
  type LoadPayload,
  type PacketPayload,
  type BootstrapPayload,
  type WorkerErrorPayload,
} from './messages.js';

interface WorkerScope {
  postMessage(data: string): void;
  onmessage?: (event: MessageEvent<unknown>) => void;
  close(): void;
}

declare const self: WorkerScope;

interface WasmModule {
  default: (module_or_path?: string | URL | WebAssembly.Module | Response | Request) => Promise<unknown>;
  WebEngine: new () => WebEngineInstance;
}

interface WebEngineInstance {
  version: string;
  load(url: string, isLive: boolean): void;
  play(): void;
  pause(): void;
  stop(): void;
  destroy(): void;
  readonly is_playing: boolean;
  request_write_region(size: number): unknown;
  commit_region(slot: number, generation: bigint, len: number): void;
  release_region(slot: number, generation: bigint): void;
  push_packet(
    slot: number,
    generation: bigint,
    trackId: number,
    ptsMs: bigint,
    dtsMs: bigint,
    durationMs: bigint,
    flags: number,
  ): void;
  poll_output(): unknown | null;
  configure(json: string): void;
}

let wasmInit: Promise<WasmModule> | undefined;
let engine: WebEngineInstance | undefined;
let currentEpoch = 0;
let wasmUrl: string | undefined;

function reply(envelope: Envelope): void {
  self.postMessage(encodeEnvelope(envelope));
}

function sendError(instance: number, sequence: number, error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  const payload: WorkerErrorPayload = {
    code: 6999,
    stage: 'worker',
    message,
    recoverable: false,
  };
  reply({
    protocolVersion: 1,
    instance,
    epoch: currentEpoch,
    sequence,
    type: 'error',
    payload,
  });
}

function sendEvent(instance: number, event: string, details?: Record<string, unknown>): void {
  const payload: EventPayload = details ? { event, details } : { event };
  reply({
    protocolVersion: 1,
    instance,
    epoch: currentEpoch,
    sequence: 0,
    type: 'event',
    payload,
  });
}

async function initWasm(): Promise<WasmModule> {
  if (wasmInit) return wasmInit;
  if (!wasmUrl) {
    throw new Error('No wasmUrl provided; send bootstrap before load');
  }
  wasmInit = (async (): Promise<WasmModule> => {
    try {
      const mod = (await import(/* @vite-ignore */ wasmUrl!)) as WasmModule;
      await mod.default();
      return mod;
    } catch (err) {
      wasmInit = undefined;
      throw err;
    }
  })();
  return wasmInit;
}

function ensureEngine(): WebEngineInstance {
  if (!engine) {
    throw new Error('Wasm module not bootstrapped');
  }
  return engine;
}

function withEngine<T>(action: (engine: WebEngineInstance) => T, instance: number, sequence: number): void {
  try {
    action(ensureEngine());
  } catch (err) {
    sendError(instance, sequence, err);
  }
}

self.onmessage = (event: MessageEvent<unknown>) => {
  if (typeof event.data !== 'string') return;
  let envelope: Envelope;
  try {
    envelope = decodeEnvelope(event.data);
  } catch (err) {
    sendError(0, 0, err);
    return;
  }

  if (envelope.protocolVersion !== 1) {
    sendError(envelope.instance, envelope.sequence, 'Unsupported protocol version');
    return;
  }

  // Ignore stale commands from a previous epoch/session.
  if (envelope.epoch < currentEpoch && envelope.type !== 'load') {
    return;
  }

  switch (envelope.type) {
    case 'bootstrap': {
      const payload = envelope.payload as BootstrapPayload;
      wasmUrl = payload.wasmUrl;
      reply({ ...envelope, payload: { event: 'bootstrapped' } });
      break;
    }
    case 'load': {
      const payload = envelope.payload as LoadPayload;
      currentEpoch = envelope.epoch;
      const instance = envelope.instance;
      const sequence = envelope.sequence;
      initWasm()
        .then((mod) => {
          if (!engine) {
            engine = new mod.WebEngine();
          }
          engine.load(payload.url, payload.isLive);
          sendEvent(instance, 'loaded', { url: payload.url, isLive: payload.isLive });
          reply({ ...envelope, payload: { event: 'loaded' } });
        })
        .catch((err) => sendError(instance, sequence, err));
      break;
    }
    case 'play':
      withEngine((e) => {
        e.play();
        sendEvent(envelope.instance, 'playing');
      }, envelope.instance, envelope.sequence);
      break;
    case 'pause':
      withEngine((e) => {
        e.pause();
        sendEvent(envelope.instance, 'paused');
      }, envelope.instance, envelope.sequence);
      break;
    case 'stop':
      currentEpoch = envelope.epoch;
      withEngine((e) => {
        e.stop();
        e.destroy();
        engine = undefined;
        sendEvent(envelope.instance, 'stopped');
        reply({ ...envelope, payload: { event: 'stopped' } });
      }, envelope.instance, envelope.sequence);
      break;
    case 'destroy':
      currentEpoch = 0;
      if (engine) {
        try {
          engine.destroy();
        } catch (err) {
          // ignored during teardown
        }
        engine = undefined;
      }
      wasmInit = undefined;
      sendEvent(envelope.instance, 'destroyed');
      break;
    case 'config': {
      withEngine((e) => {
        const json = typeof envelope.payload === 'string' ? envelope.payload : JSON.stringify(envelope.payload);
        e.configure(json);
        sendEvent(envelope.instance, 'configured');
        reply({ ...envelope, payload: { event: 'configured' } });
      }, envelope.instance, envelope.sequence);
      break;
    }
    case 'packet': {
      const payload = envelope.payload as PacketPayload;
      withEngine((e) => {
        e.push_packet(
          payload.slot,
          BigInt(payload.generation),
          payload.trackIndex,
          BigInt(payload.ptsMs),
          BigInt(payload.dtsMs),
          BigInt(payload.durationMs),
          payload.flags,
        );
        reply({ ...envelope, payload: { event: 'packet-accepted', slot: payload.slot } });
      }, envelope.instance, envelope.sequence);
      break;
    }
    case 'output':
      withEngine((e) => {
        const desc = e.poll_output() as { slot?: number; generation?: bigint } | null;
        reply({ ...envelope, payload: desc ?? null });
      }, envelope.instance, envelope.sequence);
      break;
    default:
      sendError(envelope.instance, envelope.sequence, `Unknown command type ${envelope.type}`);
  }
};

export {};
