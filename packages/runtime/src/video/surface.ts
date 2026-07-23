/**
 * Canvas surface management: DPR, resize, fit modes and transform matrices.
 */

import type { FitMode, VisibleRect, RendererConfig } from './types';
import { RendererError } from './types';

export interface Viewport {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface SurfaceTransform {
  /** Horizontal scale (mirror multiplies by -1). */
  readonly scaleX: number;
  /** Vertical scale. */
  readonly scaleY: number;
  /** Clockwise rotation in degrees. */
  readonly rotation: number;
}

export class RendererSurface {
  private canvas: HTMLCanvasElement | OffscreenCanvas;
  private cssWidth = 0;
  private cssHeight = 0;
  private _dpr = 1;
  private fit: FitMode = 'contain';
  private rotation = 0;
  private mirror = false;

  constructor(canvas: HTMLCanvasElement | OffscreenCanvas) {
    this.canvas = canvas;
    this.cssWidth = canvas.width;
    this.cssHeight = canvas.height;
  }

  get width(): number {
    return this.cssWidth;
  }

  get height(): number {
    return this.cssHeight;
  }

  get dpr(): number {
    return this._dpr;
  }

  getCanvas(): HTMLCanvasElement | OffscreenCanvas {
    return this.canvas;
  }

  getContext2d(): CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D | null {
    return (this.canvas as HTMLCanvasElement).getContext?.('2d') ??
      (this.canvas as OffscreenCanvas).getContext?.('2d') ??
      null;
  }

  getWebGlContext(): WebGL2RenderingContext | null {
    const gl =
      (this.canvas as HTMLCanvasElement).getContext?.('webgl2') ??
      (this.canvas as OffscreenCanvas).getContext?.('webgl2') ??
      null;
    return gl as WebGL2RenderingContext | null;
  }

  configure(config: RendererConfig): void {
    if (!config.canvas) {
      throw new RendererError('invalid-config', 'RendererSurface requires a canvas');
    }
    const canvasWidth = config.canvas.width;
    const canvasHeight = config.canvas.height;
    if (
      !Number.isFinite(canvasWidth) ||
      !Number.isFinite(canvasHeight) ||
      canvasWidth <= 0 ||
      canvasHeight <= 0
    ) {
      throw new RendererError('invalid-config', 'Canvas width and height must be finite positive numbers');
    }
    if (config.dpr !== undefined) {
      if (!Number.isFinite(config.dpr) || config.dpr <= 0) {
        throw new RendererError('invalid-config', 'dpr must be a finite positive number');
      }
      this._dpr = config.dpr;
    }
    if (config.fit) {
      const allowed: readonly FitMode[] = ['contain', 'cover', 'fill', 'stretch'];
      if (!allowed.includes(config.fit)) {
        throw new RendererError('invalid-config', `Unknown fit mode: ${config.fit}`);
      }
      this.fit = config.fit;
    }
    if (config.rotation !== undefined) {
      if (!Number.isFinite(config.rotation)) {
        throw new RendererError('invalid-config', 'rotation must be a finite number');
      }
      this.rotation = config.rotation % 360;
    }
    if (config.mirror !== undefined) this.mirror = config.mirror;
    this.resize(config.canvas.width, config.canvas.height);
  }

  resize(cssWidth: number, cssHeight: number): void {
    if (cssWidth <= 0 || cssHeight <= 0) return;
    this.cssWidth = cssWidth;
    this.cssHeight = cssHeight;
    const realWidth = Math.max(1, Math.floor(cssWidth * this._dpr));
    const realHeight = Math.max(1, Math.floor(cssHeight * this._dpr));
    this.canvas.width = realWidth;
    this.canvas.height = realHeight;
  }

  /** Compute the viewport rectangle in real pixels that fits the frame. */
  computeViewport(frameWidth: number, frameHeight: number): Viewport {
    const sw = this.canvas.width;
    const sh = this.canvas.height;
    if (frameWidth <= 0 || frameHeight <= 0 || sw <= 0 || sh <= 0) {
      return { x: 0, y: 0, width: sw, height: sh };
    }

    const frameRatio = frameWidth / frameHeight;
    const surfaceRatio = sw / sh;
    let w = sw;
    let h = sh;

    switch (this.fit) {
      case 'contain':
        if (frameRatio > surfaceRatio) {
          h = sw / frameRatio;
        } else {
          w = sh * frameRatio;
        }
        break;
      case 'cover':
        if (frameRatio > surfaceRatio) {
          w = sh * frameRatio;
        } else {
          h = sw / frameRatio;
        }
        break;
      case 'fill':
      case 'stretch':
      default:
        w = sw;
        h = sh;
        break;
    }

    const x = (sw - w) / 2;
    const y = (sh - h) / 2;
    return { x, y, width: w, height: h };
  }

  /** Compute the visible rect inside the coded frame (defaults to full frame). */
  static resolveVisibleRect(frame: {
    codedWidth: number;
    codedHeight: number;
    visibleRect?: VisibleRect | undefined;
  }): VisibleRect {
    return (
      frame.visibleRect ?? {
        x: 0,
        y: 0,
        width: frame.codedWidth,
        height: frame.codedHeight,
      }
    );
  }

  getTransform(): SurfaceTransform {
    return {
      scaleX: this.mirror ? -1 : 1,
      scaleY: 1,
      rotation: this.rotation,
    };
  }
}
