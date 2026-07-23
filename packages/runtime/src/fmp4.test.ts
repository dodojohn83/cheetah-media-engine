import { describe, expect, it } from 'vitest';
import {
  Fmp4BoxAccumulator,
  Fmp4SegmentBuilder,
  concatUint8,
  peekBox,
  splitFmp4,
} from './fmp4';

function makeBox(type: string, payload: Uint8Array = new Uint8Array(0)): Uint8Array {
  const size = 8 + payload.length;
  const out = new Uint8Array(size);
  const dv = new DataView(out.buffer);
  dv.setUint32(0, size, false);
  out[4] = type.charCodeAt(0);
  out[5] = type.charCodeAt(1);
  out[6] = type.charCodeAt(2);
  out[7] = type.charCodeAt(3);
  out.set(payload, 8);
  return out;
}

describe('fmp4 helpers', () => {
  it('splits init and media segments', () => {
    const ftyp = makeBox('ftyp', new Uint8Array([1, 2]));
    const moov = makeBox('moov', new Uint8Array([3]));
    const moof = makeBox('moof', new Uint8Array([4]));
    const mdat = makeBox('mdat', new Uint8Array([5, 6, 7]));
    const all = concatUint8([ftyp, moov, moof, mdat]);
    const split = splitFmp4(all);
    expect(split.init.length).toBe(ftyp.length + moov.length);
    expect(split.segments.length).toBe(1);
    expect(split.segments[0]!.length).toBe(moof.length + mdat.length);
  });

  it('accumulates incomplete boxes across chunks', () => {
    const moof = makeBox('moof', new Uint8Array([9, 9]));
    const acc = new Fmp4BoxAccumulator();
    acc.push(moof.subarray(0, 4));
    expect(acc.takeCompleteBoxes()).toEqual([]);
    acc.push(moof.subarray(4));
    const boxes = acc.takeCompleteBoxes();
    expect(boxes.length).toBe(1);
    expect(peekBox(boxes[0]!)?.type).toBe('moof');
  });

  it('builds init then media segments incrementally', () => {
    const builder = new Fmp4SegmentBuilder();
    const ftyp = makeBox('ftyp');
    const moov = makeBox('moov');
    const moof = makeBox('moof');
    const mdat = makeBox('mdat', new Uint8Array([1]));
    expect(builder.feed(ftyp)).toEqual([]);
    const afterMoov = builder.feed(moov);
    expect(afterMoov.length).toBe(1);
    expect(afterMoov[0]!.isInit).toBe(true);
    expect(builder.feed(moof)).toEqual([]);
    const media = builder.feed(mdat);
    // mdat attaches; flush produces the fragment
    expect(media).toEqual([]);
    const flushed = builder.flush();
    expect(flushed.length).toBe(1);
    expect(flushed[0]!.isInit).toBe(false);
  });

  it('rejects non-Uint8Array inputs', () => {
    expect(() => splitFmp4('not bytes' as unknown as Uint8Array)).toThrow('splitFmp4 data must be a Uint8Array');
    expect(() => peekBox(null as unknown as Uint8Array)).toThrow('peekBox data must be a Uint8Array');

    const acc = new Fmp4BoxAccumulator();
    expect(() => acc.push(null as unknown as Uint8Array)).toThrow('chunk must be a Uint8Array');

    const builder = new Fmp4SegmentBuilder();
    expect(() => builder.feed(null as unknown as Uint8Array)).toThrow('box must be a Uint8Array');
  });
});
