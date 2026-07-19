/**
 * Minimal MPEG-TS demuxer + H.264/AAC → fMP4 remuxer for HLS-TS.
 *
 * Complements `cheetah_container_mpegts::TsToFmp4Transmuxer` (Rust/WASM) for the
 * main-thread `PlaybackSession` without requiring a WASM module load.
 */

import { concatUint8 } from './fmp4';

export interface TsFmp4Segment {
  readonly init?: Uint8Array;
  readonly media?: Uint8Array;
  readonly sequence: number;
}

export interface TsTrackInfo {
  readonly kind: 'video' | 'audio';
  readonly codec: 'h264' | 'aac';
  readonly width?: number;
  readonly height?: number;
  readonly sampleRate?: number;
  readonly channels?: number;
}

const PACKET_SIZE = 188;

function writeU16(out: number[], v: number): void {
  out.push((v >>> 8) & 0xff, v & 0xff);
}
function writeU32(out: number[], v: number): void {
  out.push((v >>> 24) & 0xff, (v >>> 16) & 0xff, (v >>> 8) & 0xff, v & 0xff);
}
function box(type: string, body: Uint8Array | number[]): Uint8Array {
  const payload = body instanceof Uint8Array ? body : new Uint8Array(body);
  const out = new Uint8Array(8 + payload.length);
  new DataView(out.buffer).setUint32(0, out.length, false);
  out[4] = type.charCodeAt(0);
  out[5] = type.charCodeAt(1);
  out[6] = type.charCodeAt(2);
  out[7] = type.charCodeAt(3);
  out.set(payload, 8);
  return out;
}
function fullBox(type: string, version: number, flags: number, body: number[]): Uint8Array {
  return box(type, [(version & 0xff), (flags >>> 16) & 0xff, (flags >>> 8) & 0xff, flags & 0xff, ...body]);
}

function findStartCode(data: Uint8Array, from: number): { pos: number; size: number } | undefined {
  for (let i = from; i + 3 <= data.length; i++) {
    if (data[i] === 0 && data[i + 1] === 0 && data[i + 2] === 1) {
      return { pos: i, size: 3 };
    }
    if (i + 4 <= data.length && data[i] === 0 && data[i + 1] === 0 && data[i + 2] === 0 && data[i + 3] === 1) {
      return { pos: i, size: 4 };
    }
  }
  return undefined;
}

function annexbToAvcc(data: Uint8Array): Uint8Array {
  const nals: Uint8Array[] = [];
  let from = 0;
  while (from < data.length) {
    const sc = findStartCode(data, from);
    if (!sc) break;
    const start = sc.pos + sc.size;
    const next = findStartCode(data, start);
    const end = next ? next.pos : data.length;
    if (end > start) nals.push(data.subarray(start, end));
    from = end;
  }
  const parts: Uint8Array[] = [];
  for (const nal of nals) {
    const len = new Uint8Array(4);
    new DataView(len.buffer).setUint32(0, nal.length, false);
    parts.push(len, nal);
  }
  return concatUint8(parts);
}

function extractSpsPps(annexb: Uint8Array): { sps: Uint8Array[]; pps: Uint8Array[]; key: boolean } {
  const sps: Uint8Array[] = [];
  const pps: Uint8Array[] = [];
  let key = false;
  const avccLike = annexbToAvcc(annexb);
  let off = 0;
  while (off + 4 <= avccLike.length) {
    const len = new DataView(avccLike.buffer, avccLike.byteOffset + off, 4).getUint32(0, false);
    off += 4;
    if (off + len > avccLike.length) break;
    const nal = avccLike.subarray(off, off + len);
    off += len;
    if (nal.length === 0) continue;
    const t = nal[0]! & 0x1f;
    if (t === 7) sps.push(nal.slice());
    else if (t === 8) pps.push(nal.slice());
    else if (t === 5) key = true;
  }
  return { sps, pps, key };
}

function buildAvcC(sps: Uint8Array[], pps: Uint8Array[]): Uint8Array {
  const first = sps[0]!;
  const out: number[] = [
    1,
    first[1]!,
    first[2]!,
    first[3]!,
    0xff,
    0xe0 | (sps.length & 0x1f),
  ];
  for (const s of sps) {
    writeU16(out, s.length);
    out.push(...s);
  }
  out.push(pps.length & 0xff);
  for (const p of pps) {
    writeU16(out, p.length);
    out.push(...p);
  }
  return new Uint8Array(out);
}

interface Sample {
  trackId: number;
  data: Uint8Array;
  dts: number;
  cts: number;
  duration: number;
  key: boolean;
}

/**
 * Incremental MPEG-TS → fMP4 for H.264 (+ optional AAC ADTS in stream type 0x0f).
 */
export class TsFmp4TransmuxerJs {
  private buffer = new Uint8Array(0);
  private pmtPids = new Set<number>();
  private videoPid: number | undefined;
  private audioPid: number | undefined;
  private pesBuffers = new Map<number, Uint8Array>();
  private videoSamples: Sample[] = [];
  private audioSamples: Sample[] = [];
  private avcc: Uint8Array | undefined;
  private width = 640;
  private height = 360;
  private audioAsc: Uint8Array | undefined;
  private sampleRate = 44100;
  private channels = 2;
  private sequence = 1;
  private initSent = false;
  private pending: TsFmp4Segment[] = [];
  private tracks: TsTrackInfo[] = [];
  private lastVideoDts = 0;
  private lastAudioDts = 0;
  private readonly maxBuffer = 32 * 1024 * 1024;

  push(chunk: Uint8Array): void {
    if (chunk.length === 0) return;
    if (this.buffer.length + chunk.length > this.maxBuffer) {
      throw new Error('TS buffer exceeded limit');
    }
    this.buffer = concatUint8([this.buffer, chunk]);
    this.parsePackets();
  }

  finish(): void {
    // Flush partial PES
    for (const [pid, buf] of this.pesBuffers) {
      if (buf.length > 0) this.handlePes(pid, buf);
    }
    this.pesBuffers.clear();
    this.flush(true);
  }

  poll(): TsFmp4Segment[] {
    return this.pending.splice(0, this.pending.length);
  }

  getTracks(): readonly TsTrackInfo[] {
    return this.tracks;
  }

  private parsePackets(): void {
    // Resync to 0x47
    let start = 0;
    while (start < this.buffer.length && this.buffer[start] !== 0x47) start += 1;
    if (start > 0) this.buffer = this.buffer.subarray(start);

    let offset = 0;
    while (offset + PACKET_SIZE <= this.buffer.length) {
      if (this.buffer[offset] !== 0x47) {
        offset += 1;
        continue;
      }
      const pkt = this.buffer.subarray(offset, offset + PACKET_SIZE);
      this.processPacket(pkt);
      offset += PACKET_SIZE;
    }
    if (offset > 0) this.buffer = this.buffer.subarray(offset);
  }

  private processPacket(pkt: Uint8Array): void {
    const pid = ((pkt[1]! & 0x1f) << 8) | pkt[2]!;
    const pusi = (pkt[1]! & 0x40) !== 0;
    const afc = (pkt[3]! >> 4) & 0x03;
    let payloadOffset = 4;
    if (afc === 2 || afc === 3) {
      const afl = pkt[4]!;
      payloadOffset = 5 + afl;
    }
    if (afc === 0 || afc === 2 || payloadOffset >= PACKET_SIZE) return;
    const payload = pkt.subarray(payloadOffset, PACKET_SIZE);

    if (pid === 0) {
      this.parsePat(payload, pusi);
      return;
    }
    if (this.pmtPids.has(pid)) {
      this.parsePmt(payload, pusi);
      return;
    }
    if (pid === this.videoPid || pid === this.audioPid) {
      this.feedPes(pid, payload, pusi);
    }
  }

  private parsePat(payload: Uint8Array, pusi: boolean): void {
    let data = payload;
    if (pusi) {
      const pointer = data[0] ?? 0;
      data = data.subarray(1 + pointer);
    }
    if (data.length < 8 || data[0] !== 0x00) return;
    const sectionLength = ((data[1]! & 0x0f) << 8) | data[2]!;
    const end = Math.min(data.length, 3 + sectionLength - 4);
    for (let i = 8; i + 4 <= end; i += 4) {
      const program = (data[i]! << 8) | data[i + 1]!;
      const pmtPid = ((data[i + 2]! & 0x1f) << 8) | data[i + 3]!;
      if (program !== 0) this.pmtPids.add(pmtPid);
    }
  }

  private parsePmt(payload: Uint8Array, pusi: boolean): void {
    let data = payload;
    if (pusi) {
      const pointer = data[0] ?? 0;
      data = data.subarray(1 + pointer);
    }
    if (data.length < 12 || data[0] !== 0x02) return;
    const sectionLength = ((data[1]! & 0x0f) << 8) | data[2]!;
    const programInfoLength = ((data[10]! & 0x0f) << 8) | data[11]!;
    let i = 12 + programInfoLength;
    const end = Math.min(data.length, 3 + sectionLength - 4);
    while (i + 5 <= end) {
      const streamType = data[i]!;
      const elementaryPid = ((data[i + 1]! & 0x1f) << 8) | data[i + 2]!;
      const esInfoLength = ((data[i + 3]! & 0x0f) << 8) | data[i + 4]!;
      if (streamType === 0x1b && this.videoPid === undefined) {
        this.videoPid = elementaryPid;
      } else if (streamType === 0x0f && this.audioPid === undefined) {
        this.audioPid = elementaryPid;
      }
      i += 5 + esInfoLength;
    }
  }

  private feedPes(pid: number, payload: Uint8Array, pusi: boolean): void {
    if (pusi) {
      const prev = this.pesBuffers.get(pid);
      if (prev && prev.length > 0) this.handlePes(pid, prev);
      this.pesBuffers.set(pid, payload.slice());
    } else {
      const prev = this.pesBuffers.get(pid) ?? new Uint8Array(0);
      this.pesBuffers.set(pid, concatUint8([prev, payload]));
    }
  }

  private handlePes(pid: number, data: Uint8Array): void {
    if (data.length < 9) return;
    if (data[0] !== 0 || data[1] !== 0 || data[2] !== 1) return;
    const headerDataLength = data[8]!;
    const payloadStart = 9 + headerDataLength;
    if (payloadStart > data.length) return;
    const ptsDtsFlags = (data[7]! >> 6) & 0x03;
    let pts = 0;
    if (ptsDtsFlags >= 2 && headerDataLength >= 5) {
      pts = this.readTs(data, 9);
    }
    const es = data.subarray(payloadStart);

    if (pid === this.videoPid) {
      this.onVideoAu(es, pts);
    } else if (pid === this.audioPid) {
      this.onAudioAu(es, pts);
    }
  }

  private readTs(data: Uint8Array, offset: number): number {
    const b0 = data[offset]!;
    const b1 = data[offset + 1]!;
    const b2 = data[offset + 2]!;
    const b3 = data[offset + 3]!;
    const b4 = data[offset + 4]!;
    return (
      ((b0 & 0x0e) << 29) |
      (b1 << 22) |
      ((b2 & 0xfe) << 14) |
      (b3 << 7) |
      ((b4 & 0xfe) >> 1)
    ) >>> 0;
  }

  private onVideoAu(es: Uint8Array, pts90k: number): void {
    const { sps, pps, key } = extractSpsPps(es);
    if (sps.length && pps.length && !this.avcc) {
      this.avcc = buildAvcC(sps, pps);
      if (!this.tracks.some((t) => t.kind === 'video')) {
        this.tracks.push({ kind: 'video', codec: 'h264', width: this.width, height: this.height });
      }
    }
    if (!this.avcc) return;
    const avccPayload = annexbToAvcc(es);
    // Strip parameter set NALs from sample when present (keep VCL only) — optional; include all for simplicity.
    const dts = pts90k || this.lastVideoDts + 3000;
    const duration = Math.max(1, dts - this.lastVideoDts || 3000);
    this.lastVideoDts = dts;
    this.videoSamples.push({
      trackId: 1,
      data: avccPayload,
      dts,
      cts: 0,
      duration,
      key,
    });
    if (key && this.videoSamples.length > 1) this.flush(false);
  }

  private onAudioAu(es: Uint8Array, pts90k: number): void {
    // Split ADTS frames
    let offset = 0;
    while (offset + 7 <= es.length) {
      if (es[offset] !== 0xff || (es[offset + 1]! & 0xf0) !== 0xf0) {
        offset += 1;
        continue;
      }
      const protectionAbsent = (es[offset + 1]! & 0x01) !== 0;
      const headerSize = protectionAbsent ? 7 : 9;
      const frameLength =
        ((es[offset + 3]! & 0x03) << 11) | (es[offset + 4]! << 3) | ((es[offset + 5]! & 0xe0) >> 5);
      if (frameLength < headerSize || offset + frameLength > es.length) break;
      const profile = ((es[offset + 2]! >> 6) & 0x03) + 1;
      const sfIndex = (es[offset + 2]! >> 2) & 0x0f;
      const channelConfig = ((es[offset + 2]! & 0x01) << 2) | ((es[offset + 3]! >> 6) & 0x03);
      const rates = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];
      this.sampleRate = rates[sfIndex] ?? 44100;
      this.channels = channelConfig || 2;
      if (!this.audioAsc) {
        const aot = profile & 0x1f;
        const b0 = ((aot << 3) | ((sfIndex >> 1) & 0x07)) & 0xff;
        const b1 = (((sfIndex & 0x01) << 7) | ((channelConfig & 0x0f) << 3)) & 0xff;
        this.audioAsc = new Uint8Array([b0, b1]);
        if (!this.tracks.some((t) => t.kind === 'audio')) {
          this.tracks.push({
            kind: 'audio',
            codec: 'aac',
            sampleRate: this.sampleRate,
            channels: this.channels,
          });
        }
      }
      const raw = es.subarray(offset + headerSize, offset + frameLength);
      const dts = pts90k || this.lastAudioDts + 1024;
      const duration = Math.max(1, Math.round((1024 * 90000) / this.sampleRate));
      this.lastAudioDts = dts;
      this.audioSamples.push({
        trackId: 2,
        data: raw.slice(),
        dts,
        cts: 0,
        duration,
        key: true,
      });
      offset += frameLength;
    }
  }

  private flush(force: boolean): void {
    if (!this.avcc && this.videoSamples.length === 0 && this.audioSamples.length === 0) return;
    if (this.videoSamples.length === 0 && this.audioSamples.length === 0 && !force) return;

    if (!this.initSent && this.avcc) {
      this.pending.push({ init: this.buildInit(), sequence: this.sequence });
      this.initSent = true;
    }
    if (this.videoSamples.length > 0 || this.audioSamples.length > 0) {
      if (!this.initSent) return; // wait for video config
      this.pending.push({
        media: this.buildMedia(this.videoSamples, this.audioSamples),
        sequence: this.sequence,
      });
      this.sequence += 1;
      this.videoSamples = [];
      this.audioSamples = [];
    }
  }

  private buildInit(): Uint8Array {
    const ftyp = box('ftyp', [
      ...[...('isom')].map((c) => c.charCodeAt(0)),
      0, 0, 0, 1,
      ...[...('isom')].map((c) => c.charCodeAt(0)),
      ...[...('iso6')].map((c) => c.charCodeAt(0)),
      ...[...('avc1')].map((c) => c.charCodeAt(0)),
      ...[...('mp41')].map((c) => c.charCodeAt(0)),
    ]);
    const traks: Uint8Array[] = [];
    if (this.avcc) traks.push(this.buildVideoTrak());
    if (this.audioAsc) traks.push(this.buildAudioTrak());

    const mvhdBody: number[] = [];
    writeU32(mvhdBody, 0);
    writeU32(mvhdBody, 0);
    writeU32(mvhdBody, 90_000);
    writeU32(mvhdBody, 0);
    writeU32(mvhdBody, 0x00010000);
    writeU16(mvhdBody, 0x0100);
    writeU16(mvhdBody, 0);
    writeU32(mvhdBody, 0);
    writeU32(mvhdBody, 0);
    const matrix = [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000];
    for (const m of matrix) writeU32(mvhdBody, m);
    for (let i = 0; i < 6; i++) writeU32(mvhdBody, 0);
    writeU32(mvhdBody, 3);
    const mvhd = fullBox('mvhd', 0, 0, mvhdBody);

    const trexs: Uint8Array[] = [];
    if (this.avcc) {
      const b: number[] = [];
      writeU32(b, 1);
      writeU32(b, 1);
      writeU32(b, 3000);
      writeU32(b, 0);
      writeU32(b, 0);
      trexs.push(fullBox('trex', 0, 0, b));
    }
    if (this.audioAsc) {
      const b: number[] = [];
      writeU32(b, 2);
      writeU32(b, 1);
      writeU32(b, 1024);
      writeU32(b, 0);
      writeU32(b, 0);
      trexs.push(fullBox('trex', 0, 0, b));
    }
    const mvex = box('mvex', concatUint8(trexs));
    const moov = box('moov', concatUint8([mvhd, ...traks, mvex]));
    return concatUint8([ftyp, moov]);
  }

  private buildVideoTrak(): Uint8Array {
    const tkhdBody: number[] = [];
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 1);
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0);
    writeU16(tkhdBody, 0);
    writeU16(tkhdBody, 0);
    const matrix = [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000];
    for (const m of matrix) writeU32(tkhdBody, m);
    writeU32(tkhdBody, this.width << 16);
    writeU32(tkhdBody, this.height << 16);
    const tkhd = fullBox('tkhd', 0, 7, tkhdBody);

    const mdhdBody: number[] = [];
    writeU32(mdhdBody, 0);
    writeU32(mdhdBody, 0);
    writeU32(mdhdBody, 90_000);
    writeU32(mdhdBody, 0);
    writeU16(mdhdBody, 0x55c4);
    writeU16(mdhdBody, 0);
    const mdhd = fullBox('mdhd', 0, 0, mdhdBody);
    const hdlrBody: number[] = [];
    writeU32(hdlrBody, 0);
    hdlrBody.push(...[...('vide')].map((c) => c.charCodeAt(0)));
    writeU32(hdlrBody, 0);
    writeU32(hdlrBody, 0);
    writeU32(hdlrBody, 0);
    hdlrBody.push(...[...('VideoHandler')].map((c) => c.charCodeAt(0)), 0);
    const hdlr = fullBox('hdlr', 0, 0, hdlrBody);
    const vmhd = fullBox('vmhd', 0, 1, [0, 0, 0, 0, 0, 0, 0, 0]);
    const url = fullBox('url ', 0, 1, []);
    const drefBody: number[] = [];
    writeU32(drefBody, 1);
    drefBody.push(...url);
    const dref = fullBox('dref', 0, 0, drefBody);
    const dinf = box('dinf', dref);

    const avc1Body: number[] = [];
    for (let i = 0; i < 6; i++) avc1Body.push(0);
    writeU16(avc1Body, 1);
    writeU16(avc1Body, 0);
    writeU16(avc1Body, 0);
    for (let i = 0; i < 12; i++) avc1Body.push(0);
    writeU16(avc1Body, this.width);
    writeU16(avc1Body, this.height);
    writeU32(avc1Body, 0x00480000);
    writeU32(avc1Body, 0x00480000);
    writeU32(avc1Body, 0);
    writeU16(avc1Body, 1);
    for (let i = 0; i < 32; i++) avc1Body.push(0);
    writeU16(avc1Body, 0x0018);
    writeU16(avc1Body, 0xffff);
    avc1Body.push(...box('avcC', this.avcc!));
    const avc1 = box('avc1', avc1Body);
    const stsdBody: number[] = [];
    writeU32(stsdBody, 1);
    stsdBody.push(...avc1);
    const stsd = fullBox('stsd', 0, 0, stsdBody);
    const stts = fullBox('stts', 0, 0, [0, 0, 0, 0]);
    const stsc = fullBox('stsc', 0, 0, [0, 0, 0, 0]);
    const stsz = fullBox('stsz', 0, 0, [0, 0, 0, 0, 0, 0, 0, 0]);
    const stco = fullBox('stco', 0, 0, [0, 0, 0, 0]);
    const stbl = box('stbl', concatUint8([stsd, stts, stsc, stsz, stco]));
    const minf = box('minf', concatUint8([vmhd, dinf, stbl]));
    const mdia = box('mdia', concatUint8([mdhd, hdlr, minf]));
    return box('trak', concatUint8([tkhd, mdia]));
  }

  private buildAudioTrak(): Uint8Array {
    const tkhdBody: number[] = [];
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 2);
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0);
    writeU16(tkhdBody, 0);
    writeU16(tkhdBody, 0x0100);
    const matrix = [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000];
    for (const m of matrix) writeU32(tkhdBody, m);
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0);
    const tkhd = fullBox('tkhd', 0, 7, tkhdBody);

    const mdhdBody: number[] = [];
    writeU32(mdhdBody, 0);
    writeU32(mdhdBody, 0);
    writeU32(mdhdBody, this.sampleRate);
    writeU32(mdhdBody, 0);
    writeU16(mdhdBody, 0x55c4);
    writeU16(mdhdBody, 0);
    const mdhd = fullBox('mdhd', 0, 0, mdhdBody);
    const hdlrBody: number[] = [];
    writeU32(hdlrBody, 0);
    hdlrBody.push(...[...('soun')].map((c) => c.charCodeAt(0)));
    writeU32(hdlrBody, 0);
    writeU32(hdlrBody, 0);
    writeU32(hdlrBody, 0);
    hdlrBody.push(...[...('SoundHandler')].map((c) => c.charCodeAt(0)), 0);
    const hdlr = fullBox('hdlr', 0, 0, hdlrBody);
    const smhd = fullBox('smhd', 0, 0, [0, 0, 0, 0]);
    const url = fullBox('url ', 0, 1, []);
    const drefBody: number[] = [];
    writeU32(drefBody, 1);
    drefBody.push(...url);
    const dref = fullBox('dref', 0, 0, drefBody);
    const dinf = box('dinf', dref);

    const dsi = [0x05, this.audioAsc!.length, ...this.audioAsc!];
    const decoderConfig = [
      0x04,
      2 + 13 + dsi.length,
      0x40,
      0x15,
      0, 0, 0,
      0, 0, 0x1f, 0x40,
      0, 0, 0x1f, 0x40,
      ...dsi,
    ];
    const es = [
      0x03,
      3 + decoderConfig.length + 3,
      0, 2,
      0,
      ...decoderConfig,
      0x06, 1, 2,
    ];
    const esds = fullBox('esds', 0, 0, es);
    const mp4aBody: number[] = [];
    for (let i = 0; i < 6; i++) mp4aBody.push(0);
    writeU16(mp4aBody, 1);
    for (let i = 0; i < 8; i++) mp4aBody.push(0);
    writeU16(mp4aBody, this.channels);
    writeU16(mp4aBody, 16);
    writeU16(mp4aBody, 0);
    writeU16(mp4aBody, 0);
    writeU32(mp4aBody, (this.sampleRate << 16) >>> 0);
    mp4aBody.push(...esds);
    const mp4a = box('mp4a', mp4aBody);
    const stsdBody: number[] = [];
    writeU32(stsdBody, 1);
    stsdBody.push(...mp4a);
    const stsd = fullBox('stsd', 0, 0, stsdBody);
    const stts = fullBox('stts', 0, 0, [0, 0, 0, 0]);
    const stsc = fullBox('stsc', 0, 0, [0, 0, 0, 0]);
    const stsz = fullBox('stsz', 0, 0, [0, 0, 0, 0, 0, 0, 0, 0]);
    const stco = fullBox('stco', 0, 0, [0, 0, 0, 0]);
    const stbl = box('stbl', concatUint8([stsd, stts, stsc, stsz, stco]));
    const minf = box('minf', concatUint8([smhd, dinf, stbl]));
    const mdia = box('mdia', concatUint8([mdhd, hdlr, minf]));
    return box('trak', concatUint8([tkhd, mdia]));
  }

  private buildMedia(video: Sample[], audio: Sample[]): Uint8Array {
    const tracks: { trackId: number; samples: Sample[] }[] = [];
    if (video.length) tracks.push({ trackId: 1, samples: video });
    if (audio.length) tracks.push({ trackId: 2, samples: audio });
    const moof = this.buildMoof(tracks, 0);
    const moofFixed = this.buildMoof(tracks, moof.length + 8);
    const mdatParts: Uint8Array[] = [];
    for (const t of tracks) for (const s of t.samples) mdatParts.push(s.data);
    return concatUint8([moofFixed, box('mdat', concatUint8(mdatParts))]);
  }

  private buildMoof(tracks: { trackId: number; samples: Sample[] }[], dataOffset: number): Uint8Array {
    const mfhdBody: number[] = [];
    writeU32(mfhdBody, this.sequence);
    const mfhd = fullBox('mfhd', 0, 0, mfhdBody);
    const trafs: Uint8Array[] = [];
    let running = dataOffset;
    for (const t of tracks) {
      const tfhdBody: number[] = [];
      writeU32(tfhdBody, t.trackId);
      const tfhd = fullBox('tfhd', 0, 0x020000, tfhdBody);
      const tfdtBody: number[] = [];
      writeU32(tfdtBody, t.samples[0]?.dts ?? 0);
      const tfdt = fullBox('tfdt', 0, 0, tfdtBody);
      const trunBody: number[] = [];
      writeU32(trunBody, t.samples.length);
      writeU32(trunBody, running);
      for (const s of t.samples) {
        writeU32(trunBody, s.duration);
        writeU32(trunBody, s.data.length);
        writeU32(trunBody, s.key ? 0x02000000 : 0x01010000);
        writeU32(trunBody, s.cts >>> 0);
        running += s.data.length;
      }
      const trun = fullBox('trun', 0, 0x000f01, trunBody);
      trafs.push(box('traf', concatUint8([tfhd, tfdt, trun])));
    }
    return box('moof', concatUint8([mfhd, ...trafs]));
  }
}
