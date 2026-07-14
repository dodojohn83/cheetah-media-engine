/**
 * Lock-free ring buffer for audio samples backed by a SharedArrayBuffer.
 *
 * The state block uses Int32 entries so the AudioWorklet can update counters
 * with Atomics.  Sample storage is interleaved Float32 (frame * channels + c).
 * One slot is kept empty to distinguish full from empty.
 */

const IDX_WRITE = 0;
const IDX_READ = 1;
const IDX_CAPACITY = 2;
const IDX_CHANNELS = 3;
const IDX_GENERATION = 4;
const IDX_UNDERRUN = 5;
const IDX_OVERRUN = 6;
const STATE_SLOTS = 8;
const STATE_BYTES = STATE_SLOTS * 4;

export class AudioRingBufferFullError extends Error {
  constructor() {
    super('audio ring buffer full');
    this.name = 'AudioRingBufferFullError';
  }
}

export interface AudioRingMetrics {
  readonly writeIndex: number;
  readonly readIndex: number;
  readonly available: number;
  readonly free: number;
  readonly underrun: number;
  readonly overrun: number;
}

export interface AudioRingBuffer {
  /** Write planar frames. Returns number of frames actually written. */
  write(frames: readonly Float32Array[]): number;
  /** Reset the ring, dropping all samples and bumping generation. */
  reset(): void;
  /** Report that the writer dropped frames because the ring was full. */
  reportOverrun(count: number): void;
  /** Capacity in frames. */
  readonly capacity: number;
  readonly channels: number;
  /** Atomics-safe metrics snapshot. */
  getMetrics(): AudioRingMetrics;
}

export class SharedAudioRingBuffer implements AudioRingBuffer {
  private sab: SharedArrayBuffer;
  private state: Int32Array;
  private samples: Float32Array;
  private _capacity: number;
  private _channels: number;
  private localGeneration = 0;

  constructor(sharedArrayBuffer: SharedArrayBuffer, capacity: number, channels: number) {
    this.sab = sharedArrayBuffer;
    const required = STATE_BYTES + capacity * channels * 4;
    if (sharedArrayBuffer.byteLength < required) {
      throw new Error(
        `SharedArrayBuffer too small: ${sharedArrayBuffer.byteLength} < ${required}`,
      );
    }
    this.state = new Int32Array(sharedArrayBuffer, 0, STATE_SLOTS);
    this.samples = new Float32Array(sharedArrayBuffer, STATE_BYTES, capacity * channels);
    this._capacity = capacity;
    this._channels = channels;
    this.state[IDX_CAPACITY] = capacity;
    this.state[IDX_CHANNELS] = channels;
    this.reset();
  }

  get capacity(): number {
    return this._capacity;
  }

  get channels(): number {
    return this._channels;
  }

  write(frames: readonly Float32Array[]): number {
    const frameCount = frames[0]?.length ?? 0;
    if (frameCount === 0) return 0;

    const writeIndex = Atomics.load(this.state, IDX_WRITE);
    const readIndex = Atomics.load(this.state, IDX_READ);
    const available = (writeIndex - readIndex + this._capacity) % this._capacity;
    const free = Math.max(0, this._capacity - available - 1); // reserve one slot

    const toWrite = Math.min(frameCount, free);
    if (toWrite <= 0) return 0;

    for (let c = 0; c < this._channels; c += 1) {
      const src = frames[c] ?? frames[0] ?? new Float32Array(0);
      for (let i = 0; i < toWrite; i += 1) {
        const pos = (writeIndex + i) % this._capacity;
        this.samples[pos * this._channels + c] = src[i] ?? 0;
      }
    }

    Atomics.store(this.state, IDX_WRITE, (writeIndex + toWrite) % this._capacity);
    return toWrite;
  }

  getBuffer(): SharedArrayBuffer {
    return this.sab;
  }

  reset(): void {
    this.localGeneration += 1;
    Atomics.store(this.state, IDX_READ, 0);
    Atomics.store(this.state, IDX_WRITE, 0);
    Atomics.store(this.state, IDX_GENERATION, this.localGeneration);
    Atomics.store(this.state, IDX_UNDERRUN, 0);
    Atomics.store(this.state, IDX_OVERRUN, 0);
  }

  getMetrics(): AudioRingMetrics {
    const writeIndex = Atomics.load(this.state, IDX_WRITE);
    const readIndex = Atomics.load(this.state, IDX_READ);
    const available = (writeIndex - readIndex + this._capacity) % this._capacity;
    return {
      writeIndex,
      readIndex,
      available,
      free: Math.max(0, this._capacity - available - 1),
      underrun: Atomics.load(this.state, IDX_UNDERRUN),
      overrun: Atomics.load(this.state, IDX_OVERRUN),
    };
  }

  /**
   * Report that samples were dropped because the writer ran out of free space.
   * Called by the main-thread writer, not the worklet.
   */
  reportOverrun(count: number): void {
    Atomics.add(this.state, IDX_OVERRUN, count);
  }

  /** Read a contiguous chunk of up to `maxFrames`.  Used by tests and helpers. */
  read(maxFrames: number): Float32Array[] {
    const writeIndex = Atomics.load(this.state, IDX_WRITE);
    const readIndex = Atomics.load(this.state, IDX_READ);
    const available = (writeIndex - readIndex + this._capacity) % this._capacity;
    const toRead = Math.min(maxFrames, available);
    const out: Float32Array[] = [];
    for (let c = 0; c < this._channels; c += 1) {
      out.push(new Float32Array(toRead));
    }
    for (let i = 0; i < toRead; i += 1) {
      const pos = (readIndex + i) % this._capacity;
      const base = pos * this._channels;
      for (let c = 0; c < this._channels; c += 1) {
        const dst = out[c];
        if (dst) dst[i] = this.samples[base + c] ?? 0;
      }
    }
    Atomics.store(this.state, IDX_READ, (readIndex + toRead) % this._capacity);
    return out;
  }
}

/**
 * Single-threaded fallback ring used when Atomics/SharedArrayBuffer are not
 * available.  API-compatible with SharedAudioRingBuffer so the pipeline can
 * swap it in for non-isolated mode.
 */
export class LocalAudioRingBuffer implements AudioRingBuffer {
  private _capacity: number;
  private _channels: number;
  private writeIndex = 0;
  private readIndex = 0;
  private samples: Float32Array;
  private underrun = 0;
  private overrun = 0;
  private generation = 0;

  constructor(capacity: number, channels: number) {
    this._capacity = capacity;
    this._channels = channels;
    this.samples = new Float32Array(capacity * channels);
  }

  get capacity(): number {
    return this._capacity;
  }

  get channels(): number {
    return this._channels;
  }

  write(frames: readonly Float32Array[]): number {
    const frameCount = frames[0]?.length ?? 0;
    if (frameCount === 0) return 0;
    const available = (this.writeIndex - this.readIndex + this._capacity) % this._capacity;
    const free = Math.max(0, this._capacity - available - 1);
    const toWrite = Math.min(frameCount, free);
    for (let c = 0; c < this._channels; c += 1) {
      const src = frames[c] ?? frames[0] ?? new Float32Array(0);
      for (let i = 0; i < toWrite; i += 1) {
        const pos = (this.writeIndex + i) % this._capacity;
        this.samples[pos * this._channels + c] = src[i] ?? 0;
      }
    }
    this.writeIndex = (this.writeIndex + toWrite) % this._capacity;
    return toWrite;
  }

  reset(): void {
    this.generation += 1;
    this.readIndex = 0;
    this.writeIndex = 0;
    this.underrun = 0;
    this.overrun = 0;
  }

  getMetrics(): AudioRingMetrics {
    const available = (this.writeIndex - this.readIndex + this._capacity) % this._capacity;
    return {
      writeIndex: this.writeIndex,
      readIndex: this.readIndex,
      available,
      free: Math.max(0, this._capacity - available - 1),
      underrun: this.underrun,
      overrun: this.overrun,
    };
  }

  reportOverrun(count: number): void {
    this.overrun += count;
  }

  /** Used by non-isolated worklet fallback to consume blocks sent over a MessagePort. */
  consumeBlock(block: Float32Array, frames: number, channels: number): number {
    const written = this.writeInterleaved(block, frames, channels);
    if (written < frames) {
      this.reportOverrun(frames - written);
    }
    return written;
  }

  private writeInterleaved(block: Float32Array, frameCount: number, channels: number): number {
    const available = (this.writeIndex - this.readIndex + this._capacity) % this._capacity;
    const free = Math.max(0, this._capacity - available - 1);
    const toWrite = Math.min(frameCount, free);
    for (let i = 0; i < toWrite; i += 1) {
      const pos = (this.writeIndex + i) % this._capacity;
      for (let c = 0; c < this._channels; c += 1) {
        this.samples[pos * this._channels + c] = block[i * channels + c] ?? 0;
      }
    }
    this.writeIndex = (this.writeIndex + toWrite) % this._capacity;
    return toWrite;
  }
}
