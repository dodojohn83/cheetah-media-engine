/**
 * Pure TypeScript FLV demuxer + H.264/AAC → fMP4 remuxer for MSE.
 *
 * This mirrors `cheetah_container_flv::FlvToFmp4Transmuxer` for the main-thread
 * playback session so HTTP/WS-FLV works without waiting on a WASM module load.
 * The Rust implementation remains the authoritative path for wasm-bindgen and
 * fixture round-trip tests.
 */

import { concatUint8 } from './fmp4';

export interface FlvFmp4Segment {
  readonly init?: Uint8Array;
  readonly media?: Uint8Array;
  readonly sequence: number;
}

export interface FlvTrackInfo {
  readonly kind: 'video' | 'audio';
  readonly codec: 'h264' | 'h265' | 'aac';
  readonly width?: number;
  readonly height?: number;
  readonly sampleRate?: number;
  readonly channels?: number;
}

export type FlvTransmuxEvent =
  | { type: 'track'; track: FlvTrackInfo }
  | { type: 'segment'; segment: FlvFmp4Segment }
  | { type: 'error'; message: string };

const SOUND_RATES = [5500, 11025, 22050, 44100] as const;

function u32be(data: Uint8Array, offset: number): number {
  return (
    ((data[offset]! << 24) |
      (data[offset + 1]! << 16) |
      (data[offset + 2]! << 8) |
      data[offset + 3]!) >>>
    0
  );
}

function u24be(data: Uint8Array, offset: number): number {
  return (data[offset]! << 16) | (data[offset + 1]! << 8) | data[offset + 2]!;
}

function writeU16(out: number[], v: number): void {
  out.push((v >>> 8) & 0xff, v & 0xff);
}

function writeU32(out: number[], v: number): void {
  out.push((v >>> 24) & 0xff, (v >>> 16) & 0xff, (v >>> 8) & 0xff, v & 0xff);
}

function box(type: string, body: Uint8Array | number[]): Uint8Array {
  const payload = body instanceof Uint8Array ? body : new Uint8Array(body);
  const out = new Uint8Array(8 + payload.length);
  const dv = new DataView(out.buffer);
  dv.setUint32(0, out.length, false);
  out[4] = type.charCodeAt(0);
  out[5] = type.charCodeAt(1);
  out[6] = type.charCodeAt(2);
  out[7] = type.charCodeAt(3);
  out.set(payload, 8);
  return out;
}

function fullBox(type: string, version: number, flags: number, body: number[]): Uint8Array {
  const b = [(version & 0xff), (flags >>> 16) & 0xff, (flags >>> 8) & 0xff, flags & 0xff, ...body];
  return box(type, b);
}

function parseAvcCDimensions(avcc: Uint8Array): { width: number; height: number; codec: string } {
  // Minimal: profile/compat/level for codec string; dimensions defaulted if SPS parse fails.
  let codec = 'avc1.42001e';
  if (avcc.length >= 4) {
    const profile = avcc[1]!.toString(16).padStart(2, '0');
    const compat = avcc[2]!.toString(16).padStart(2, '0');
    const level = avcc[3]!.toString(16).padStart(2, '0');
    codec = `avc1.${profile}${compat}${level}`;
  }
  // Best-effort SPS parse for width/height (exp-golomb light path).
  let width = 640;
  let height = 360;
  try {
    if (avcc.length > 8 && (avcc[5]! & 0x1f) >= 1) {
      const spsLen = (avcc[6]! << 8) | avcc[7]!;
      if (spsLen > 0 && 8 + spsLen <= avcc.length) {
        const sps = avcc.subarray(8, 8 + spsLen);
        const dim = parseSpsSize(sps);
        if (dim) {
          width = dim.width;
          height = dim.height;
        }
      }
    }
  } catch {
    // keep defaults
  }
  return { width, height, codec };
}

function parseSpsSize(sps: Uint8Array): { width: number; height: number } | undefined {
  // Extremely simplified: scan for pic_width_in_mbs / pic_height after common header.
  // For robust production use the Rust bitstream crate; this covers baseline fixtures.
  if (sps.length < 4) return undefined;
  // Skip NAL header
  const data = sps.subarray(1);
  // profile_idc, constraint, level, then exp-golomb fields — use a tiny reader
  const r = new BitReader(data);
  r.readBits(8); // profile
  r.readBits(8); // constraints
  r.readBits(8); // level
  r.readUE(); // seq_parameter_set_id
  // For high profiles more fields exist; baseline/main path:
  const profileIdc = sps[1]!;
  if ([100, 110, 122, 244, 44, 83, 86, 118, 128, 138, 139, 134].includes(profileIdc)) {
    const chroma = r.readUE();
    if (chroma === 3) r.readBits(1);
    r.readUE();
    r.readUE();
    r.readBits(1);
    if (r.readBits(1)) {
      const n = chroma !== 3 ? 8 : 12;
      for (let i = 0; i < n; i++) {
        if (r.readBits(1)) {
          // scaling list skip approximate
          let last = 8;
          let next = 8;
          for (let j = 0; j < 16; j++) {
            if (next !== 0) {
              const delta = r.readSE();
              next = (last + delta + 256) % 256;
            }
            last = next === 0 ? last : next;
          }
        }
      }
    }
  }
  r.readUE(); // log2_max_frame_num
  const pocType = r.readUE();
  if (pocType === 0) r.readUE();
  else if (pocType === 1) {
    r.readBits(1);
    r.readSE();
    r.readSE();
    const n = r.readUE();
    for (let i = 0; i < n; i++) r.readSE();
  }
  r.readUE(); // max_num_ref_frames
  r.readBits(1);
  const widthMbs = r.readUE() + 1;
  const heightMap = r.readUE() + 1;
  const frameMbsOnly = r.readBits(1);
  if (!frameMbsOnly) r.readBits(1);
  r.readBits(1);
  let cropLeft = 0;
  let cropRight = 0;
  let cropTop = 0;
  let cropBottom = 0;
  if (r.readBits(1)) {
    cropLeft = r.readUE();
    cropRight = r.readUE();
    cropTop = r.readUE();
    cropBottom = r.readUE();
  }
  const width = widthMbs * 16 - (cropLeft + cropRight) * 2;
  const height = (2 - frameMbsOnly) * heightMap * 16 - (cropTop + cropBottom) * 2 * (2 - frameMbsOnly);
  if (width > 0 && height > 0 && width < 8192 && height < 8192) {
    return { width, height };
  }
  return undefined;
}

class BitReader {
  private bit = 0;
  constructor(private readonly data: Uint8Array) {}
  readBits(n: number): number {
    let v = 0;
    for (let i = 0; i < n; i++) {
      const byte = this.data[this.bit >> 3] ?? 0;
      const bit = (byte >> (7 - (this.bit & 7))) & 1;
      v = (v << 1) | bit;
      this.bit += 1;
    }
    return v;
  }
  readUE(): number {
    let zeros = 0;
    while (this.readBits(1) === 0 && zeros < 31) zeros += 1;
    if (zeros === 0) return 0;
    return (1 << zeros) - 1 + this.readBits(zeros);
  }
  readSE(): number {
    const v = this.readUE();
    const sign = ((v & 1) === 0 ? -1 : 1) as number;
    return sign * ((v + 1) >> 1);
  }
}

interface Sample {
  readonly trackId: number;
  readonly data: Uint8Array;
  readonly dts: number;
  readonly cts: number;
  readonly duration: number;
  readonly key: boolean;
}

interface VideoState {
  avcc: Uint8Array;
  width: number;
  height: number;
  codec: string;
  trackId: number;
  lastDts: number;
}

interface AudioState {
  asc: Uint8Array;
  sampleRate: number;
  channels: number;
  trackId: number;
  lastDts: number;
}

/**
 * Incremental FLV → fMP4 transmuxer (H.264 + AAC).
 */
export class FlvFmp4TransmuxerJs {
  private buffer: Uint8Array = new Uint8Array(0);
  private headerParsed = false;
  private expectPrevTagSize = true;
  private video: VideoState | undefined;
  private audio: AudioState | undefined;
  private videoSamples: Sample[] = [];
  private audioSamples: Sample[] = [];
  private sequence = 1;
  private initSent = false;
  private readonly pending: FlvFmp4Segment[] = [];
  private readonly tracks: FlvTrackInfo[] = [];
  private errored = false;
  private readonly maxBuffer = 64 * 1024 * 1024;
  private readonly maxSamples = 40;

  push(chunk: Uint8Array): void {
    if (this.errored || chunk.length === 0) return;
    if (this.buffer.length + chunk.length > this.maxBuffer) {
      this.errored = true;
      throw new Error('FLV buffer exceeded limit');
    }
    this.buffer = concatUint8([this.buffer, chunk]);
    this.parse();
  }

  finish(): void {
    if (this.errored) return;
    this.flush(true);
  }

  poll(): FlvFmp4Segment[] {
    const out = this.pending.splice(0, this.pending.length);
    return out;
  }

  getTracks(): readonly FlvTrackInfo[] {
    return this.tracks;
  }

  private parse(): void {
    let offset = 0;
    if (!this.headerParsed) {
      if (this.buffer.length < 9) return;
      if (
        this.buffer[0] !== 0x46 ||
        this.buffer[1] !== 0x4c ||
        this.buffer[2] !== 0x56
      ) {
        this.errored = true;
        throw new Error('Invalid FLV signature');
      }
      const dataOffset = u32be(this.buffer, 5);
      offset = Math.max(9, dataOffset);
      // Optional PreviousTagSize0
      if (this.buffer.length >= offset + 4 && u32be(this.buffer, offset) === 0) {
        offset += 4;
      }
      this.headerParsed = true;
      this.buffer = this.buffer.subarray(offset);
      offset = 0;
    }

    while (true) {
      if (this.buffer.length - offset < 11) break;
      const tagType = this.buffer[offset]!;
      const dataSize = u24be(this.buffer, offset + 1);
      const ts = u24be(this.buffer, offset + 4) | (this.buffer[offset + 7]! << 24);
      const total = 11 + dataSize + (this.expectPrevTagSize ? 4 : 0);
      if (this.buffer.length - offset < total) {
        // Live streams sometimes omit PreviousTagSize; try without.
        const totalNoPts = 11 + dataSize;
        if (this.buffer.length - offset < totalNoPts) break;
        this.expectPrevTagSize = false;
      }
      const body = this.buffer.subarray(offset + 11, offset + 11 + dataSize);
      const used = 11 + dataSize + (this.expectPrevTagSize ? 4 : 0);
      if (tagType === 8) {
        this.onAudio(body, ts >>> 0);
      } else if (tagType === 9) {
        this.onVideo(body, ts >>> 0);
      }
      offset += used;
      if (this.videoSamples.length + this.audioSamples.length >= this.maxSamples) {
        this.flush(false);
      }
    }
    if (offset > 0) {
      this.buffer = this.buffer.subarray(offset);
    }
  }

  private onVideo(body: Uint8Array, dts: number): void {
    if (body.length < 5) return;
    const frameType = body[0]! >> 4;
    const codecId = body[0]! & 0x0f;
    if (codecId !== 7 && codecId !== 12) return; // only AVC/HEVC
    const packetType = body[1]!;
    const ctsRaw = (body[2]! << 16) | (body[3]! << 8) | body[4]!;
    const cts = ctsRaw & 0x800000 ? ctsRaw | ~0xffffff : ctsRaw;
    const payload = body.subarray(5);

    if (packetType === 0) {
      // sequence header = avcC / hvcC
      if (codecId === 7) {
        const dim = parseAvcCDimensions(payload);
        this.video = {
          avcc: payload.slice(),
          width: dim.width,
          height: dim.height,
          codec: dim.codec,
          trackId: 1,
          lastDts: dts,
        };
        if (!this.tracks.some((t) => t.kind === 'video')) {
          this.tracks.push({
            kind: 'video',
            codec: 'h264',
            width: dim.width,
            height: dim.height,
          });
        }
      }
      return;
    }
    if (packetType !== 1 || !this.video) return;
    const duration = Math.max(1, dts - this.video.lastDts || 33);
    this.video.lastDts = dts;
    this.videoSamples.push({
      trackId: 1,
      data: payload.slice(),
      dts,
      cts,
      duration,
      key: frameType === 1,
    });
    if (frameType === 1 && this.videoSamples.length > 1) {
      this.flush(false);
    }
  }

  private onAudio(body: Uint8Array, dts: number): void {
    if (body.length < 2) return;
    const soundFormat = (body[0]! >> 4) & 0x0f;
    if (soundFormat !== 10) return; // AAC only for fMP4 path
    const packetType = body[1]!;
    const payload = body.subarray(2);
    const rateIdx = (body[0]! >> 2) & 0x03;
    const channels = (body[0]! & 0x01) === 1 ? 2 : 1;
    const sampleRate = SOUND_RATES[rateIdx] ?? 44100;

    if (packetType === 0) {
      this.audio = {
        asc: payload.slice(),
        sampleRate,
        channels,
        trackId: 2,
        lastDts: dts,
      };
      if (!this.tracks.some((t) => t.kind === 'audio')) {
        this.tracks.push({
          kind: 'audio',
          codec: 'aac',
          sampleRate,
          channels,
        });
      }
      return;
    }
    if (!this.audio) return;
    const duration = Math.max(1, Math.round((1024 * 1000) / this.audio.sampleRate));
    this.audio.lastDts = dts;
    this.audioSamples.push({
      trackId: 2,
      data: payload.slice(),
      dts,
      cts: 0,
      duration,
      key: true,
    });
  }

  private flush(force: boolean): void {
    if (!this.video && !this.audio) return;
    if (this.videoSamples.length === 0 && this.audioSamples.length === 0 && !force) return;

    const segs: FlvFmp4Segment[] = [];
    if (!this.initSent && (this.video || this.audio)) {
      const init = this.buildInit();
      this.initSent = true;
      segs.push({ init, sequence: this.sequence });
    }

    if (this.videoSamples.length > 0 || this.audioSamples.length > 0) {
      const media = this.buildMedia(this.videoSamples, this.audioSamples);
      segs.push({ media, sequence: this.sequence });
      this.sequence += 1;
      this.videoSamples = [];
      this.audioSamples = [];
    }

    for (const s of segs) this.pending.push(s);
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
    if (this.video) traks.push(this.buildVideoTrak(this.video));
    if (this.audio) traks.push(this.buildAudioTrak(this.audio));

    const mvhdBody: number[] = [];
    // version 0
    mvhdBody.push(0, 0, 0, 0);
    writeU32(mvhdBody, 0); // creation
    writeU32(mvhdBody, 0); // modification
    writeU32(mvhdBody, 1000); // timescale
    writeU32(mvhdBody, 0); // duration
    writeU32(mvhdBody, 0x00010000); // rate
    writeU16(mvhdBody, 0x0100); // volume
    writeU16(mvhdBody, 0);
    writeU32(mvhdBody, 0);
    writeU32(mvhdBody, 0);
    // matrix
    const matrix = [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000];
    for (const m of matrix) writeU32(mvhdBody, m);
    for (let i = 0; i < 6; i++) writeU32(mvhdBody, 0);
    writeU32(mvhdBody, 3); // next_track_ID

    const mvhd = fullBox('mvhd', 0, 0, mvhdBody);
    const trexParts: Uint8Array[] = [];
    if (this.video) {
      const b: number[] = [];
      writeU32(b, this.video.trackId);
      writeU32(b, 1);
      writeU32(b, 33);
      writeU32(b, 0);
      writeU32(b, 0);
      trexParts.push(fullBox('trex', 0, 0, b));
    }
    if (this.audio) {
      const b: number[] = [];
      writeU32(b, this.audio.trackId);
      writeU32(b, 1);
      writeU32(b, 1024);
      writeU32(b, 0);
      writeU32(b, 0);
      trexParts.push(fullBox('trex', 0, 0, b));
    }
    const mvex = box('mvex', concatUint8(trexParts));
    const moov = box('moov', concatUint8([mvhd, ...traks, mvex]));
    return concatUint8([ftyp, moov]);
  }

  private buildVideoTrak(v: VideoState): Uint8Array {
    const tkhdBody: number[] = [];
    tkhdBody.push(0, 0, 0, 7); // flags track enabled+in movie+preview
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, v.trackId);
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0); // duration
    writeU32(tkhdBody, 0);
    writeU32(tkhdBody, 0);
    writeU16(tkhdBody, 0);
    writeU16(tkhdBody, 0);
    const matrix = [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000];
    for (const m of matrix) writeU32(tkhdBody, m);
    writeU32(tkhdBody, v.width << 16);
    writeU32(tkhdBody, v.height << 16);
    const tkhd = fullBox('tkhd', 0, 7, tkhdBody.slice(4)); // version/flags already in fullBox

    // Rebuild tkhd properly with fullBox
    const tkhd2Body: number[] = [];
    writeU32(tkhd2Body, 0);
    writeU32(tkhd2Body, 0);
    writeU32(tkhd2Body, v.trackId);
    writeU32(tkhd2Body, 0);
    writeU32(tkhd2Body, 0);
    writeU32(tkhd2Body, 0);
    writeU32(tkhd2Body, 0);
    writeU16(tkhd2Body, 0);
    writeU16(tkhd2Body, 0);
    for (const m of matrix) writeU32(tkhd2Body, m);
    writeU32(tkhd2Body, v.width << 16);
    writeU32(tkhd2Body, v.height << 16);
    const tkhdBox = fullBox('tkhd', 0, 7, tkhd2Body);

    const mdhdBody: number[] = [];
    writeU32(mdhdBody, 0);
    writeU32(mdhdBody, 0);
    writeU32(mdhdBody, 1000);
    writeU32(mdhdBody, 0);
    writeU16(mdhdBody, 0x55c4); // und
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
    const dref = fullBox('dref', 0, 0, (() => {
      const b: number[] = [];
      writeU32(b, 1);
      const url = fullBox('url ', 0, 1, []);
      b.push(...url);
      return b;
    })());
    const dinf = box('dinf', dref);

    // stsd with avc1
    const avc1Body: number[] = [];
    for (let i = 0; i < 6; i++) avc1Body.push(0);
    writeU16(avc1Body, 1);
    writeU16(avc1Body, 0);
    writeU16(avc1Body, 0);
    for (let i = 0; i < 12; i++) avc1Body.push(0);
    writeU16(avc1Body, v.width);
    writeU16(avc1Body, v.height);
    writeU32(avc1Body, 0x00480000);
    writeU32(avc1Body, 0x00480000);
    writeU32(avc1Body, 0);
    writeU16(avc1Body, 1);
    for (let i = 0; i < 32; i++) avc1Body.push(0);
    writeU16(avc1Body, 0x0018);
    writeU16(avc1Body, 0xffff);
    const avccBox = box('avcC', v.avcc);
    avc1Body.push(...avccBox);
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
    void tkhd;
    return box('trak', concatUint8([tkhdBox, mdia]));
  }

  private buildAudioTrak(a: AudioState): Uint8Array {
    const tkhd2Body: number[] = [];
    writeU32(tkhd2Body, 0);
    writeU32(tkhd2Body, 0);
    writeU32(tkhd2Body, a.trackId);
    writeU32(tkhd2Body, 0);
    writeU32(tkhd2Body, 0);
    writeU32(tkhd2Body, 0);
    writeU32(tkhd2Body, 0);
    writeU16(tkhd2Body, 0);
    writeU16(tkhd2Body, 0x0100);
    const matrix = [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000];
    for (const m of matrix) writeU32(tkhd2Body, m);
    writeU32(tkhd2Body, 0);
    writeU32(tkhd2Body, 0);
    const tkhdBox = fullBox('tkhd', 0, 7, tkhd2Body);

    const mdhdBody: number[] = [];
    writeU32(mdhdBody, 0);
    writeU32(mdhdBody, 0);
    writeU32(mdhdBody, a.sampleRate);
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
    const dref = fullBox('dref', 0, 0, (() => {
      const b: number[] = [];
      writeU32(b, 1);
      const url = fullBox('url ', 0, 1, []);
      b.push(...url);
      return b;
    })());
    const dinf = box('dinf', dref);

    // esds with ASC
    const esds = this.buildEsds(a.asc);
    const mp4aBody: number[] = [];
    for (let i = 0; i < 6; i++) mp4aBody.push(0);
    writeU16(mp4aBody, 1);
    for (let i = 0; i < 8; i++) mp4aBody.push(0);
    writeU16(mp4aBody, a.channels);
    writeU16(mp4aBody, 16);
    writeU16(mp4aBody, 0);
    writeU16(mp4aBody, 0);
    writeU32(mp4aBody, (a.sampleRate << 16) >>> 0);
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
    return box('trak', concatUint8([tkhdBox, mdia]));
  }

  private buildEsds(asc: Uint8Array): Uint8Array {
    // Minimal ES_Descriptor / DecoderConfig / DecSpecificInfo
    const dsi = [0x05, asc.length, ...asc];
    const decoderConfig = [
      0x04,
      2 + 13 + dsi.length,
      0x40, // MPEG-4 AAC
      0x15, // stream type audio
      0, 0, 0, // buffer size
      0, 0, 0x1f, 0x40, // max bitrate
      0, 0, 0x1f, 0x40, // avg bitrate
      ...dsi,
    ];
    const es = [
      0x03,
      3 + decoderConfig.length + 3,
      0, 1, // ES_ID
      0, // flags
      ...decoderConfig,
      0x06, 1, 2, // SL config
    ];
    return fullBox('esds', 0, 0, es);
  }

  private buildMedia(video: Sample[], audio: Sample[]): Uint8Array {
    const trafs: Uint8Array[] = [];
    const mdatParts: Uint8Array[] = [];
    let dataOffsetBase = 8; // moof size filled later; use absolute calculation

    // First pass: compute sizes
    const trackSamples: { trackId: number; samples: Sample[]; timescale: number }[] = [];
    if (video.length && this.video) {
      trackSamples.push({ trackId: 1, samples: video, timescale: 1000 });
    }
    if (audio.length && this.audio) {
      trackSamples.push({ trackId: 2, samples: audio, timescale: this.audio.sampleRate });
    }

    // Build moof with provisional base data offset; fix with free box approach:
    // compute moof size then set data_offset = moofSize + 8
    const moofWithoutSize = this.buildMoof(trackSamples, 0);
    const moofSize = moofWithoutSize.length;
    const moof = this.buildMoof(trackSamples, moofSize + 8);

    for (const t of trackSamples) {
      for (const s of t.samples) mdatParts.push(s.data);
    }
    const mdat = box('mdat', concatUint8(mdatParts));
    void dataOffsetBase;
    void trafs;
    return concatUint8([moof, mdat]);
  }

  private buildMoof(
    tracks: { trackId: number; samples: Sample[]; timescale: number }[],
    dataOffset: number,
  ): Uint8Array {
    const mfhdBody: number[] = [];
    writeU32(mfhdBody, this.sequence);
    const mfhd = fullBox('mfhd', 0, 0, mfhdBody);

    const trafs: Uint8Array[] = [];
    let runningOffset = dataOffset;
    for (const t of tracks) {
      const tfhdBody: number[] = [];
      writeU32(tfhdBody, t.trackId);
      // flags: default-base-is-moof | sample-description-index-present
      const tfhd = fullBox('tfhd', 0, 0x020000, tfhdBody);

      const tfdtBody: number[] = [];
      writeU32(tfdtBody, t.samples[0]?.dts ?? 0);
      const tfdt = fullBox('tfdt', 0, 0, tfdtBody);

      // trun
      const trunBody: number[] = [];
      writeU32(trunBody, t.samples.length);
      writeU32(trunBody, runningOffset); // data_offset
      // flags: data-offset-present | sample-duration | sample-size | sample-flags | sample-composition-time-offsets
      for (const s of t.samples) {
        writeU32(trunBody, s.duration);
        writeU32(trunBody, s.data.length);
        // sample flags: keyframe = 0x02000000 else 0x01010000
        writeU32(trunBody, s.key ? 0x02000000 : 0x01010000);
        writeU32(trunBody, s.cts >>> 0);
        runningOffset += s.data.length;
      }
      const trun = fullBox('trun', 0, 0x000f01, trunBody);
      trafs.push(box('traf', concatUint8([tfhd, tfdt, trun])));
    }
    return box('moof', concatUint8([mfhd, ...trafs]));
  }
}
