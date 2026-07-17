/**
 * Small G.711 A-law and mu-law encoder for the Web Audio capture pipeline.
 *
 * The implementation mirrors the CCITT reference algorithms and is safe for
 * 8 kHz voice intercom: no floating-point tables, just integer arithmetic on
 * clamped 16-bit PCM values.
 */

export type G711Kind = 'alaw' | 'mulaw';

const A_LAW_SEG_AEND = [0x1f, 0x3f, 0x7f, 0xff, 0x1ff, 0x3ff, 0x7ff, 0xfff];

const MU_LAW_EXP_LUT = new Uint8Array(256);
for (let i = 0; i < 256; i += 1) {
  MU_LAW_EXP_LUT[i] = i === 0 ? 0 : 31 - Math.clz32(i);
}

function searchSegment(val: number, table: readonly number[]): number {
  for (let i = 0; i < table.length; i += 1) {
    const end = table[i];
    if (end !== undefined && val <= end) {
      return i;
    }
  }
  return table.length;
}

function clampI16(sample: number): number {
  const rounded = Math.round(sample);
  if (rounded > 32767) return 32767;
  if (rounded < -32768) return -32768;
  return rounded;
}

function alawFromPcm(sample: number): number {
  let pcm = (sample | 0) >> 3;
  const mask = pcm >= 0 ? 0xd5 : 0x55;
  if (pcm < 0) {
    pcm = -pcm - 1;
  }

  const seg = searchSegment(pcm, A_LAW_SEG_AEND);
  let aval: number;
  if (seg >= 8) {
    aval = 0x7f;
  } else {
    const mant = seg < 2 ? (pcm >> 1) & 0x0f : (pcm >> seg) & 0x0f;
    aval = (seg << 4) | mant;
  }

  return (aval ^ mask) & 0xff;
}

function ulawFromPcm(sample: number): number {
  const sign = sample < 0 ? 0x80 : 0;
  let magnitude = Math.abs(sample | 0);
  magnitude += 0x84; // bias
  if (magnitude > 0x7fff) {
    magnitude = 0x7fff;
  }

  const index = (magnitude >> 7) & 0xff;
  const exponent = MU_LAW_EXP_LUT[index];
  if (exponent === undefined) {
    // Unreachable for 0 <= index < 256, but keep the type checker happy.
    return 0xff;
  }
  const mantissa = (magnitude >> (exponent + 3)) & 0x0f;

  return (~(sign | (exponent << 4) | mantissa)) & 0xff;
}

function encodeOne(kind: G711Kind, sample: number): number {
  const pcm = clampI16(sample);
  return kind === 'alaw' ? alawFromPcm(pcm) : ulawFromPcm(pcm);
}

/** Encode an Int16Array of linear PCM to 8-bit G.711. */
export function encodeG711Int16(
  kind: G711Kind,
  input: Int16Array,
  output: Uint8Array,
): void {
  const len = Math.min(input.length, output.length);
  const f = kind === 'alaw' ? alawFromPcm : ulawFromPcm;
  for (let i = 0; i < len; i += 1) {
    const sample = input[i];
    output[i] = f(sample ?? 0);
  }
}

/** Encode a Float32Array of nominal [-1.0, 1.0] PCM to 8-bit G.711. */
export function encodeG711F32(kind: G711Kind, input: Float32Array, output: Uint8Array): void {
  const len = Math.min(input.length, output.length);
  for (let i = 0; i < len; i += 1) {
    const sample = input[i] ?? 0;
    const pcm =
      sample >= 1.0 ? 32767 : sample <= -1.0 ? -32768 : clampI16(sample * 32767);
    output[i] = encodeOne(kind, pcm);
  }
}

/** Convenience single-sample encoder for tests and small callers. */
export function encodeG711F32One(kind: G711Kind, sample: number): number {
  const pcm =
    sample >= 1.0 ? 32767 : sample <= -1.0 ? -32768 : clampI16(sample * 32767);
  return encodeOne(kind, pcm);
}
