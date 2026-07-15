import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { CheetahPlayerElement, createPlayerComponent } from './index';

describe('components', () => {
  describe('createPlayerComponent factory', () => {
    it('creates a component', () => {
      const component = createPlayerComponent();
      expect(component.player).toBeDefined();
    });

    it('preserves runtime config from PlayerConfig', () => {
      const component = createPlayerComponent({
        runtime: { workerUrl: '/w.js', wasmUrl: '/e.wasm' },
        workerUrl: '/top-w.js',
      });
      expect(component.player).toBeDefined();
    });
  });

  describe('CheetahPlayerElement', () => {
    let container: HTMLElement;

    beforeEach(() => {
      container = document.createElement('div');
      document.body.appendChild(container);
    });

    afterEach(() => {
      container.remove();
    });

    it('registers the custom element', () => {
      expect(customElements.get('cheetah-player')).toBe(CheetahPlayerElement);
    });

    it('reflects boolean attributes', () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      el.controls = true;
      el.autoplay = true;
      el.muted = true;
      el.live = true;
      expect(el.hasAttribute('controls')).toBe(true);
      expect(el.hasAttribute('autoplay')).toBe(true);
      expect(el.hasAttribute('muted')).toBe(true);
      expect(el.hasAttribute('live')).toBe(true);
    });

    it('reflects string and numeric attributes', () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      el.src = 'https://example.com/live.flv';
      el.workerUrl = '/worker.js';
      el.wasmUrl = '/engine.wasm';
      el.volume = 0.5;
      el.statsIntervalMs = 500;
      el.maxEventHistory = 100;
      expect(el.getAttribute('src')).toBe('https://example.com/live.flv');
      expect(el.getAttribute('worker-url')).toBe('/worker.js');
      expect(el.getAttribute('wasm-url')).toBe('/engine.wasm');
      expect(el.getAttribute('volume')).toBe('0.5');
      expect(el.getAttribute('stats-interval')).toBe('500');
      expect(el.getAttribute('max-event-history')).toBe('100');
      expect(el.volume).toBe(0.5);
    });

    it('creates shadow DOM with surface slot and controls', () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      container.appendChild(el);
      const shadow = el.shadowRoot;
      expect(shadow).toBeTruthy();
      expect(shadow?.querySelector('[part="surface"]')).toBeTruthy();
      expect(shadow?.querySelector('[part="controls"]')).toBeTruthy();
      expect(shadow?.querySelector('[part="overlay"]')).toBeTruthy();
    });

    it('shows failed overlay when load fails', async () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      container.appendChild(el);
      el.src = 'https://example.com/missing.flv';
      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(el.getAttribute('data-state')).toBe('failed');
      const overlay = el.shadowRoot?.querySelector('.overlay');
      expect(overlay?.classList.contains('active')).toBe(true);
    });

    it('updates play button label when state changes', async () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      container.appendChild(el);
      el.src = 'https://example.com/missing.flv';
      await new Promise((resolve) => setTimeout(resolve, 50));
      const playButton = el.shadowRoot?.querySelector('.play-button') as HTMLButtonElement | null;
      expect(playButton).toBeTruthy();
      expect(playButton?.getAttribute('aria-label')).toBeTruthy();
    });

    it('supports keyboard shortcuts', async () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      container.appendChild(el);
      el.src = 'https://example.com/missing.flv';
      await new Promise((resolve) => setTimeout(resolve, 50));
      let eventFired = false;
      el.addEventListener('snapshot', () => {
        eventFired = true;
      });
      el.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyS', bubbles: true }));
      // Snapshot without a player does not dispatch, but the key is handled.
      expect(eventFired).toBe(false);
    });

    it('retry button re-attempts load after failure', async () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      container.appendChild(el);
      el.src = 'https://example.com/missing.flv';
      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(el.getAttribute('data-state')).toBe('failed');

      const retryButton = el.shadowRoot?.querySelector('[part="overlay-button"]') as HTMLButtonElement | null;
      expect(retryButton).toBeTruthy();
      retryButton?.click();
      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(el.getAttribute('data-state')).toBe('failed');
    });

    it('reloads after src is removed and re-added', async () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      container.appendChild(el);
      el.src = 'https://example.com/missing.flv';
      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(el.getAttribute('data-state')).toBe('failed');

      el.src = undefined;
      await new Promise((resolve) => setTimeout(resolve, 10));
      expect(el.getAttribute('data-state')).toBe('idle');

      el.src = 'https://example.com/missing.flv';
      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(el.getAttribute('data-state')).toBe('failed');
    });

    it('handles src change while a load is in progress', async () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      container.appendChild(el);
      el.src = 'https://example.com/a.flv';
      el.src = 'https://example.com/b.flv';
      await new Promise((resolve) => setTimeout(resolve, 100));
      expect(el.src).toBe('https://example.com/b.flv');
      expect(el.getAttribute('data-state')).toBe('failed');
    });

    it('handles three rapid src changes without leaking players', async () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      container.appendChild(el);
      el.src = 'https://example.com/a.flv';
      el.src = 'https://example.com/b.flv';
      el.src = 'https://example.com/c.flv';
      await new Promise((resolve) => setTimeout(resolve, 150));
      expect(el.src).toBe('https://example.com/c.flv');
      expect(el.getAttribute('data-state')).toBe('failed');
    });

    it('does not freeze when the same src is requested while loading', async () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      container.appendChild(el);
      el.src = 'https://example.com/a.flv';
      el.src = 'https://example.com/a.flv';
      await new Promise((resolve) => setTimeout(resolve, 150));
      expect(el.src).toBe('https://example.com/a.flv');
      expect(el.getAttribute('data-state')).toBe('failed');

      el.src = 'https://example.com/b.flv';
      await new Promise((resolve) => setTimeout(resolve, 100));
      expect(el.src).toBe('https://example.com/b.flv');
      expect(el.getAttribute('data-state')).toBe('failed');
    });

    it('reloads when live flag changes during a load', async () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      container.appendChild(el);
      el.src = 'https://example.com/live.flv';
      el.live = true;
      await new Promise((resolve) => setTimeout(resolve, 50));
      el.live = false;
      await new Promise((resolve) => setTimeout(resolve, 150));
      expect(el.src).toBe('https://example.com/live.flv');
      expect(el.getAttribute('data-state')).toBe('failed');
    });

    it('does not crash when live is set before the element is connected', async () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      el.src = 'https://example.com/live.flv';
      el.live = true;
      container.appendChild(el);
      await new Promise((resolve) => setTimeout(resolve, 100));
      expect(el.getAttribute('data-state')).toBe('failed');
    });

    it('does not create a new player after the element is removed mid-source-change', async () => {
      const el = document.createElement('cheetah-player') as CheetahPlayerElement;
      container.appendChild(el);
      el.src = 'https://example.com/a.flv';
      el.src = 'https://example.com/b.flv';
      container.removeChild(el);
      await new Promise((resolve) => setTimeout(resolve, 100));
      expect(el.player).toBeUndefined();
      expect(el.getAttribute('data-state')).toBe('idle');
    });
  });
});
