export const RUNTIME_VERSION = '0.1.0';

export {
  detectCapabilities,
  type CapabilityReport,
  type ProbedCapabilityReport,
  type ProbeDetails,
  CapabilityCache,
  probeCapabilities,
  computeFingerprint,
} from './capabilities';
export {
  plan,
  explain,
  type Backend,
  type LatencyTarget,
  type PlanCandidate,
  type PlanRequest,
  type PlaybackPlan,
  type Protocol,
  type Renderer,
  type TrackProfile,
  type TransportMode,
} from './planner';
export {
  FallbackController,
  type MediaBackend,
  type BackendContext,
  type MediaBackendFactory,
  type FallbackEvent,
  type FallbackOptions,
  type FallbackState,
} from './fallback';
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
export {
  createTransport,
  FetchTransport,
  WebSocketTransport,
  TransportErrorCode,
  type Transport,
  type TransportConfig,
  type TransportError,
  type TransportStats,
  type Chunk,
} from './transport';
export {
  WebCodecsBackend,
  webcodecsBackendFactory,
  type CloseableVideoFrame,
  type CloseableAudioData,
  type WebCodecsCallbacks,
  type WebCodecsBackendOptions,
  type WebCodecsMetrics,
} from './webcodecs';
export {
  MseBackend,
  mseBackendFactory,
  MseError,
  type MseErrorCode,
  type MseCallbacks,
  type MseBackendOptions,
  type MseMetrics,
  type HTMLVideoElementLike,
  type TimeRangesLike,
  type SourceBufferLike,
  type MediaSourceLike,
} from './mse';
