import { describe, it, expect } from 'vitest';
import { TsFmp4TransmuxerJs } from './ts-transmux';

describe('TsFmp4TransmuxerJs', () => {
  it('rejects non-Uint8Array push data', () => {
    const transmuxer = new TsFmp4TransmuxerJs();
    expect(() => transmuxer.push('not bytes' as unknown as Uint8Array)).toThrow('chunk must be a Uint8Array');
    expect(() => transmuxer.push(null as unknown as Uint8Array)).toThrow('chunk must be a Uint8Array');
    expect(() => transmuxer.push({ length: 188 } as unknown as Uint8Array)).toThrow('chunk must be a Uint8Array');
  });

  it('ignores empty Uint8Array pushes', () => {
    const transmuxer = new TsFmp4TransmuxerJs();
    expect(() => transmuxer.push(new Uint8Array(0))).not.toThrow();
    expect(transmuxer.getTracks()).toEqual([]);
    expect(transmuxer.poll()).toEqual([]);
  });
});
