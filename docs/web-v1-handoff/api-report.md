# Cheetah Media Engine Web v1 API Report

Generated: 2026-07-15T14:41:03.838Z
Commit: unknown

## Rust Crate Public API

Top-level public declarations from each crate's `src/lib.rs`.

### cheetah-container-flv

- `pub mod amf;`
- `pub mod audio;`
- `pub mod demuxer;`
- `pub mod error;`
- `pub mod header;`
- `pub mod muxer;`
- `pub mod recorder;`
- `pub mod video;`
- `pub use amf::{AmfLimits, AmfValue, FlvScriptData, parse_script_data};`
- `pub use audio::{AudioTagHeader, SoundFormat};`
- `pub use demuxer::{FlvDemuxer, FlvEvent, FlvMode};`
- `pub use error::FlvError;`
- `pub use header::{FlvHeader, FlvTagHeader, TagType};`
- `pub use muxer::FlvMuxer;`
- `pub use recorder::{FlvRecorder, FlvRecordingCancel, FlvWriter};`
- `pub use video::{FrameType, VideoCodecId, VideoTagHeader};`

### cheetah-container-isobmff

- `pub mod boxes;`
- `pub mod demuxer;`
- `pub mod error;`
- `pub mod fragment;`
- `pub mod moov;`
- `pub mod muxer;`
- `pub mod recorder;`
- `pub mod sample_entry;`
- `pub use demuxer::{IsobmffDemuxer, Mp4Event};`
- `pub use error::Mp4Error;`
- `pub use moov::TrackData;`
- `pub use muxer::{FragmentedMp4Muxer, SegmentOutput, TrackConfig};`
- `pub use recorder::Mp4Muxer as ProgressiveMp4Muxer;`

### cheetah-container-mpegts

- `pub mod clock;`
- `pub mod demuxer;`
- `pub mod error;`
- `pub mod packet;`
- `pub mod pes;`
- `pub mod section;`
- `pub use clock::{ClockState, PcrClock};`
- `pub use demuxer::{TsDemuxer, TsDiagnostics, TsEvent};`
- `pub use error::TsError;`
- `pub use packet::TsPacket;`
- `pub use pes::{PesAssembler, PesHeader, PesOutput};`
- `pub use section::{PatEntry, SectionAssembler, parse_pat, parse_pmt};`

### cheetah-hls-client

- `pub mod client;`
- `pub mod error;`
- `pub mod model;`
- `pub mod parser;`
- `pub mod variant;`
- `pub use client::{HlsAction, HlsClient, HlsEvent};`
- `pub use error::HlsError;`
- `pub use model::*;`
- `pub use parser::{parse, parse_master, parse_media};`
- `pub use variant::{VariantCapabilities, VariantSelector, select_initial_variant};`

### cheetah-media-abi

- `pub mod arena;`
- `pub mod descriptor;`
- `pub mod error;`
- `pub mod handle;`
- `pub mod version;`
- `pub use arena::MemoryArena;`
- `pub use descriptor::{FrameDescriptor, MemoryDescriptor, PacketDescriptor};`
- `pub use error::AbiError;`
- `pub use handle::Handle;`
- `pub use version::AbiVersion;`
- `pub struct Input<'a> {`
- `pub struct Output<'a> {`
- `pub trait DecoderProbe {`
- `pub trait Decoder: DecoderProbe {`
- `pub trait Renderer {`
- `pub trait AudioSink {`
- `pub trait Clock {`
- `pub trait ByteSource {`
- `pub trait Demuxer {`
- `pub trait Recorder {`
- `pub trait MetricsSink {`

### cheetah-media-backend-api

- `pub use port::*;`
- `pub mod port;`
- `pub trait CapabilityProbe {`
- `pub trait TransportSource: Send {`
- `pub enum TransportError {`

### cheetah-media-bitstream

- `pub mod aac;`
- `pub mod bit;`
- `pub mod g711;`
- `pub mod h264;`
- `pub mod h265;`
- `pub mod mp3;`
- `pub use aac::{AdtsHeader, AudioSpecificConfig};`
- `pub use bit::{BitCursor, BitError};`
- `pub use g711::{G711Kind, PcmFormat};`
- `pub use h264::{H264CodecConfig, H264Error, NalUnit as H264NalUnit};`
- `pub use h265::{`
- `pub use mp3::{Mp3Error, Mp3Header};`
- `pub enum ReadError {`
- `pub struct ByteCursor<'a> {`
- `pub const fn new(buf: &'a [u8]) -> Self {`
- `pub fn remaining(&self) -> usize {`
- `pub fn is_empty(&self) -> bool {`
- `pub fn read_u8(&mut self) -> Result<u8, ReadError> {`
- `pub fn read_u16_be(&mut self) -> Result<u16, ReadError> {`
- `pub fn read_u24_be(&mut self) -> Result<u32, ReadError> {`
- `pub fn read_u32_be(&mut self) -> Result<u32, ReadError> {`
- `pub fn peek_bytes(&mut self, n: usize) -> Result<&'a [u8], ReadError> {`
- `pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], ReadError> {`
- `pub fn skip(&mut self, n: usize) -> Result<(), ReadError> {`

### cheetah-media-core

- `pub use cheetah_container_flv as flv;`
- `pub use cheetah_container_isobmff as isobmff;`
- `pub use cheetah_container_mpegts as mpegts;`
- `pub use cheetah_hls_client as hls;`
- `pub use cheetah_media_abi as abi;`
- `pub use cheetah_media_bitstream as bitstream;`
- `pub use cheetah_media_pipeline_core as pipeline;`
- `pub use cheetah_media_timeline as timeline;`
- `pub use cheetah_media_types as types;`
- `pub const VERSION: &str = env!("CARGO_PKG_VERSION");`

### cheetah-media-engine

- `pub mod latency;`
- `pub mod metrics;`
- `pub mod recovery;`
- `pub mod resource;`
- `pub mod scheduler;`
- `pub mod state;`
- `pub use latency::{LatencyAction, LatencyBreakdown, LatencyController, LatencyTarget};`
- `pub use metrics::{AllocationMetric, CopyMetric, Metrics, MetricsSnapshot};`
- `pub use recovery::{`
- `pub use resource::{ResourceGuard, ResourceKind, ResourceLedger};`
- `pub use scheduler::{BoundedQueue, Priority, QueueConfig, QueueName, Scheduler, SchedulerEvent};`
- `pub use state::{`
- `pub const VERSION: &str = env!("CARGO_PKG_VERSION");`
- `pub struct PlayerBudget {`
- `pub const fn desktop() -> Self {`
- `pub fn select_backend<'a>( codec: CodecId, probes: &'a [&'a dyn CapabilityProbe], ) -> Option<&'a dyn CapabilityProbe> {`

### cheetah-media-pipeline-core

- `pub mod planner;`
- `pub struct Pipeline {`
- `pub const fn new() -> Self {`
- `pub fn set_decoder(&mut self, decoder: Box<dyn Decoder>) {`
- `pub fn set_renderer(&mut self, renderer: Box<dyn Renderer>) {`
- `pub fn feed<'a>(&mut self, input: &Input<'a>) -> Result<Output<'a>, AbiError> {`
- `pub fn flush(&mut self) -> Result<(), AbiError> {`

### cheetah-media-testkit

- `pub mod compare;`
- `pub mod store;`
- `pub mod validate;`
- `pub use compare::{`
- `pub use store::{FixtureStatus, FixtureStore};`
- `pub use validate::{FixtureError, validate_manifest};`
- `pub struct Fixture {`
- `pub fn h264_video(id: &'static str, duration_ms: u64) -> Self {`
- `pub fn aac_audio(id: &'static str, duration_ms: u64) -> Self {`
- `pub fn g711a_audio(id: &'static str, duration_ms: u64) -> Self {`
- `pub fn g711u_audio(id: &'static str, duration_ms: u64) -> Self {`
- `pub fn timestamp_sequence(start: i64, count: usize, step: i64) -> impl Iterator<Item = MediaTime> {`
- `pub enum SourceType {`
- `pub struct FixtureSource {`
- `pub struct FixtureManifestEntry {`
- `pub struct FixtureManifest {`
- `pub fn load_manifest(json: &str) -> Result<FixtureManifest, serde_json::Error> {`
- `pub fn workspace_manifest() -> Result<FixtureManifest, serde_json::Error> {`
- `pub fn find_fixture_by_id<'a>( manifest: &'a FixtureManifest, id: &str, ) -> Option<&'a FixtureManifestEntry> {`

### cheetah-media-timeline

- `pub mod clock;`
- `pub mod gop;`
- `pub mod sync;`
- `pub struct Timeline {`
- `pub const fn new() -> Self {`
- `pub fn push(&mut self, time: MediaTime) {`
- `pub fn len(&self) -> usize {`
- `pub fn is_empty(&self) -> bool {`
- `pub fn iter(&self) -> impl Iterator<Item = &MediaTime> {`
- `pub fn next_after_ms(&self, pts_ms: i64) -> Option<&MediaTime> {`

### cheetah-media-types

- `pub mod buffer;`
- `pub mod error;`
- `pub mod format;`
- `pub mod frame;`
- `pub mod limits;`
- `pub mod packet;`
- `pub mod time;`
- `pub mod track;`
- `pub use buffer::{`
- `pub use error::{MediaError, Recoverability};`
- `pub use format::{AudioFormat, ChannelLayout, ColorSpace, PixelFormat, SampleFormat, VideoFormat};`
- `pub use frame::{AudioFrame, ExternalFrameHandle, VideoFrame};`
- `pub use limits::MediaLimits;`
- `pub use packet::{MediaPacket, PacketFlags};`
- `pub use time::{MediaDuration, MediaTime, TimeBase, Timestamp};`
- `pub use track::{CodecConfig, SequenceNumber, StreamEpoch, TrackId, TrackInfo};`
- `pub enum CodecId {`
- `pub enum TrackKind {`

### cheetah-media-web-bindings

- `pub fn start() {`
- `pub fn engine_version() -> String {`
- `pub fn codec_name(codec_index: u8) -> String {`
- `pub struct MemoryDescriptor {`
- `pub fn slot(&self) -> u32 {`
- `pub fn generation(&self) -> u64 {`
- `pub fn offset(&self) -> u32 {`
- `pub fn length(&self) -> u32 {`
- `pub fn capacity(&self) -> u32 {`
- `pub fn flags(&self) -> u32 {`
- `pub struct WebEngine {`
- `pub fn new() -> Self {`
- `pub fn js_version(&self) -> String {`
- `pub fn configure(&mut self, _json: &str) -> Result<(), JsValue> {`
- `pub fn load(&mut self, url: &str, is_live: bool) -> Result<(), JsValue> {`
- `pub fn play(&mut self) -> Result<(), JsValue> {`
- `pub fn pause(&mut self) -> Result<(), JsValue> {`
- `pub fn is_playing(&self) -> bool {`
- `pub fn request_write_region(&mut self, size: u32) -> Result<MemoryDescriptor, JsValue> {`
- `pub fn commit_region(&mut self, slot: u32, generation: u64, len: u32) -> Result<(), JsValue> {`
- `pub fn release_region(&mut self, slot: u32, generation: u64) -> Result<(), JsValue> {`
- `pub fn push_packet( &mut self, slot: u32, generation: u64, _track_id: u32, _pts_ms: i64, _dts_ms: i64, _duration_ms: i64, _flags: u32, ) -> Result<(), JsValue> {`
- `pub fn poll_output(&mut self) -> Result<Option<MemoryDescriptor>, JsValue> {`
- `pub fn stop(&mut self) -> Result<(), JsValue> {`
- `pub fn destroy(self) {`

## TypeScript Public API

Exports from `packages/*/src/index.ts`.

### @cheetah-media/runtime

- `from ./capabilities: detectCapabilities, type CapabilityReport, type ProbedCapabilityReport, type ProbeDetails, CapabilityCache, probeCapabilities, computeFingerprint`
- `from ./planner: plan, explain, type Backend, type LatencyTarget, type PlanCandidate, type PlanRequest, type PlaybackPlan, type Protocol, type Renderer, type TrackProfile, type TransportMode`
- `from ./fallback: FallbackController, type MediaBackend, type BackendContext, type MediaBackendFactory, type FallbackEvent, type FallbackOptions, type FallbackState`
- `from ./messages: decodeEnvelope, encodeEnvelope, PROTOCOL_VERSION, type Envelope, type CapabilityPayload, type EventPayload, type LoadPayload, type MessageType, type MetricsPayload, type OutputPayload, type PacketPayload, type WorkerErrorPayload`
- `from ./memory: MemoryArenaView`
- `from ./runtime: createRuntime, type EngineRuntime, type RuntimeOptions`
- `from ./transport: createTransport, FetchTransport, WebSocketTransport, TransportErrorCode, type Transport, type TransportConfig, type TransportError, type TransportStats, type Chunk`
- `from ./webcodecs: WebCodecsBackend, webcodecsBackendFactory, type CloseableVideoFrame, type CloseableAudioData, type WebCodecsCallbacks, type WebCodecsBackendOptions, type WebCodecsMetrics`
- `from ./mse: MseBackend, mseBackendFactory, MseError, type MseErrorCode, type MseCallbacks, type MseBackendOptions, type MseMetrics, type HTMLVideoElementLike, type TimeRangesLike, type SourceBufferLike, type MediaSourceLike`
- `from ./ffmpeg-pack: FfmpegPackImpl, FfmpegPackError, PackReturnCode, PackFeatureFlags, PackCodecId, type FfmpegPack, type FfmpegPackModule, type FfmpegPackOptions, type FfmpegPacket, type MemoryDescriptor as FfmpegMemoryDescriptor, type PacketDescriptor as FfmpegPacketDescriptor, type FrameDescriptor as FfmpegFrameDescriptor`
- `from ./video: createRenderer, VideoRenderer, Canvas2DRenderer, WebGL2Renderer, WebGpuRenderer, RendererSurface, resolveColorSpace, buildYuvToRgbCoeffs, getYuvMatrix, getColorRange, type RendererConfig, type RendererMetrics, type RenderFrame, type VisibleRect, type FitMode, type SnapshotOptions, type SnapshotResult, type ColorSpaceInfo, RendererError`
- `from ./observability: MetricRegistry, startTrace, endTrace, childSpan, endSpan, addChild, buildDiagnostics, sanitizeUrl, redactHeaders, DIAGNOSTICS_VERSION, type MetricCategory, type MetricType, type MetricDefinition, type CounterSnapshot, type GaugeSnapshot, type HistogramBucket, type HistogramSnapshot, type MetricSnapshot, type MetricsSnapshot, type TraceSpan, type TraceContext, type DiagnosticsEvent, type DiagnosticsOptions, type DiagnosticsBundle`
- `* from ./audio`
- `export const RUNTIME_VERSION = '0.1.0'`

### @cheetah-media/web

- `from @cheetah-media/runtime: MetricRegistry, startTrace, endTrace, childSpan, endSpan, addChild, buildDiagnostics, sanitizeUrl, redactHeaders, DIAGNOSTICS_VERSION, type MetricCategory, type MetricType, type MetricDefinition, type CounterSnapshot, type GaugeSnapshot, type HistogramBucket, type HistogramSnapshot, type MetricSnapshot, type MetricsSnapshot, type TraceSpan, type TraceContext, type DiagnosticsEvent, type DiagnosticsOptions, type DiagnosticsBundle`
- `from ./abi-constants: ABI_VERSION_MAJOR, ABI_VERSION_MINOR, ABI_VERSION, MEMORY_DESCRIPTOR_SIZE, MEMORY_DESCRIPTOR_OFFSET_REGION, MEMORY_DESCRIPTOR_OFFSET_OFFSET, MEMORY_DESCRIPTOR_OFFSET_LENGTH, MEMORY_DESCRIPTOR_OFFSET_CAPACITY, MEMORY_DESCRIPTOR_OFFSET_GENERATION, MEMORY_DESCRIPTOR_OFFSET_FLAGS, PACKET_DESCRIPTOR_SIZE, PACKET_DESCRIPTOR_OFFSET_TRACK_INDEX, PACKET_DESCRIPTOR_OFFSET_PAYLOAD, PACKET_DESCRIPTOR_OFFSET_SIDE_DATA, PACKET_DESCRIPTOR_OFFSET_PTS_MS, PACKET_DESCRIPTOR_OFFSET_DTS_MS, PACKET_DESCRIPTOR_OFFSET_DURATION_MS, PACKET_DESCRIPTOR_OFFSET_FLAGS, PACKET_DESCRIPTOR_OFFSET_EPOCH, FRAME_DESCRIPTOR_SIZE, FRAME_DESCRIPTOR_OFFSET_TRACK_INDEX, FRAME_DESCRIPTOR_OFFSET_PAYLOAD, FRAME_DESCRIPTOR_OFFSET_PLANES, FRAME_DESCRIPTOR_OFFSET_SIDE_DATA, FRAME_DESCRIPTOR_OFFSET_WIDTH, FRAME_DESCRIPTOR_OFFSET_HEIGHT, FRAME_DESCRIPTOR_OFFSET_PTS_MS, FRAME_DESCRIPTOR_OFFSET_DURATION_MS, FRAME_DESCRIPTOR_OFFSET_FLAGS, FRAME_DESCRIPTOR_OFFSET_EPOCH`
- `from ./player: type CheetahPlayer, type CheetahPlayerEvent, type CheetahPlayerEventType, type EventListener, type PlayerConfig, type TransportConfig, type LatencyConfig, type BackendConfig, type MemoryConfig, type RenderConfig, type AudioConfig, type RecordingConfig, type SecurityConfig, type DiagnosticsConfig, type RuntimeConfig, type PlayerState, type PlayerStats, type DiagnosticsSnapshot, type MemoryDescriptor, type PacketDescriptor, type FrameDescriptor, CheetahMediaError, AbiFeatureFlags, createPlayer`
- `from ./budget: BudgetController, type ResourceBudgetConfig, type CellDemand, type CellAllocation, type StreamProfile, type Resolution`
- `from ./test-helpers: createPlayerWithRuntime`

### @cheetah-media/components

- `{ createPlayer, CheetahPlayerElement, CheetahWallElement, CheetahWallCellElement }`
- `{ CheetahPlayer, PlayerConfig }`
- `export interface PlayerComponentOptions extends PlayerConfig`
- `export interface PlayerComponent`
- `export function createPlayerComponent(options: PlayerComponentOptions =`

## Events, Errors and Message Payloads

| Name | Kind | Source | Values / Note |
|------|------|--------|---------------|
| AudioPipelineError | class | packages/runtime/src/audio/pipeline.ts | `export class AudioPipelineError extends Error { readonly code: string; constructor(code: string, message: string) { supe...` |
| AudioRingBufferFullError | class | packages/runtime/src/audio/ring.ts | `export class AudioRingBufferFullError extends Error { constructor() { super('audio ring buffer full'); this.name = 'Audi...` |
| BackendChangeEvent | interface | packages/runtime/src/fallback.ts | `export interface BackendChangeEvent { readonly from: Backend | undefined; readonly to: Backend; readonly reason: string;...` |
| FallbackEvent | type | packages/runtime/src/fallback.ts | `export type FallbackEvent = | { type: 'backendchange'; payload: BackendChangeEvent }` |
| FfmpegPackErrorCode | type | packages/runtime/src/ffmpeg-pack.ts | `unsupported`, `not-initialized`, `memory`, `abi-mismatch`, `send-failed`, `receive-failed`, `closed` |
| FfmpegPackError | class | packages/runtime/src/ffmpeg-pack.ts | `export type FfmpegPackErrorCode = | 'unsupported' | 'not-initialized' | 'memory' | 'abi-mismatch' | 'send-failed' | 'rec...` |
| WorkerErrorPayload | interface | packages/runtime/src/messages.ts | `export interface WorkerErrorPayload { readonly code: number; readonly stage: string; readonly message: string; readonly ...` |
| BootstrapPayload | interface | packages/runtime/src/messages.ts | `export interface BootstrapPayload { readonly wasmUrl: string; }` |
| CapabilityPayload | interface | packages/runtime/src/messages.ts | `export interface CapabilityPayload { readonly capabilities: import('./capabilities').CapabilityReport; }` |
| LoadPayload | interface | packages/runtime/src/messages.ts | `export interface LoadPayload { readonly url: string; readonly isLive: boolean; }` |
| PacketPayload | interface | packages/runtime/src/messages.ts | `export interface PacketPayload { readonly slot: number; readonly generation: number; readonly trackIndex: number; readon...` |
| OutputPayload | interface | packages/runtime/src/messages.ts | `export interface OutputPayload { readonly slot: number; readonly generation: number; readonly trackIndex: number; readon...` |
| EventPayload | interface | packages/runtime/src/messages.ts | `export interface EventPayload { readonly event: string; readonly details?: Record<string, unknown> | undefined; }` |
| MetricsPayload | interface | packages/runtime/src/messages.ts | `export interface MetricsPayload { readonly dropped: number; readonly queue: string; readonly level: number; }` |
| SnapshotPayload | interface | packages/runtime/src/messages.ts | `export interface SnapshotPayload { readonly maxWidth?: number; readonly maxHeight?: number; }` |
| SnapshotResultPayload | interface | packages/runtime/src/messages.ts | `export interface SnapshotResultPayload { readonly width: number; readonly height: number; readonly data?: Uint8ClampedAr...` |
| SwitchVariantPayload | interface | packages/runtime/src/messages.ts | `export interface SwitchVariantPayload { readonly bandwidth?: number; readonly index?: number; }` |
| RecordingPayload | interface | packages/runtime/src/messages.ts | `export interface RecordingPayload { readonly mimeType?: string; readonly filename?: string; }` |
| StatsPayload | interface | packages/runtime/src/messages.ts | `export interface StatsPayload { readonly bufferedMs?: number; readonly decodedFrames?: number; readonly droppedFrames?: ...` |
| DiagnosticsPayload | interface | packages/runtime/src/messages.ts | `export interface DiagnosticsPayload { readonly playerId: string; readonly version: string; readonly state: string; reado...` |
| MseErrorCode | type | packages/runtime/src/mse.ts | `not-configured`, `mse-not-supported`, `source-open-timeout`, `append-error`, `remove-error`, `source-buffer-error`, `media-source-error`, `video-element-error`, `quota-exceeded`, `unknown` |
| MseError | class | packages/runtime/src/mse.ts | `export type MseErrorCode = | 'not-configured' | 'mse-not-supported' | 'source-open-timeout' | 'append-error' | 'remove-e...` |
| DiagnosticsEvent | interface | packages/runtime/src/observability/diagnostics.ts | `export interface DiagnosticsEvent { readonly type: string; readonly timestamp: number; readonly epoch?: number; readonly...` |
| TransportError | interface | packages/runtime/src/transport.ts | `export interface TransportError { readonly code: number; readonly stage: 'transport'; readonly message: string; readonly...` |
| RendererError | class | packages/runtime/src/video/types.ts | `export class RendererError extends Error { readonly code: string; constructor(code: string, message: string) { super(mes...` |
| CheetahPlayerEventType | type | packages/web/src/player.ts | `statechange`, `tracks`, `firstframe`, `backendchange`, `variantchange`, `buffering`, `stats`, `warning`, `error`, `recording` |
| CheetahPlayerEvent | interface | packages/web/src/player.ts | `export type CheetahPlayerEventType = | 'statechange' | 'tracks' | 'firstframe' | 'backendchange' | 'variantchange' | 'bu...` |
| CheetahMediaError | class | packages/web/src/player.ts | `export class CheetahMediaError extends Error { readonly code: number; readonly stage: string; readonly recoverable: bool...` |

## Notes

- This report is a snapshot of exported identifiers; see the source files for full signatures and documentation.
- Event/error values are extracted from string-literal unions where possible; dynamic objects and class bodies are summarized.
