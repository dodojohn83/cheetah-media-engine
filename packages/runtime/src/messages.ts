/**
 * Message envelope used between the main thread and the media worker.
 *
 * The envelope carries a protocol version, instance id, stream epoch,
 * monotonic sequence number and a typed payload so the worker can reject
 * stale commands and the main thread can correlate async replies.
 */

export const PROTOCOL_VERSION = 1;

export type MessageType =
  | 'capabilities'
  | 'load'
  | 'play'
  | 'pause'
  | 'stop'
  | 'destroy'
  | 'config'
  | 'packet'
  | 'output'
  | 'event'
  | 'error'
  | 'memory-growth'
  | 'metrics';

export interface Envelope {
  readonly protocolVersion: number;
  readonly instance: number;
  readonly epoch: number;
  readonly sequence: number;
  readonly type: MessageType;
  readonly payload?: unknown;
}

/**
 * Structured errors that can be sent across the worker boundary.
 */
export interface WorkerErrorPayload {
  readonly code: number;
  readonly stage: string;
  readonly message: string;
  readonly recoverable: boolean;
}

export interface CapabilityPayload {
  readonly capabilities: import('./capabilities').CapabilityReport;
}

export interface LoadPayload {
  readonly url: string;
  readonly isLive: boolean;
}

export interface PacketPayload {
  readonly slot: number;
  readonly generation: number;
  readonly trackIndex: number;
  readonly ptsMs: number;
  readonly dtsMs: number;
  readonly durationMs: number;
  readonly flags: number;
  readonly epoch: number;
}

export interface OutputPayload {
  readonly slot: number;
  readonly generation: number;
  readonly trackIndex: number;
  readonly width: number;
  readonly height: number;
  readonly ptsMs: number;
  readonly durationMs: number;
  readonly flags: number;
  readonly epoch: number;
}

export interface EventPayload {
  readonly event: string;
  readonly details?: Record<string, unknown> | undefined;
}

export interface MetricsPayload {
  readonly dropped: number;
  readonly queue: string;
  readonly level: number;
}

export function encodeEnvelope(envelope: Envelope): string {
  if (envelope.protocolVersion !== PROTOCOL_VERSION) {
    throw new Error(`Unsupported protocol version ${envelope.protocolVersion}`);
  }
  return JSON.stringify(envelope);
}

export function decodeEnvelope(data: string): Envelope {
  const parsed = JSON.parse(data) as Envelope;
  if (parsed.protocolVersion !== PROTOCOL_VERSION) {
    throw new Error(`Unsupported protocol version ${parsed.protocolVersion}`);
  }
  if (
    typeof parsed.instance !== 'number' ||
    typeof parsed.epoch !== 'number' ||
    typeof parsed.sequence !== 'number' ||
    typeof parsed.type !== 'string'
  ) {
    throw new Error('Malformed envelope');
  }
  return parsed;
}
