/**
 * Media worker entry point.
 *
 * The worker loads the wasm module, initializes the engine, and dispatches
 * commands received as envelopes from the main thread. It keeps no volatile
 * TypedArray views across await boundaries.
 */

import {
  decodeEnvelope,
  encodeEnvelope,
  type Envelope,
  type EventPayload,
  type LoadPayload,
  type PacketPayload,
  type WorkerErrorPayload,
} from './messages';

interface WorkerScope {
  postMessage(data: string): void;
  onmessage?: (event: MessageEvent<unknown>) => void;
  close(): void;
}

declare const self: WorkerScope;

let wasmInit: Promise<unknown> | undefined;
let currentEpoch = 0;

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

async function initWasm(): Promise<unknown> {
  if (wasmInit) return wasmInit;
  // The main runtime passes the wasm URL/import path via a dedicated
  // 'bootstrap' message before the first command.
  wasmInit = Promise.resolve();
  return wasmInit;
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
    case 'load': {
      const payload = envelope.payload as LoadPayload;
      currentEpoch = envelope.epoch;
      wasmInit ??= initWasm();
      wasmInit
        .then(() => {
          sendEvent(envelope.instance, 'loaded', { url: payload.url, isLive: payload.isLive });
          // Reply with the original command type so the main thread can resolve
          // the pending load promise by sequence number.
          reply({ ...envelope, payload: { event: 'loaded' } });
        })
        .catch((err) => sendError(envelope.instance, envelope.sequence, err));
      break;
    }
    case 'play':
    case 'pause':
      sendEvent(envelope.instance, envelope.type);
      break;
    case 'stop':
      currentEpoch = envelope.epoch;
      sendEvent(envelope.instance, 'stopped');
      // Reply with the original command type so the main thread can resolve
      // the pending stop promise.
      reply({ ...envelope, payload: { event: 'stopped' } });
      break;
    case 'destroy':
      currentEpoch = 0;
      sendEvent(envelope.instance, 'destroyed');
      break;
    case 'packet': {
      const payload = envelope.payload as PacketPayload;
      // Placeholder: real implementation would copy packet bytes into the arena
      // and call the Rust engine. Here we just acknowledge.
      reply({
        ...envelope,
        payload: { event: 'packet-accepted', slot: payload.slot },
      });
      break;
    }
    default:
      sendError(envelope.instance, envelope.sequence, `Unknown command type ${envelope.type}`);
  }
};

export {};
