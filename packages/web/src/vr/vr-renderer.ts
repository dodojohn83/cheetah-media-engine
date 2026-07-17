import type { ProcessableFrame, VrProjectionMetadata } from './types';

/**
 * Pluggable VR / 360° renderer extension.
 *
 * The player calls `initialize` once when a 360° projection is detected and
 * `render` for every decoded frame. The default `NoopVrRenderer` disables the
 * feature and lets the normal 2D renderer handle the frame.
 */
export interface VrRenderer {
  /** Whether the renderer is currently active and consuming frames. */
  readonly active: boolean;

  /**
   * Prepare the renderer on the supplied output surface.
   *
   * @param surface The canvas that the player renders into.
   * @param metadata Projection metadata detected from the stream.
   * @returns `true` if the renderer was initialized successfully.
   */
  initialize(surface: HTMLCanvasElement | OffscreenCanvas, metadata: VrProjectionMetadata): boolean;

  /**
   * Render a frame.
   *
   * @param frame The decoded frame ready for display.
   */
  render(frame: ProcessableFrame): void;

  /** Release GPU/CPU resources and deactivate the renderer. */
  destroy(): void;
}

/** Default no-op VR renderer. */
export class NoopVrRenderer implements VrRenderer {
  get active(): boolean {
    return false;
  }

  initialize(): boolean {
    return false;
  }

  render(): void {
    // No-op: the standard 2D renderer draws the frame.
  }

  destroy(): void {
    // Nothing to release.
  }
}
