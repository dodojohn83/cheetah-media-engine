import { describe, expect, it } from 'vitest';
import { IntercomPacketizer, type IntercomPacket } from './intercom';
import type { AudioPacket } from './capture';

function makeAudioPacket(payload: number[], kind: 'alaw' | 'mulaw' = 'mulaw'): AudioPacket {
  return {
    kind,
    payload: new Uint8Array(payload),
    timestampMs: 1234,
    sampleRate: 8000,
    channels: 1,
  };
}

describe('IntercomPacketizer', () => {
  it('produces a 12-byte RTP header plus payload for G.711', () => {
    const packets: IntercomPacket[] = [];
    const packetizer = new IntercomPacketizer({
      payloadType: 8,
      ssrc: 0x12345678,
      onPacket: (p) => packets.push(p),
    });

    packetizer.push(makeAudioPacket([0, 1, 2, 3]));

    expect(packets.length).toBe(1);
    const p = packets[0]!;
    expect(p.payload.length).toBe(16);
    expect(p.payload[0]).toBe(0x80); // V=2
    expect(p.payload[1]).toBe(8); // payload type
    expect(p.sequence).toBe(0);
    expect(p.rtpTimestamp).toBe(0);
    expect(p.ssrc).toBe(0x12345678);
    expect(Array.from(p.payload.subarray(12))).toEqual([0, 1, 2, 3]);
  });

  it('increments sequence and RTP timestamp per packet', () => {
    const packets: IntercomPacket[] = [];
    const packetizer = new IntercomPacketizer({ onPacket: (p) => packets.push(p) });
    packetizer.push(makeAudioPacket(new Array(160).fill(0)));
    packetizer.push(makeAudioPacket(new Array(160).fill(0)));

    expect(packets[0]!.sequence).toBe(0);
    expect(packets[0]!.rtpTimestamp).toBe(0);
    expect(packets[1]!.sequence).toBe(1);
    expect(packets[1]!.rtpTimestamp).toBe(160);
  });

  it('resets sequence and timestamp', () => {
    const packetizer = new IntercomPacketizer();
    packetizer.push(makeAudioPacket([0]));
    expect(packetizer.currentSequence).toBe(1);
    packetizer.reset();
    expect(packetizer.currentSequence).toBe(0);
  });

  it('rejects invalid constructor options', () => {
    expect(() => new IntercomPacketizer({ payloadType: 128 })).toThrow();
    expect(() => new IntercomPacketizer({ payloadType: NaN })).toThrow();
    expect(() => new IntercomPacketizer({ ssrc: -1 })).toThrow();
  });

  it('rejects malformed audio packets', () => {
    const packetizer = new IntercomPacketizer();
    expect(() => packetizer.push({ kind: 'mulaw' } as unknown as AudioPacket)).toThrow();
  });
});
