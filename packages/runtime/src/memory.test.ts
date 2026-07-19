import { describe, it, expect } from 'vitest';
import { MemoryArenaView } from './memory';

describe('MemoryArenaView', () => {
  it('rejects bigint offsets that exceed safe integer range', () => {
    const view = new MemoryArenaView(new WebAssembly.Memory({ initial: 1 }));
    expect(() =>
      view.getUint8Array({
        region: 0,
        offset: BigInt(Number.MAX_SAFE_INTEGER) + 1n,
        length: 1,
        capacity: 1,
        generation: 0,
        flags: 0,
      }),
    ).toThrow(RangeError);
  });

  it('rejects non-finite, negative, or out-of-bounds offsets and lengths', () => {
    const view = new MemoryArenaView(new WebAssembly.Memory({ initial: 1 }));
    const base = { region: 0, capacity: 1, generation: 0, flags: 0 };
    expect(() => view.getUint8Array({ ...base, offset: NaN, length: 1 })).toThrow(RangeError);
    expect(() => view.getUint8Array({ ...base, offset: -1, length: 1 })).toThrow(RangeError);
    expect(() => view.getUint8Array({ ...base, offset: 0, length: -1 })).toThrow(RangeError);
    expect(() => view.getUint8Array({ ...base, offset: 0, length: 70000 })).toThrow('Descriptor region out of bounds');
    expect(() => view.getUint8Array({ ...base, offset: BigInt(-1), length: 1 })).toThrow(RangeError);
  });
});
