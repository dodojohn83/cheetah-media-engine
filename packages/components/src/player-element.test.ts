import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { CheetahPlayerElement } from './player-element';

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

  it('keeps controls above the status overlay so they remain clickable', () => {
    const el = document.createElement('cheetah-player') as CheetahPlayerElement;
    container.appendChild(el);
    const controls = el.shadowRoot?.querySelector('.controls');
    const overlay = el.shadowRoot?.querySelector('.overlay');
    const styles = getComputedStyle(controls!);
    const overlayStyles = getComputedStyle(overlay!);
    expect(parseInt(styles.zIndex, 10)).toBeGreaterThan(parseInt(overlayStyles.zIndex, 10));
  });
});
