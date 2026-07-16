import { describe, it, expect } from 'vitest';
import { createGb28181PtzCmd, type PtzCommand } from './ptz';

function parseHex(hex: string): number[] {
  const bytes: number[] = [];
  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(parseInt(hex.slice(i, i + 2), 16));
  }
  return bytes;
}

function checksum(bytes: number[]): number {
  return bytes.slice(0, 7).reduce((sum, b) => sum + b, 0) & 0xff;
}

describe('createGb28181PtzCmd', () => {
  it('generates a stop command', () => {
    const cmd = createGb28181PtzCmd({ action: 'stop' });
    expect(cmd).toMatch(/^[0-9A-F]{16}$/);
    const bytes = parseHex(cmd);
    expect(bytes[0]).toBe(0xa5);
    expect(bytes[1]).toBe(0x0f);
    expect(bytes[2]).toBe(0x01);
    expect(bytes[3]).toBe(0x00);
    expect(bytes[4]).toBe(0x00);
    expect(bytes[5]).toBe(0x00);
    expect(bytes[6]).toBe(0x00);
    expect(bytes[7]).toBe(checksum(bytes));
  });

  it('includes horizontal and vertical speeds for pan/tilt', () => {
    const cmd = createGb28181PtzCmd({ action: 'upRight', speeds: { horizontal: 10, vertical: 20 } });
    const bytes = parseHex(cmd);
    expect(bytes[3]).toBe(0x09); // up | right
    expect(bytes[4]).toBe(10);
    expect(bytes[5]).toBe(20);
    expect(bytes[7]).toBe(checksum(bytes));
  });

  it('places zoom speed in the high nibble', () => {
    const cmd = createGb28181PtzCmd({ action: 'zoomIn', speeds: { zoom: 0x78 } });
    const bytes = parseHex(cmd);
    expect(bytes[3]).toBe(0x10);
    expect(bytes[6]).toBe(0x70);
    expect(bytes[7]).toBe(checksum(bytes));
  });

  it('rounds and clamps speeds', () => {
    const cmd = createGb28181PtzCmd({
      action: 'left',
      speeds: { horizontal: 300.6, vertical: -5, zoom: 0xab },
    });
    const bytes = parseHex(cmd);
    expect(bytes[4]).toBe(255);
    expect(bytes[5]).toBe(0);
    expect(bytes[6]).toBe(0xa0);
  });

  it('uses the configured channel address', () => {
    const cmd = createGb28181PtzCmd({ action: 'stop', channel: 5 });
    const bytes = parseHex(cmd);
    expect(bytes[2]).toBe(5);
    expect(bytes[7]).toBe(checksum(bytes));
  });

  it('generates preset commands with a point index', () => {
    const cmd = createGb28181PtzCmd({ action: 'presetCall', presetPoint: 7 });
    const bytes = parseHex(cmd);
    expect(bytes[3]).toBe(0x82);
    expect(bytes[5]).toBe(7);
    expect(bytes[7]).toBe(checksum(bytes));
  });

  it('rejects preset actions without a point', () => {
    expect(() => createGb28181PtzCmd({ action: 'presetSet' })).toThrow('presetPoint');
  });

  it('rejects invalid channel and action values', () => {
    expect(() => createGb28181PtzCmd({ action: 'stop', channel: 256 })).toThrow('channel');
    expect(() => createGb28181PtzCmd({ action: 'unknown' as unknown as PtzCommand['action'] })).toThrow('Unsupported');
  });

  it('rejects inherited Object.prototype names as actions', () => {
    expect(() =>
      createGb28181PtzCmd({ action: 'toString' as unknown as PtzCommand['action'] }),
    ).toThrow('Unsupported');
    expect(() =>
      createGb28181PtzCmd({ action: 'constructor' as unknown as PtzCommand['action'] }),
    ).toThrow('Unsupported');
  });
});
