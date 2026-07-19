/**
 * Minimal descriptor used by the arena view to avoid a cyclic dependency on
 * `@cheetah-media/web`.
 */
export interface MemoryDescriptor {
  readonly region: number;
  readonly offset: number | bigint;
  readonly length: number;
  readonly capacity: number;
  readonly generation: number | bigint;
  readonly flags: number;
}

/**
 * Arena view over a WebAssembly.Memory buffer.
 *
 * All TypedArray views are created on demand and never cached across await
 * boundaries, so memory growth cannot produce stale views.
 */
export class MemoryArenaView {
  private memory: WebAssembly.Memory;

  constructor(memory: WebAssembly.Memory) {
    this.memory = memory;
  }

  /** Re-obtain the current buffer after memory growth. */
  refresh(): void {
    // No cached view is stored; all methods read from memory.buffer directly.
  }

  /** Return a Uint8Array slice for the descriptor's region. */
  getUint8Array(desc: MemoryDescriptor): Uint8Array {
    const buffer = this.memory.buffer;
    const offset = memoryOffsetToNumber(desc.offset);
    const length = desc.length;
    if (!Number.isFinite(offset) || offset < 0 || !Number.isFinite(length) || length < 0) {
      throw new RangeError('Descriptor offset and length must be finite non-negative numbers');
    }
    if (offset > buffer.byteLength || length > buffer.byteLength - offset) {
      throw new Error('Descriptor region out of bounds');
    }
    return new Uint8Array(buffer, offset, length);
  }

  /** Copy the descriptor payload into a new ArrayBuffer. */
  copyBuffer(desc: MemoryDescriptor): ArrayBuffer {
    const src = this.getUint8Array(desc);
    const dst = new Uint8Array(src.length);
    dst.set(src);
    return dst.buffer;
  }
}

/** Convert a possibly-bigint offset to a byte offset, guarding precision. */
function memoryOffsetToNumber(offset: number | bigint): number {
  if (typeof offset === 'number') {
    if (!Number.isFinite(offset) || offset < 0) {
      throw new RangeError('Memory offset must be a finite non-negative number');
    }
    return offset;
  }
  if (offset < 0n || offset > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new RangeError('Memory offset exceeds safe integer range or is negative');
  }
  return Number(offset);
}
