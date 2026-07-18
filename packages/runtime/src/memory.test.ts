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
});
