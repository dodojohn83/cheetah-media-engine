import { describe, it, expect } from 'vitest';
import { TsFmp4TransmuxerJs } from './ts-transmux';

function makePacket(pid: number, payload: Uint8Array, pusi = false, cc = 0): Uint8Array {
  const pkt = new Uint8Array(188).fill(0xff);
  pkt[0] = 0x47;
  pkt[1] = ((pusi ? 0x40 : 0x00) | ((pid >> 8) & 0x1f));
  pkt[2] = pid & 0xff;

  if (payload.length <= 184) {
    pkt[3] = 0x10 | (cc & 0x0f);
    pkt.set(payload, 4);
  } else if (payload.length <= 183) {
    const adaptLen = 183 - payload.length;
    pkt[3] = 0x30 | (cc & 0x0f);
    pkt[4] = adaptLen;
    pkt[5] = 0x00;
    for (let i = 6; i < 6 + adaptLen - 1; i += 1) pkt[i] = 0xff;
    pkt.set(payload, 5 + adaptLen);
  } else {
    throw new Error('TS payload too large');
  }
  return pkt;
}

function concatUint8(parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((sum, p) => sum + p.length, 0);
  const out = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    out.set(p, off);
    off += p.length;
  }
  return out;
}

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

  it('derives video track dimensions from the H.264 SPS instead of using hard-coded 640x360', () => {
    // Minimal baseline SPS for 1280x720 and a dummy PPS.
    const sps = new Uint8Array([0x67, 0x42, 0xc0, 0x1f, 0xf4, 0x02, 0x80, 0x2d, 0xc8]);
    const pps = new Uint8Array([0x68, 0xce, 0x3c, 0x80]);
    const accessUnit = concatUint8([
      new Uint8Array([0x00, 0x00, 0x00, 0x01]),
      sps,
      new Uint8Array([0x00, 0x00, 0x00, 0x01]),
      pps,
    ]);
    const pesHeader = new Uint8Array([
      0x00, 0x00, 0x01, 0xe0, 0x00, 0x00, 0x84, 0x80, 0x05, 0x21, 0x00, 0x01, 0x00, 0x01,
    ]);
    const videoPayload = concatUint8([pesHeader, accessUnit]);

    const pat = new Uint8Array(184).fill(0xff);
    pat[0] = 0x00; // pointer
    pat[1] = 0x00; // table_id
    pat[2] = 0xb0;
    pat[3] = 0x0d; // section_length
    pat[4] = 0x00;
    pat[5] = 0x01; // transport_stream_id
    pat[6] = 0xc1;
    pat[7] = 0x00;
    pat[8] = 0x00; // section numbers
    pat[9] = 0x00;
    pat[10] = 0x01; // program 1
    pat[11] = 0xe1;
    pat[12] = 0x00; // PMT PID 0x100

    const pmt = new Uint8Array(184).fill(0xff);
    pmt[0] = 0x00; // pointer
    pmt[1] = 0x02; // table_id
    pmt[2] = 0xb0;
    pmt[3] = 0x12; // section_length
    pmt[4] = 0x00;
    pmt[5] = 0x01; // program_number
    pmt[6] = 0xc1;
    pmt[7] = 0x00;
    pmt[8] = 0x00; // section numbers
    pmt[9] = 0xe1;
    pmt[10] = 0x00; // PCR PID
    pmt[11] = 0xf0;
    pmt[12] = 0x00; // program_info_length
    pmt[13] = 0x1b; // stream_type H.264
    pmt[14] = 0xe1;
    pmt[15] = 0x01; // elementary_PID 0x101
    pmt[16] = 0xf0;
    pmt[17] = 0x00; // ES_info_length

    const transmuxer = new TsFmp4TransmuxerJs();
    transmuxer.push(
      concatUint8([
        makePacket(0x0000, pat, true, 0),
        makePacket(0x0100, pmt, true, 0),
        makePacket(0x0101, videoPayload, true, 0),
      ]),
    );
    transmuxer.finish();

    const tracks = transmuxer.getTracks();
    const video = tracks.find((t) => t.kind === 'video');
    expect(video).toBeDefined();
    expect(video!.width).toBe(1280);
    expect(video!.height).toBe(720);
  });
});
