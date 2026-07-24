/**
 * Minimal fMP4 box helpers for progressive and fragmented streams.
 *
 * Used by the playback session to split init (ftyp+moov) from media
 * fragments (moof+mdat) before appending to MSE.
 */

export interface Fmp4Split {
  readonly init: Uint8Array;
  readonly segments: readonly Uint8Array[];
}

export interface BoxHeader {
  readonly size: number;
  readonly headerSize: number;
  readonly type: string;
}

function readBoxSize(data: Uint8Array, offset: number): { size: number; headerSize: number } | undefined {
  if (offset + 8 > data.length) return undefined;
  const dv = new DataView(data.buffer, data.byteOffset + offset, Math.min(16, data.length - offset));
  const size = dv.getUint32(0, false);
  if (size === 0) {
    return { size: data.length - offset, headerSize: 8 };
  }
  if (size === 1) {
    if (offset + 16 > data.length) return undefined;
    const high = dv.getUint32(8, false);
    const low = dv.getUint32(12, false);
    const extended = high * 0x100000000 + low;
    return { size: extended, headerSize: 16 };
  }
  if (size < 8) return undefined;
  return { size, headerSize: 8 };
}

function boxType(data: Uint8Array, offset: number): string {
  return String.fromCharCode(
    data[offset + 4]!,
    data[offset + 5]!,
    data[offset + 6]!,
    data[offset + 7]!,
  );
}

export function concatUint8(chunks: readonly Uint8Array[]): Uint8Array {
  if (!Array.isArray(chunks)) {
    throw new Error('concatUint8 chunks must be an array');
  }
  let total = 0;
  for (const c of chunks) {
    if (!isUint8Array(c)) {
      throw new Error('concatUint8 chunks must contain only Uint8Array instances');
    }
    total += c.length;
  }
  const out = new Uint8Array(total);
  let off = 0;
  for (const c of chunks) {
    out.set(c, off);
    off += c.length;
  }
  return out;
}

function isUint8Array(value: unknown): value is Uint8Array {
  if (typeof Uint8Array !== 'undefined' && value instanceof Uint8Array) return true;
  return Object.prototype.toString.call(value) === '[object Uint8Array]';
}

/** Parse a single box header if a complete header is present. */
function isNonNegativeInteger(value: number): boolean {
  return Number.isFinite(value) && value >= 0 && Number.isInteger(value);
}

export function peekBox(data: Uint8Array, offset = 0): BoxHeader | undefined {
  if (!isUint8Array(data)) {
    throw new Error('peekBox data must be a Uint8Array');
  }
  if (!isNonNegativeInteger(offset)) {
    throw new Error('peekBox offset must be a non-negative integer');
  }
  const sizeInfo = readBoxSize(data, offset);
  if (!sizeInfo) return undefined;
  if (offset + 8 > data.length) return undefined;
  return {
    size: sizeInfo.size,
    headerSize: sizeInfo.headerSize,
    type: boxType(data, offset),
  };
}

/**
 * Split a complete fMP4 buffer into init segment and media segments.
 * Partial trailing bytes are dropped (caller should buffer them).
 */
export function splitFmp4(data: Uint8Array): Fmp4Split {
  if (!isUint8Array(data)) {
    throw new Error('splitFmp4 data must be a Uint8Array');
  }
  const initChunks: Uint8Array[] = [];
  const segments: Uint8Array[] = [];
  let current: Uint8Array[] = [];
  let offset = 0;
  while (offset < data.length) {
    const box = readBoxSize(data, offset);
    if (!box) break;
    if (offset + box.size > data.length) break;
    const type = boxType(data, offset);
    const chunk = data.subarray(offset, offset + box.size);
    if (type === 'ftyp' || type === 'moov') {
      initChunks.push(chunk);
    } else if (type === 'moof') {
      if (current.length > 0) {
        segments.push(concatUint8(current));
      }
      current = [chunk];
    } else if (type === 'mfra' || type === 'free' || type === 'skip' || type === 'meta') {
      if (current.length > 0) {
        segments.push(concatUint8(current));
        current = [];
      }
    } else {
      current.push(chunk);
    }
    offset += box.size;
  }
  if (current.length > 0) {
    segments.push(concatUint8(current));
  }
  return { init: concatUint8(initChunks), segments };
}

/**
 * Incremental fMP4 box accumulator. Call `push` with network chunks and
 * drain complete boxes via `takeCompleteBoxes`.
 */
export class Fmp4BoxAccumulator {
  private buffer: Uint8Array = new Uint8Array(0);
  private readonly maxBytes: number;

  constructor(maxBytes = 64 * 1024 * 1024) {
    this.maxBytes = maxBytes;
  }

  get length(): number {
    return this.buffer.length;
  }

  push(chunk: Uint8Array): void {
    if (!isUint8Array(chunk)) {
      throw new Error('Fmp4BoxAccumulator.push chunk must be a Uint8Array');
    }
    if (chunk.length === 0) return;
    if (this.buffer.length + chunk.length > this.maxBytes) {
      throw new Error(`fMP4 buffer exceeded ${this.maxBytes} bytes`);
    }
    this.buffer = concatUint8([this.buffer, chunk]);
  }

  /**
   * Extract complete top-level boxes. Incomplete trailing bytes remain buffered.
   */
  takeCompleteBoxes(): Uint8Array[] {
    const boxes: Uint8Array[] = [];
    let offset = 0;
    while (offset < this.buffer.length) {
      const box = readBoxSize(this.buffer, offset);
      if (!box) break;
      if (offset + box.size > this.buffer.length) break;
      boxes.push(this.buffer.subarray(offset, offset + box.size));
      offset += box.size;
    }
    if (offset > 0) {
      this.buffer = this.buffer.subarray(offset);
      // Copy so the returned subarrays are not invalidated by future grows
      // that reallocate `this.buffer` via concat.
      this.buffer = this.buffer.slice();
    }
    return boxes;
  }

  clear(): void {
    this.buffer = new Uint8Array(0);
  }
}

/**
 * Group a sequence of complete boxes into init + media segments suitable
 * for MSE appendBuffer.
 */
export class Fmp4SegmentBuilder {
  private initChunks: Uint8Array[] = [];
  private current: Uint8Array[] = [];
  private initReady = false;
  private initEmitted = false;

  /**
   * Feed one complete top-level box. Returns segments ready to append.
   * The first emitted segment with `isInit: true` is the init segment.
   */
  feed(box: Uint8Array): { data: Uint8Array; isInit: boolean }[] {
    if (!isUint8Array(box)) {
      throw new Error('Fmp4SegmentBuilder.feed box must be a Uint8Array');
    }
    if (box.length < 8) {
      throw new Error('Fmp4SegmentBuilder.feed box must be at least 8 bytes');
    }
    const sizeInfo = readBoxSize(box, 0);
    if (!sizeInfo || sizeInfo.size !== box.length) {
      throw new Error('Fmp4SegmentBuilder.feed box size does not match declared box length');
    }
    const type = boxType(box, 0);
    const out: { data: Uint8Array; isInit: boolean }[] = [];

    if (type === 'ftyp' || type === 'moov') {
      this.initChunks.push(box);
      if (type === 'moov') {
        this.initReady = true;
        if (!this.initEmitted) {
          this.initEmitted = true;
          out.push({ data: concatUint8(this.initChunks), isInit: true });
        }
      }
      return out;
    }

    if (type === 'moof') {
      if (this.current.length > 0) {
        out.push({ data: concatUint8(this.current), isInit: false });
      }
      this.current = [box];
      return out;
    }

    if (type === 'mfra' || type === 'free' || type === 'skip' || type === 'meta') {
      if (this.current.length > 0) {
        out.push({ data: concatUint8(this.current), isInit: false });
        this.current = [];
      }
      return out;
    }

    // mdat and other media boxes attach to the current fragment.
    if (this.current.length > 0) {
      this.current.push(box);
    } else if (this.initReady && !this.initEmitted) {
      // Progressive moov-only path: treat remaining as one media segment after init.
      this.initEmitted = true;
      out.push({ data: concatUint8(this.initChunks), isInit: true });
      this.current = [box];
    } else {
      this.current.push(box);
    }
    return out;
  }

  flush(): { data: Uint8Array; isInit: boolean }[] {
    const out: { data: Uint8Array; isInit: boolean }[] = [];
    if (this.initReady && !this.initEmitted && this.initChunks.length > 0) {
      this.initEmitted = true;
      out.push({ data: concatUint8(this.initChunks), isInit: true });
    }
    if (this.current.length > 0) {
      out.push({ data: concatUint8(this.current), isInit: false });
      this.current = [];
    }
    return out;
  }

  get hasInit(): boolean {
    return this.initEmitted || this.initReady;
  }
}
