/**
 * Small linear resampler for near-ratio corrections and sample-rate conversion.
 *
 * Operates on planar Float32 samples.  Keeps leftover samples between calls so
 * it can be fed one decoder output at a time without copying whole buffers.
 */

export interface ResamplerOptions {
  readonly inputSampleRate: number;
  readonly outputSampleRate: number;
  readonly channels: number;
  readonly minRatio?: number;
  readonly maxRatio?: number;
  /** Maximum ratio step per push to avoid audible jumps. */
  readonly maxRatioDelta?: number;
}

export class AudioResampler {
  readonly channels: number;
  private baseRatio: number;
  private minRatio: number;
  private maxRatio: number;
  private maxRatioDelta: number;
  private ratio: number;
  private position = 0;
  private leftover: Float32Array[] = [];

  constructor(options: ResamplerOptions) {
    if (!Number.isFinite(options.inputSampleRate) || options.inputSampleRate <= 0) {
      throw new Error('Resampler inputSampleRate must be a finite positive number');
    }
    if (!Number.isFinite(options.outputSampleRate) || options.outputSampleRate <= 0) {
      throw new Error('Resampler outputSampleRate must be a finite positive number');
    }
    if (!Number.isFinite(options.channels) || options.channels <= 0 || options.channels % 1 !== 0) {
      throw new Error('Resampler channels must be a positive integer');
    }
    this.channels = options.channels;
    this.baseRatio = options.inputSampleRate / options.outputSampleRate;
    this.minRatio = options.minRatio ?? 0.95;
    this.maxRatio = options.maxRatio ?? 1.05;
    this.maxRatioDelta = options.maxRatioDelta ?? 0.01;
    if (!Number.isFinite(this.minRatio) || this.minRatio <= 0) {
      throw new Error('Resampler minRatio must be a finite positive number');
    }
    if (!Number.isFinite(this.maxRatio) || this.maxRatio <= 0 || this.maxRatio < this.minRatio) {
      throw new Error('Resampler maxRatio must be a finite positive number >= minRatio');
    }
    if (!Number.isFinite(this.maxRatioDelta) || this.maxRatioDelta < 0) {
      throw new Error('Resampler maxRatioDelta must be a finite non-negative number');
    }
    this.ratio = this.baseRatio;
  }

  get currentRatio(): number {
    return this.ratio;
  }

  /** Adapt the resampling ratio to correct clock drift without sudden jumps. */
  setRatio(ratio: number): void {
    const minAbsolute = this.baseRatio * this.minRatio;
    const maxAbsolute = this.baseRatio * this.maxRatio;
    const clamped = Math.max(minAbsolute, Math.min(maxAbsolute, ratio));
    const delta = clamped - this.ratio;
    if (delta > this.maxRatioDelta) {
      this.ratio += this.maxRatioDelta;
    } else if (delta < -this.maxRatioDelta) {
      this.ratio -= this.maxRatioDelta;
    } else {
      this.ratio = clamped;
    }
  }

  /** Reset internal state after a seek or sync baseline change. */
  reset(): void {
    this.position = 0;
    this.leftover = [];
    this.ratio = this.baseRatio;
  }

  private combineInput(frames: readonly Float32Array[]): Float32Array[] {
    const full: Float32Array[] = [];
    for (let c = 0; c < this.channels; c += 1) {
      const prev = this.leftover.at(c) ?? new Float32Array(0);
      const next = frames.at(c) ?? frames.at(0) ?? new Float32Array(0);
      const combined = new Float32Array(prev.length + next.length);
      combined.set(prev);
      combined.set(next, prev.length);
      full.push(combined);
    }
    return full;
  }

  private resample(full: readonly Float32Array[]): Float32Array[] {
    const first = full.at(0);
    if (!first) {
      return [];
    }
    const available = first.length - 1; // need one sample ahead for interpolation
    if (available <= 0) {
      return [];
    }

    const outFrames: number[] = [];
    while (Math.floor(this.position) < available) {
      const index = Math.floor(this.position);
      const frac = this.position - index;
      for (let c = 0; c < this.channels; c += 1) {
        const ch = full.at(c);
        const a = ch?.at(index) ?? 0;
        const b = ch?.at(index + 1) ?? a;
        outFrames.push(a + (b - a) * frac);
      }
      this.position += this.ratio;
    }

    const totalOutputFrames = Math.floor(outFrames.length / this.channels);
    const output: Float32Array[] = [];
    for (let c = 0; c < this.channels; c += 1) {
      const dst = new Float32Array(totalOutputFrames);
      for (let i = 0; i < totalOutputFrames; i += 1) {
        dst[i] = outFrames.at(i * this.channels + c) ?? 0;
      }
      output.push(dst);
    }
    return output;
  }

  /**
   * Resample one chunk of planar Float32 data.  Returns planar output.
   * Any un-consumed input is retained for the next call.
   */
  push(frames: readonly Float32Array[]): Float32Array[] {
    const full = this.combineInput(frames);
    const output = this.resample(full);

    const consumed = Math.floor(this.position);

    this.leftover = [];
    for (let c = 0; c < this.channels; c += 1) {
      const ch = full.at(c) ?? new Float32Array(0);
      this.leftover.push(ch.slice(consumed));
    }
    this.position -= consumed;

    return output;
  }

  /** Flush any remaining input as output, even if incomplete. */
  flush(): Float32Array[] {
    const full: Float32Array[] = [];
    for (let c = 0; c < this.channels; c += 1) {
      const prev = this.leftover.at(c) ?? new Float32Array(0);
      const extended = new Float32Array(prev.length + 1);
      extended.set(prev);
      extended[prev.length] = prev.at(prev.length - 1) ?? 0;
      full.push(extended);
    }

    const output = this.resample(full);

    this.leftover = [];
    this.position = 0;
    return output;
  }
}
