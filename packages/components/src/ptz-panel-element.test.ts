import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createGb28181PtzCmd } from '@cheetah-media/web';
import { CheetahPtzPanelElement, type PtzEventDetail } from './ptz-panel-element';

describe('CheetahPtzPanelElement', () => {
  let container: HTMLElement;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    if (!customElements.get('cheetah-ptz-panel')) {
      customElements.define('cheetah-ptz-panel', CheetahPtzPanelElement);
    }
  });

  afterEach(() => {
    container.remove();
  });

  it('sanity: createGb28181PtzCmd is importable', () => {
    const cmd = createGb28181PtzCmd({ action: 'up', speeds: { vertical: 16 } });
    expect(typeof cmd).toBe('string');
    expect(cmd).toMatch(/^[0-9A-F]{16}$/);
  });

  it('sanity: custom events carry detail', () => {
    const el = document.createElement('cheetah-ptz-panel') as CheetahPtzPanelElement;
    container.appendChild(el);
    let detail: unknown;
    el.addEventListener('ptz', (event) => {
      detail = (event as CustomEvent).detail;
    });
    el.dispatchEvent(new CustomEvent('ptz', { detail: { action: 'test' }, bubbles: true, composed: true }));
    expect(detail).toEqual({ action: 'test' });
  });

  it('creates a shadow root with a ptz pad', () => {
    const el = document.createElement('cheetah-ptz-panel') as CheetahPtzPanelElement;
    container.appendChild(el);
    expect(el.shadowRoot).toBeTruthy();
    expect(el.shadowRoot?.querySelectorAll('.ptz-pad button').length).toBe(9);
  });

  it('dispatches a ptz event with a GB28181 payload when a direction button is clicked', () => {
    const el = document.createElement('cheetah-ptz-panel') as CheetahPtzPanelElement;
    container.appendChild(el);

    let detail: PtzEventDetail | undefined;
    el.addEventListener('ptz', (event) => {
      detail = (event as CustomEvent).detail as PtzEventDetail;
    });

    const up = el.shadowRoot?.querySelector('[part="ptz-up"]') as HTMLButtonElement | null;
    expect(up).toBeTruthy();
    up?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));

    expect(detail).toBeDefined();
    expect(detail?.action).toBe('up');
    expect(detail?.protocol).toBe('gb28181');
    expect(detail?.ptzCmd).toMatch(/^[0-9A-F]{16}$/);
  });

  it('reflects speed attribute and slider value changes', () => {
    const el = document.createElement('cheetah-ptz-panel') as CheetahPtzPanelElement;
    el.speed = 200;
    container.appendChild(el);
    expect(el.getAttribute('speed')).toBe('200');

    const slider = el.shadowRoot?.querySelector('input[name="speed"]') as HTMLInputElement | null;
    expect(slider).toBeTruthy();
    slider!.value = '50';
    slider?.dispatchEvent(new Event('input'));
    expect(el.speed).toBe(50);
  });

  it('dispatches a preset command when a preset button is clicked', () => {
    const el = document.createElement('cheetah-ptz-panel') as CheetahPtzPanelElement;
    container.appendChild(el);

    const input = el.shadowRoot?.querySelector('input[part="preset-number"]') as HTMLInputElement | null;
    input!.value = '3';

    let detail: PtzEventDetail | undefined;
    el.addEventListener('ptz', (event) => {
      detail = (event as CustomEvent).detail as PtzEventDetail;
    });

    const call = el.shadowRoot?.querySelector('[part="ptz-preset-call"]') as HTMLButtonElement | null;
    call?.click();

    expect(detail?.action).toBe('presetCall');
    expect(detail?.ptzCmd).toMatch(/^[0-9A-F]{16}$/);
  });

  it('survives disconnection and reconnection without recreating shadow root', () => {
    const el = document.createElement('cheetah-ptz-panel') as CheetahPtzPanelElement;
    container.appendChild(el);
    const shadow = el.shadowRoot;
    expect(shadow).toBeTruthy();

    container.removeChild(el);
    container.appendChild(el);
    expect(el.shadowRoot).toBe(shadow);

    let detail: PtzEventDetail | undefined;
    el.addEventListener('ptz', (event) => {
      detail = (event as CustomEvent).detail as PtzEventDetail;
    });
    el.dispatchEvent(new KeyboardEvent('keydown', { code: 'ArrowUp', bubbles: true }));
    expect(detail?.action).toBe('up');
  });

  it('keyboard arrow keys dispatch ptz events', () => {
    const el = document.createElement('cheetah-ptz-panel') as CheetahPtzPanelElement;
    container.appendChild(el);

    let detail: PtzEventDetail | undefined;
    el.addEventListener('ptz', (event) => {
      detail = (event as CustomEvent).detail as PtzEventDetail;
    });

    el.dispatchEvent(new KeyboardEvent('keydown', { code: 'ArrowRight', bubbles: true }));
    expect(detail?.action).toBe('right');
  });
});
