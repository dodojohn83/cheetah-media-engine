import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { CheetahPlayerElement } from './player-element';
import type { CheetahPlayer, CheetahPlayerEvent } from '@cheetah-media/web';
import type { Watermark } from './watermark';

describe('CheetahPlayerElement', () => {
  let container: HTMLElement;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    if (!customElements.get('cheetah-player')) {
      customElements.define('cheetah-player', CheetahPlayerElement);
    }
  });

  afterEach(() => {
    container.remove();
  });

  it('renders watermarks declared before the element is connected', () => {
    const el = document.createElement('cheetah-player') as CheetahPlayerElement;
    el.setAttribute(
      'watermarks',
      JSON.stringify([{ type: 'text', content: 'mark', x: 10, y: 20 }]),
    );
    container.appendChild(el);
    const items = el.shadowRoot?.querySelectorAll('.watermark-item');
    expect(items?.length).toBe(1);
    expect(items?.[0]?.textContent).toBe('mark');
  });

  it('updates watermarks after the element is connected', () => {
    const el = document.createElement('cheetah-player') as CheetahPlayerElement;
    container.appendChild(el);
    el.setWatermarks([{ type: 'text', content: 'updated' }]);
    const items = el.shadowRoot?.querySelectorAll('.watermark-item');
    expect(items?.length).toBe(1);
    expect(items?.[0]?.textContent).toBe('updated');
  });

  it('gracefully drops non-serializable watermarks instead of throwing', () => {
    const el = document.createElement('cheetah-player') as CheetahPlayerElement;
    container.appendChild(el);
    const circular = { type: 'text', content: 'x' } as unknown as Record<string, unknown>;
    circular.self = circular;
    expect(() => el.setWatermarks([circular] as unknown as Watermark[])).not.toThrow();
    expect(el.hasAttribute('watermarks')).toBe(false);
    expect(el.watermarks).toEqual([]);
  });

  it('keeps controls above the status overlay so they remain clickable', () => {
    const el = document.createElement('cheetah-player') as CheetahPlayerElement;
    container.appendChild(el);
    const controls = el.shadowRoot?.querySelector('.controls');
    const overlay = el.shadowRoot?.querySelector('.overlay');
    const styles = getComputedStyle(controls!);
    const overlayStyles = getComputedStyle(overlay!);
    expect(parseInt(styles.zIndex, 10)).toBeGreaterThan(parseInt(overlayStyles.zIndex, 10));
  });

  it('renders metadata overlay shapes from player metadata events', () => {
    const el = document.createElement('cheetah-player') as CheetahPlayerElement;
    container.appendChild(el);

    const listeners = new Map<string, (event: CheetahPlayerEvent<'metadata'>) => void>();
    const fakePlayer = {
      addEventListener: <T extends string>(type: T, fn: (event: CheetahPlayerEvent<'metadata'>) => void) => {
        listeners.set(type, fn);
      },
      removeEventListener: () => {},
    } as unknown as CheetahPlayer;

    (el as unknown as { _player: CheetahPlayer })._player = fakePlayer;
    (el as unknown as { _bindPlayer: () => void })._bindPlayer();

    const svg = el.shadowRoot?.querySelector('.overlay-svg');
    expect(svg).not.toBeNull();

    const metadataListener = listeners.get('metadata');
    expect(metadataListener).toBeDefined();

    const payload = JSON.stringify({
      shapes: [
        { type: 'line', x1: 0.1, y1: 0.2, x2: 0.3, y2: 0.4 },
        { type: 'text', x: 0.1, y: 0.1, text: 'label' },
      ],
    });

    metadataListener!({
      type: 'metadata',
      playerId: 'test',
      epoch: 1,
      sequence: 1,
      timestamp: Date.now(),
      details: { items: [{ source: 0, key: 0, value: payload }] },
    } as CheetahPlayerEvent<'metadata'>);

    expect(svg?.querySelectorAll('line').length).toBe(1);
    expect(svg?.querySelectorAll('text').length).toBe(1);
    expect(svg?.querySelector('text')?.textContent).toBe('label');

    // A second empty metadata event clears the overlay.
    metadataListener!({
      type: 'metadata',
      playerId: 'test',
      epoch: 1,
      sequence: 2,
      timestamp: Date.now(),
      details: { items: [] },
    } as CheetahPlayerEvent<'metadata'>);

    expect(svg?.children.length).toBe(0);
  });

  it('reflects recordingactive attribute and dispatches recordingprogress events', () => {
    const el = document.createElement('cheetah-player') as CheetahPlayerElement;
    container.appendChild(el);

    const listeners = new Map<string, (event: CheetahPlayerEvent<'compositeRecording'>) => void>();
    const fakePlayer = {
      addEventListener: <T extends string>(type: T, fn: (event: CheetahPlayerEvent<'compositeRecording'>) => void) => {
        listeners.set(type, fn);
      },
      removeEventListener: () => {},
      startCompositeRecording: () => Promise.resolve(),
      stopCompositeRecording: () => Promise.resolve({ blob: new Blob(['x']) } as unknown as import('@cheetah-media/web').CompositeRecordingResult),
    } as unknown as CheetahPlayer;

    (el as unknown as { _player: CheetahPlayer })._player = fakePlayer;
    (el as unknown as { _bindPlayer: () => void })._bindPlayer();

    const recordingListener = listeners.get('compositeRecording');
    expect(recordingListener).toBeDefined();

    let progressFired = false;
    el.addEventListener('recordingprogress', () => {
      progressFired = true;
    });

    recordingListener!({
      type: 'compositeRecording',
      playerId: 'test',
      epoch: 1,
      sequence: 1,
      timestamp: Date.now(),
      details: { active: true, progress: { bytesWritten: 100, durationMs: 50, state: 'recording' } },
    } as CheetahPlayerEvent<'compositeRecording'>);

    expect(el.recordingactive).toBe(true);
    expect(el.hasAttribute('recordingactive')).toBe(true);
    expect(progressFired).toBe(true);
  });

  it('dispatches downloadprogress events and toggles the download button', async () => {
    const el = document.createElement('cheetah-player') as CheetahPlayerElement;
    el.setAttribute('download', 'https://example.com/video.mp4');
    container.appendChild(el);

    const listeners = new Map<string, (event: CheetahPlayerEvent<'download'>) => void>();
    let downloadActive = false;
    const fakePlayer = {
      addEventListener: <T extends string>(type: T, fn: (event: CheetahPlayerEvent<'download'>) => void) => {
        listeners.set(type, fn);
      },
      removeEventListener: () => {},
      get downloadActive() {
        return downloadActive;
      },
      startDownload: () => {
        downloadActive = true;
        return Promise.resolve();
      },
      stopDownload: () => {
        downloadActive = false;
        return Promise.resolve();
      },
    } as unknown as CheetahPlayer;

    (el as unknown as { _player: CheetahPlayer })._player = fakePlayer;
    (el as unknown as { _bindPlayer: () => void })._bindPlayer();

    const downloadListener = listeners.get('download');
    expect(downloadListener).toBeDefined();

    let progressFired = false;
    el.addEventListener('downloadprogress', () => {
      progressFired = true;
    });

    await (el as unknown as { _toggleDownload: () => Promise<void> })._toggleDownload();
    downloadListener!({
      type: 'download',
      playerId: 'test',
      epoch: 1,
      sequence: 1,
      timestamp: Date.now(),
      details: { active: true, progress: { bytesWritten: 1024, state: 'downloading' } },
    } as CheetahPlayerEvent<'download'>);

    expect(progressFired).toBe(true);
    expect(downloadActive).toBe(true);
  });
});
