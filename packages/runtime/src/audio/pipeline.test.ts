import { describe, it, expect, vi } from 'vitest';
import { AudioPipeline, AudioPipelineError, type AudioContextLike, type AudioFrame } from './index';

function setCurrentTime(ctx: AudioContextLike, t: number): void {
  (ctx as unknown as { currentTime: number }).currentTime = t;
}

function makeContext(sampleRate = 48000, currentTime = 0): AudioContextLike {
  return {
    sampleRate,
    currentTime,
    state: 'suspended',
    resume: vi.fn(async () => undefined),
    suspend: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
    audioWorklet: { addModule: vi.fn(async () => undefined) },
    destination: { maxChannelCount: 2 },
  };
}

function makeFrame(
  sampleRate: number,
  channels: number,
  frames: number,
  timestamp: number,
  value: number,
): AudioFrame {
  const data: Float32Array[] = [];
  for (let c = 0; c < channels; c += 1) {
    data.push(new Float32Array(frames).fill(value));
  }
  return {
    timestamp,
    sampleRate,
    channels,
    numberOfFrames: frames,
    format: 'f32-planar',
    data,
  };
}

describe('AudioPipeline', () => {
  it('configures and pushes frames without resampling', async () => {
    const context = makeContext(48000, 0);
    const pipeline = new AudioPipeline({ audioContext: context });
    await pipeline.configure({ inputSampleRate: 48000, inputChannels: 2 });
    pipeline.push(makeFrame(48000, 2, 10, 0, 0.5));
    const metrics = pipeline.getMetrics();
    expect(metrics.framesPushed).toBe(10);
    expect(metrics.framesWritten).toBe(9); // resampler needs a lookahead sample
    expect(metrics.ratio).toBeCloseTo(1);
  });

  it('resamples when input and output sample rates differ', async () => {
    const context = makeContext(48000, 0);
    const pipeline = new AudioPipeline({ audioContext: context });
    await pipeline.configure({ inputSampleRate: 44100, inputChannels: 1 });
    pipeline.push(makeFrame(44100, 1, 4410, 0, 0.5));
    const metrics = pipeline.getMetrics();
    expect(metrics.framesWritten).toBeGreaterThan(0);
    expect(metrics.ratio).toBeCloseTo(44100 / 48000);
  });

  it('reports overflow when ring is full', async () => {
    const context = makeContext(48000, 0);
    const onOverrun = vi.fn();
    const pipeline = new AudioPipeline({
      audioContext: context,
      ringCapacityFrames: 4,
      callbacks: { onOverrun },
    });
    await pipeline.configure({ inputSampleRate: 48000, inputChannels: 1 });
    pipeline.push(makeFrame(48000, 1, 10, 0, 0.5));
    const metrics = pipeline.getMetrics();
    expect(metrics.framesDropped).toBeGreaterThan(0);
    expect(onOverrun).toHaveBeenCalled();
  });

  it('rebase on large drift and resets ring', async () => {
    const context = makeContext(48000, 0);
    const pipeline = new AudioPipeline({ audioContext: context, largeDriftMs: 50 });
    await pipeline.configure({ inputSampleRate: 48000, inputChannels: 1 });
    setCurrentTime(context, 0.1);
    pipeline.push(makeFrame(48000, 1, 10, 0, 0.5));

    setCurrentTime(context, 10);
    pipeline.push(makeFrame(48000, 1, 10, 1000, 0.5));

    const metrics = pipeline.getMetrics();
    expect(Math.abs(metrics.driftMs)).toBeGreaterThan(50);
    expect(metrics.ring?.available).toBe(0); // reset should clear ring
  });

  it('adjusts resampler ratio on small drift', async () => {
    const context = makeContext(48000, 0);
    const pipeline = new AudioPipeline({
      audioContext: context,
      smallDriftMs: 10,
      largeDriftMs: 100,
      minRatio: 0.9,
      maxRatio: 1.1,
    });
    await pipeline.configure({ inputSampleRate: 48000, inputChannels: 1 });
    pipeline.push(makeFrame(48000, 1, 10, 0, 0.5));

    setCurrentTime(context, 0.05);
    pipeline.push(makeFrame(48000, 1, 10, 30, 0.5));

    const metrics = pipeline.getMetrics();
    expect(metrics.driftMs).toBeLessThan(0);
    expect(metrics.ratio).toBeGreaterThan(1);
  });

  it('keeps 44.1k->48k ratio near base after drift correction', async () => {
    const context = makeContext(48000, 0);
    const pipeline = new AudioPipeline({
      audioContext: context,
      smallDriftMs: 10,
      largeDriftMs: 100,
    });
    await pipeline.configure({ inputSampleRate: 44100, inputChannels: 1 });
    pipeline.push(makeFrame(44100, 1, 4410, 0, 0.5));

    // Clock runs ahead of media, triggering a small negative drift correction.
    setCurrentTime(context, 0.1);
    pipeline.push(makeFrame(44100, 1, 4410, 50, 0.5));

    const metrics = pipeline.getMetrics();
    expect(metrics.driftMs).toBeLessThan(0);
    expect(metrics.ratio).toBeGreaterThan(0.91);
    expect(metrics.ratio).toBeLessThan(0.93);
  });

  it('requires configure before push', async () => {
    const context = makeContext();
    const onError = vi.fn();
    const pipeline = new AudioPipeline({ audioContext: context, callbacks: { onError } });
    pipeline.push(makeFrame(48000, 1, 10, 0, 0.5));
    expect(onError).toHaveBeenCalled();
  });

  it('rejects invalid constructor and configure options', async () => {
    const context = makeContext();
    expect(() => new AudioPipeline({ audioContext: context, ringCapacityFrames: NaN })).toThrow(AudioPipelineError);
    expect(() => new AudioPipeline({ audioContext: context, smallDriftMs: 100, largeDriftMs: 50 })).toThrow(AudioPipelineError);
    expect(() => new AudioPipeline({ audioContext: context, minRatio: 1.1, maxRatio: 1.0 })).toThrow(AudioPipelineError);

    const pipeline = new AudioPipeline({ audioContext: context });
    await expect(pipeline.configure({ inputSampleRate: NaN, inputChannels: 1 })).rejects.toBeInstanceOf(AudioPipelineError);
    await expect(pipeline.configure({ inputSampleRate: 48000, inputChannels: 0 })).rejects.toBeInstanceOf(AudioPipelineError);
    await expect(pipeline.configure({ inputSampleRate: 48000, inputChannels: 1, outputSampleRate: NaN })).rejects.toBeInstanceOf(AudioPipelineError);
    await expect(pipeline.configure({ inputSampleRate: 48000, inputChannels: 1, outputSampleRate: 44100.5 })).rejects.toBeInstanceOf(AudioPipelineError);
  });

  it('emits error for malformed audio frames', async () => {
    const context = makeContext(48000, 0);
    const onError = vi.fn();
    const pipeline = new AudioPipeline({ audioContext: context, callbacks: { onError } });
    await pipeline.configure({ inputSampleRate: 48000, inputChannels: 1 });
    pipeline.push(makeFrame(48000, 1, 10, NaN, 0.5));
    expect(onError).toHaveBeenCalled();
  });

  it('emits error when frame data is shorter than numberOfFrames', async () => {
    const context = makeContext(48000, 0);
    const onError = vi.fn();
    const pipeline = new AudioPipeline({ audioContext: context, callbacks: { onError } });
    await pipeline.configure({ inputSampleRate: 48000, inputChannels: 1 });
    pipeline.push({
      sampleRate: 48000,
      channels: 1,
      numberOfFrames: 100,
      timestamp: 0,
      format: 'f32-planar',
      data: [new Float32Array(10)],
    });
    expect(onError).toHaveBeenCalled();
  });
});
