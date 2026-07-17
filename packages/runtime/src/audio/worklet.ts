/**
 * AudioWorklet processor source and helper to build a Blob URL for it.
 *
 * Two processors are provided:
 *  - `cheetah-audio-processor` uses a SharedArrayBuffer ring (isolated mode).
 *  - `cheetah-audio-transfer-processor` receives transferable blocks over a
 *    MessagePort (non-isolated fallback).
 */

const SHARED_RING_PROCESSOR = `
const STATE_SLOTS = 8;
const IDX_WRITE = 0;
const IDX_READ = 1;
const IDX_CAPACITY = 2;
const IDX_CHANNELS = 3;
const IDX_GENERATION = 4;
const IDX_UNDERRUN = 5;
const IDX_OVERRUN = 6;
const STATE_BYTES = STATE_SLOTS * 4;

class CheetahAudioProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const sab = options.processorOptions.sab;
    this.capacity = options.processorOptions.capacity;
    this.channels = options.processorOptions.channels;
    this.state = new Int32Array(sab, 0, STATE_SLOTS);
    this.samples = new Float32Array(sab, STATE_BYTES, this.capacity * this.channels);
    this.readIndex = 0;
    this.generation = 0;
    this.underrun = 0;
  }

  process(inputs, outputs) {
    const output = outputs[0];
    const outFrames = output[0].length;
    const writeIndex = Atomics.load(this.state, IDX_WRITE);
    this.readIndex = Atomics.load(this.state, IDX_READ) % this.capacity;
    const gen = Atomics.load(this.state, IDX_GENERATION);
    if (gen !== this.generation) {
      this.readIndex = writeIndex;
      this.generation = gen;
    }

    for (let s = 0; s < outFrames; s += 1) {
      if (this.readIndex === writeIndex) {
        for (let c = 0; c < this.channels; c += 1) {
          output[c][s] = 0;
        }
        this.underrun += 1;
        continue;
      }
      const base = this.readIndex * this.channels;
      for (let c = 0; c < this.channels; c += 1) {
        output[c][s] = this.samples[base + c];
      }
      this.readIndex = (this.readIndex + 1) % this.capacity;
    }

    Atomics.store(this.state, IDX_READ, this.readIndex);
    if (this.underrun > 0) {
      Atomics.add(this.state, IDX_UNDERRUN, this.underrun);
      this.underrun = 0;
    }
    return true;
  }
}
registerProcessor('cheetah-audio-processor', CheetahAudioProcessor);
`;

const TRANSFER_RING_PROCESSOR = `
class CheetahAudioTransferProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.capacity = options.processorOptions.capacity;
    this.channels = options.processorOptions.channels;
    this.samples = new Float32Array(this.capacity * this.channels);
    this.writeIndex = 0;
    this.readIndex = 0;
    this.underrun = 0;
    this.overrun = 0;
    this.generation = 0;
    this.port = options.processorOptions.port;
    this.port.onmessage = (e) => {
      const msg = e.data;
      if (msg.type === 'reset') {
        this.generation = msg.generation || 0;
        this.writeIndex = this.readIndex;
        return;
      }
      const view = new Float32Array(msg.buffer);
      const frames = msg.frames;
      const channels = msg.channels || this.channels;
      for (let i = 0; i < frames; i += 1) {
        if ((this.writeIndex + 1) % this.capacity === this.readIndex) {
          this.overrun += frames - i;
          break;
        }
        const base = this.writeIndex * this.channels;
        for (let c = 0; c < this.channels; c += 1) {
          this.samples[base + c] = view[i * channels + c] || 0;
        }
        this.writeIndex = (this.writeIndex + 1) % this.capacity;
      }
    };
  }

  process(inputs, outputs) {
    const output = outputs[0];
    const outFrames = output[0].length;
    for (let s = 0; s < outFrames; s += 1) {
      if (this.readIndex === this.writeIndex) {
        for (let c = 0; c < this.channels; c += 1) {
          output[c][s] = 0;
        }
        this.underrun += 1;
        continue;
      }
      const base = this.readIndex * this.channels;
      for (let c = 0; c < this.channels; c += 1) {
        output[c][s] = this.samples[base + c];
      }
      this.readIndex = (this.readIndex + 1) % this.capacity;
    }
    if (this.underrun > 0) {
      this.port.postMessage({ type: 'underrun', count: this.underrun });
      this.underrun = 0;
    }
    if (this.overrun > 0) {
      this.port.postMessage({ type: 'overrun', count: this.overrun });
      this.overrun = 0;
    }
    return true;
  }
}
registerProcessor('cheetah-audio-transfer-processor', CheetahAudioTransferProcessor);
`;

export function buildWorkletBlobUrl(source: string): string {
  if (typeof Blob === 'undefined' || typeof URL === 'undefined') {
    throw new Error('Blob/URL API not available; cannot build AudioWorklet source URL');
  }
  const blob = new Blob([source], { type: 'application/javascript' });
  return URL.createObjectURL(blob);
}

export function getSharedRingProcessorSource(): string {
  return SHARED_RING_PROCESSOR;
}

export function getTransferRingProcessorSource(): string {
  return TRANSFER_RING_PROCESSOR;
}

const CAPTURE_PROCESSOR = `
class CheetahCaptureProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.frameSize = options.processorOptions.frameSize;
    this.buffer = new Float32Array(this.frameSize);
    this.write = 0;
  }

  process(inputs, outputs, parameters) {
    // Capture processors need an output so the Web Audio graph pulls them.
    for (let o = 0; o < outputs.length; o += 1) {
      const output = outputs[o];
      for (let c = 0; c < output.length; c += 1) {
        const channel = output[c];
        for (let s = 0; s < channel.length; s += 1) {
          channel[s] = 0;
        }
      }
    }

    const input = inputs[0] && inputs[0][0];
    if (!input) return true;
    for (let i = 0; i < input.length; i += 1) {
      this.buffer[this.write] = input[i];
      this.write += 1;
      if (this.write === this.frameSize) {
        const frame = this.buffer;
        this.port.postMessage({ type: 'frame', samples: frame }, [frame.buffer]);
        this.buffer = new Float32Array(this.frameSize);
        this.write = 0;
      }
    }
    return true;
  }
}
registerProcessor('cheetah-capture-processor', CheetahCaptureProcessor);
`;

export function getCaptureProcessorSource(): string {
  return CAPTURE_PROCESSOR;
}
