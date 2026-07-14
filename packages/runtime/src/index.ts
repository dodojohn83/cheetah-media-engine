export const RUNTIME_VERSION = '0.1.0';

export { detectCapabilities, type CapabilityReport } from './capabilities';
export {
  decodeEnvelope,
  encodeEnvelope,
  PROTOCOL_VERSION,
  type Envelope,
  type CapabilityPayload,
  type EventPayload,
  type LoadPayload,
  type MessageType,
  type MetricsPayload,
  type OutputPayload,
  type PacketPayload,
  type WorkerErrorPayload,
} from './messages';
export { MemoryArenaView } from './memory';
export { createRuntime, type EngineRuntime, type RuntimeOptions } from './runtime';
