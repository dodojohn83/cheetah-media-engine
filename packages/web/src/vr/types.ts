/**
 * Shared types for VR/360° rendering and AI frame processing extensions.
 *
 * These are intentionally lightweight placeholders: the full 360° renderer and
 * ML inference pipeline are out of scope for the current milestone. The
 * interfaces allow third-party plugins to register themselves with the player
 * without the core knowing their internal details.
 */

/** Frame-like input that can be delivered to a processor or renderer. */
export interface ProcessableFrame {
  readonly width: number;
  readonly height: number;
  /** Presentation timestamp in milliseconds. */
  readonly timestampMs: number;
  /** Underlying video/canvas element or WebCodecs VideoFrame. */
  readonly source: HTMLVideoElement | HTMLCanvasElement | OffscreenCanvas | ImageBitmap | VideoFrame;
}

/** VR projection metadata extracted from the stream or supplied by the caller. */
export interface VrProjectionMetadata {
  readonly projection: 'equirectangular' | 'cubemap' | 'flat';
  readonly yaw?: number;
  readonly pitch?: number;
  readonly roll?: number;
}

/** Per-frame resource budget supplied to an AI processor. */
export interface AiFrameBudget {
  /** Maximum time the processor may spend on this frame in milliseconds. */
  readonly deadlineMs: number;
  /** Whether the player has spare CPU/GPU headroom this frame. */
  readonly canAllocate: boolean;
}

/** Detected object / region result from an AI processor. */
export interface AiDetectionBox {
  readonly label: string;
  readonly confidence: number;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/** Result returned by an AI frame processor; may be undefined if skipped. */
export interface AiFrameResult {
  readonly boxes: readonly AiDetectionBox[];
}
