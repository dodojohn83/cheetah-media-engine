/**
 * PCM sample format and AudioData abstractions used by the audio pipeline.
 *
 * The pipeline works with planar Float32 internally; this module converts
 * arbitrary AudioData formats without guessing channel layouts.
 */

export type AudioSampleFormat =
  | 'f32-planar'
  | 'f32-interleaved'
  | 's16-planar'
  | 's16-interleaved'
  | 'u8'
  | 's16'
  | 'f32';

export interface AudioFrame {
  readonly timestamp: number; // media time in milliseconds
  readonly sampleRate: number;
  readonly channels: number;
  readonly numberOfFrames: number;
  readonly format: string;
  /**
   * Optional planar data for tests or pre-extracted frames.  If omitted,
   * `copyTo` must be implemented.
   */
  readonly data?: readonly Float32Array[] | undefined;
  /**
   * Browser WebCodecs AudioData copyTo interface.
   */
  copyTo?(
    destination: Float32Array | Int16Array | Uint8Array,
    options: { planeIndex: number; frameOffset?: number | undefined; frameCount?: number | undefined },
  ): void;
}

interface AudioFrameWithCopy extends AudioFrame {
  copyTo: (
    destination: Float32Array | Int16Array | Uint8Array,
    options: { planeIndex: number; frameOffset?: number | undefined; frameCount?: number | undefined },
  ) => void;
}

interface FormatCategory {
  readonly signed: boolean;
  readonly bytes: number;
  readonly planar: boolean;
}

const FORMAT_TO_CATEGORY: Record<string, FormatCategory> = {
  'f32-planar': { signed: true, bytes: 4, planar: true },
  'f32-interleaved': { signed: true, bytes: 4, planar: false },
  's16-planar': { signed: true, bytes: 2, planar: true },
  's16-interleaved': { signed: true, bytes: 2, planar: false },
  f32: { signed: true, bytes: 4, planar: false },
  s16: { signed: true, bytes: 2, planar: false },
  u8: { signed: false, bytes: 1, planar: false },
};

function parseFormat(format: string): FormatCategory | undefined {
  const f = format.toLowerCase();
  return FORMAT_TO_CATEGORY[f];
}

function normalizeToF32(value: number, category: FormatCategory): number {
  if (category.bytes === 4) return value;
  if (category.bytes === 2) return value / 32768;
  // u8: [0,255] -> [-1,1]
  return value / 128 - 1;
}

function copyPlanarFromFrame(
  frame: AudioFrameWithCopy,
  category: FormatCategory,
  outputChannels: number,
): Float32Array[] {
  const { channels, numberOfFrames } = frame;
  const result: Float32Array[] = [];

  for (let c = 0; c < outputChannels; c += 1) {
    const dst = new Float32Array(numberOfFrames);
    const planeIndex = c < channels ? c : 0;

    if (category.bytes === 4) {
      const tmp = new Float32Array(numberOfFrames);
      frame.copyTo(tmp, { planeIndex, frameCount: numberOfFrames });
      for (let i = 0; i < numberOfFrames; i += 1) {
        dst[i] = tmp.at(i) ?? 0;
      }
    } else if (category.bytes === 2) {
      const tmp = new Int16Array(numberOfFrames);
      frame.copyTo(tmp, { planeIndex, frameCount: numberOfFrames });
      for (let i = 0; i < numberOfFrames; i += 1) {
        const raw = tmp.at(i) ?? 0;
        dst[i] = normalizeToF32(raw, category);
      }
    } else {
      const tmp = new Uint8Array(numberOfFrames);
      frame.copyTo(tmp, { planeIndex, frameCount: numberOfFrames });
      for (let i = 0; i < numberOfFrames; i += 1) {
        const raw = tmp.at(i) ?? 128;
        dst[i] = normalizeToF32(raw, category);
      }
    }

    result.push(dst);
  }
  return result;
}

function deinterleave(
  interleaved: Float32Array,
  channels: number,
  outputChannels: number,
  frames: number,
): Float32Array[] {
  const result: Float32Array[] = [];
  for (let c = 0; c < outputChannels; c += 1) {
    const dst = new Float32Array(frames);
    const srcChannel = c < channels ? c : 0;
    for (let i = 0; i < frames; i += 1) {
      dst[i] = interleaved.at(i * channels + srcChannel) ?? 0;
    }
    result.push(dst);
  }
  return result;
}

function copyInterleavedFromFrame(
  frame: AudioFrameWithCopy,
  category: FormatCategory,
  outputChannels: number,
): Float32Array[] {
  const { channels, numberOfFrames } = frame;
  const interleaved = new Float32Array(numberOfFrames * channels);

  if (category.bytes === 4) {
    const tmp = new Float32Array(numberOfFrames * channels);
    frame.copyTo(tmp, { planeIndex: 0, frameCount: numberOfFrames });
    for (let i = 0; i < tmp.length; i += 1) {
      interleaved[i] = tmp.at(i) ?? 0;
    }
  } else if (category.bytes === 2) {
    const tmp = new Int16Array(numberOfFrames * channels);
    frame.copyTo(tmp, { planeIndex: 0, frameCount: numberOfFrames });
    for (let i = 0; i < tmp.length; i += 1) {
      interleaved[i] = normalizeToF32(tmp.at(i) ?? 0, category);
    }
  } else {
    const tmp = new Uint8Array(numberOfFrames * channels);
    frame.copyTo(tmp, { planeIndex: 0, frameCount: numberOfFrames });
    for (let i = 0; i < tmp.length; i += 1) {
      interleaved[i] = normalizeToF32(tmp.at(i) ?? 128, category);
    }
  }

  return deinterleave(interleaved, channels, outputChannels, numberOfFrames);
}

export function extractPlanarF32(frame: AudioFrame, outputChannels: number): Float32Array[] {
  if (!frame || typeof frame !== 'object') {
    throw new Error('frame must be an object');
  }
  if (!Number.isInteger(outputChannels) || outputChannels < 1 || !Number.isFinite(outputChannels)) {
    throw new Error('outputChannels must be a finite positive integer');
  }
  const { numberOfFrames } = frame;

  if (frame.data) {
    if (!Array.isArray(frame.data) || frame.data.length === 0) {
      throw new Error('AudioFrame data must be a non-empty array of Float32Array planes');
    }
    const planar = frame.data;
    const minFrames = planar.reduce((m, p) => Math.min(m, p.length), Infinity);
    if (!Number.isFinite(minFrames) || numberOfFrames > minFrames) {
      throw new Error('AudioFrame numberOfFrames exceeds provided plane length');
    }
    const result: Float32Array[] = [];
    for (let c = 0; c < outputChannels; c += 1) {
      const src = planar.at(c) ?? planar.at(0) ?? new Float32Array(0);
      const dst = new Float32Array(numberOfFrames);
      for (let i = 0; i < numberOfFrames; i += 1) {
        dst[i] = src.at(i) ?? 0;
      }
      result.push(dst);
    }
    return result;
  }

  if (!frame.copyTo) {
    throw new Error(`AudioFrame must provide data or copyTo: ${frame.format}`);
  }

  const category = parseFormat(frame.format);
  if (!category) {
    throw new Error(`Unsupported audio format: ${frame.format}`);
  }

  const frameWithCopy = frame as AudioFrameWithCopy;
  return category.planar
    ? copyPlanarFromFrame(frameWithCopy, category, outputChannels)
    : copyInterleavedFromFrame(frameWithCopy, category, outputChannels);
}
