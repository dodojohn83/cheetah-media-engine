/**
 * Message envelope used between the main thread and the media worker.
 *
 * The envelope carries a protocol version, instance id, stream epoch,
 * monotonic sequence number and a typed payload so the worker can reject
 * stale commands and the main thread can correlate async replies.
 */

export const PROTOCOL_VERSION = 1;

export type MessageType =
  | 'bootstrap'
  | 'capabilities'
  | 'load'
  | 'play'
  | 'pause'
  | 'frame-step'
  | 'pause-display'
  | 'stop'
  | 'destroy'
  | 'config'
  | 'packet'
  | 'output'
  | 'seek'
  | 'set-playback-rate'
  | 'event'
  | 'error'
  | 'memory-growth'
  | 'metrics'
  | 'snapshot'
  | 'switch-variant'
  | 'start-recording'
  | 'stop-recording'
  | 'get-stats'
  | 'get-diagnostics';

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

export interface BootstrapPayload {
  readonly wasmUrl: string;
}

export interface CapabilityPayload {
  readonly capabilities: import('./capabilities').CapabilityReport;
}

export interface LoadPayload {
  readonly url: string;
  readonly isLive: boolean;
}

export interface SeekPayload {
  readonly timeMs: number;
}

export interface SetPlaybackRatePayload {
  readonly rate: number;
}

export interface FrameStepPayload {
  readonly direction: 'forward' | 'backward';
  readonly keyframeOnly?: boolean;
}

export interface PauseDisplayPayload {
  /** When true, keep the network/decoder alive while freezing the display. */
  readonly keepConnection: boolean;
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

export interface SnapshotPayload {
  readonly maxWidth?: number;
  readonly maxHeight?: number;
}

export interface SnapshotResultPayload {
  readonly width: number;
  readonly height: number;
  readonly data?: Uint8ClampedArray;
}

export interface SwitchVariantPayload {
  readonly bandwidth?: number;
  readonly index?: number;
}

export interface RecordingPayload {
  readonly mimeType?: string;
  readonly filename?: string;
}

export interface StatsPayload {
  readonly bufferedMs?: number;
  readonly decodedFrames?: number;
  readonly droppedFrames?: number;
  readonly networkBytes?: number;
  readonly latencyMs?: number;
}

export interface DiagnosticsPayload {
  readonly playerId: string;
  readonly version: string;
  readonly state: string;
  readonly epoch: number;
  readonly eventCount: number;
}

export function encodeEnvelope(envelope: Envelope): string {
  if (envelope.protocolVersion !== PROTOCOL_VERSION) {
    throw new Error(`Unsupported protocol version ${envelope.protocolVersion}`);
  }
  return JSON.stringify(envelope);
}

const VALID_MESSAGE_TYPES: ReadonlySet<string> = new Set([
  'bootstrap',
  'capabilities',
  'load',
  'play',
  'pause',
  'frame-step',
  'pause-display',
  'stop',
  'destroy',
  'config',
  'packet',
  'output',
  'seek',
  'set-playback-rate',
  'event',
  'error',
  'memory-growth',
  'metrics',
  'snapshot',
  'switch-variant',
  'start-recording',
  'stop-recording',
  'get-stats',
  'get-diagnostics',
]);

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 && Number.isInteger(value);
}

export function decodeEnvelope(data: string): Envelope {
  const parsed = JSON.parse(data) as Envelope;
  if (parsed.protocolVersion !== PROTOCOL_VERSION) {
    throw new Error(`Unsupported protocol version ${parsed.protocolVersion}`);
  }
  if (
    !isNonNegativeInteger(parsed.instance) ||
    !isNonNegativeInteger(parsed.epoch) ||
    !isNonNegativeInteger(parsed.sequence) ||
    typeof parsed.type !== 'string' ||
    !VALID_MESSAGE_TYPES.has(parsed.type)
  ) {
    throw new Error('Malformed envelope');
  }
  return parsed;
}
