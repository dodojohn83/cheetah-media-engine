import { describe, expect, it } from 'vitest';
import { encodeG711F32One, encodeG711Int16 } from './g711';

describe('G.711 encoder', () => {
  it('encodes mu-law silence to 0xff', () => {
    expect(encodeG711F32One('mulaw', 0.0)).toBe(0xff);
  });

  it('encodes A-law silence to 0xd5', () => {
    expect(encodeG711F32One('alaw', 0.0)).toBe(0xd5);
  });

  it('encodes positive and negative extremes', () => {
    expect(encodeG711F32One('alaw', 1.0)).toBe(0xaa);
    expect(encodeG711F32One('alaw', -1.0)).toBe(0x2a);
    expect(encodeG711F32One('mulaw', 1.0)).toBe(0x80);
    expect(encodeG711F32One('mulaw', -1.0)).toBe(0x00);
  });

  it('encodes an Int16 buffer into the output', () => {
    const input = new Int16Array([0, 100, -100, 16000, -16000]);
    const output = new Uint8Array(input.length);
    encodeG711Int16('mulaw', input, output);
    expect(output[0]).toBe(0xff);
    expect(output[1]).not.toBe(output[2]);
    expect(output[3]).not.toBe(output[4]);
  });

  it('does not write beyond output length', () => {
    const input = new Int16Array([0, 0, 0]);
    const output = new Uint8Array(1);
    encodeG711Int16('alaw', input, output);
    expect(output.length).toBe(1);
    expect(output[0]).toBe(0xd5);
  });
});
