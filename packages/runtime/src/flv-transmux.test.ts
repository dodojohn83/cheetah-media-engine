import { describe, it, expect } from 'vitest';
import { FlvFmp4TransmuxerJs } from './flv-transmux';

describe('FlvFmp4TransmuxerJs', () => {
  it('rejects string push data', () => {
    const transmuxer = new FlvFmp4TransmuxerJs();
    expect(() => transmuxer.push('not bytes' as unknown as Uint8Array)).toThrow('chunk must be a Uint8Array');
  });

  it('rejects null push data', () => {
    const transmuxer = new FlvFmp4TransmuxerJs();
    expect(() => transmuxer.push(null as unknown as Uint8Array)).toThrow('chunk must be a Uint8Array');
  });

  it('rejects object-with-length push data', () => {
    const transmuxer = new FlvFmp4TransmuxerJs();
    expect(() => transmuxer.push({ length: 4 } as unknown as Uint8Array)).toThrow('chunk must be a Uint8Array');
  });

  it('ignores empty Uint8Array pushes', () => {
    const transmuxer = new FlvFmp4TransmuxerJs();
    expect(() => transmuxer.push(new Uint8Array(0))).not.toThrow();
    expect(transmuxer.getTracks()).toEqual([]);
    expect(transmuxer.poll()).toEqual([]);
  });
});
