import type { CheetahPlayer } from '@cheetah-media/web';

const OBSERVED_ATTRIBUTES = [
  'cell-id',
  'main-src',
  'sub-src',
  'label',
  'live',
  'autoplay',
  'controls',
  'muted',
  'volume',
  'worker-url',
  'wasm-url',
  'quality',
] as const;

type ObservedAttribute = (typeof OBSERVED_ATTRIBUTES)[number];

export class CheetahWallCellElement extends HTMLElement {
  static get observedAttributes(): readonly string[] {
    return OBSERVED_ATTRIBUTES as unknown as readonly string[];
  }

  private _playerEl: HTMLElement | undefined;
  private _quality: 'main' | 'sub' | 'auto' = 'auto';

  get cellId(): string | undefined {
    return this.getAttribute('cell-id') ?? undefined;
  }

  set cellId(value: string | undefined) {
    if (value === undefined) this.removeAttribute('cell-id');
    else this.setAttribute('cell-id', value);
  }

  get mainSrc(): string | undefined {
    return this.getAttribute('main-src') ?? undefined;
  }

  set mainSrc(value: string | undefined) {
    if (value === undefined) this.removeAttribute('main-src');
    else this.setAttribute('main-src', value);
  }

  get subSrc(): string | undefined {
    return this.getAttribute('sub-src') ?? undefined;
  }

  set subSrc(value: string | undefined) {
    if (value === undefined) this.removeAttribute('sub-src');
    else this.setAttribute('sub-src', value);
  }

  get quality(): 'main' | 'sub' | 'auto' {
    return this._quality;
  }

  set quality(value: 'main' | 'sub' | 'auto') {
    this._quality = value;
    this.setAttribute('quality', value);
  }

  get player(): CheetahPlayer | undefined {
    const el = this._playerEl;
    if (!el) return undefined;
    return (el as { player?: CheetahPlayer }).player;
  }

  connectedCallback(): void {
    if (!this.shadowRoot) {
      const shadow = this.attachShadow({ mode: 'open' });
      const style = document.createElement('style');
      style.textContent = /*css*/ `
        :host {
          display: block;
          position: relative;
          width: 100%;
          height: 100%;
          background: #000;
          overflow: hidden;
        }
        .container {
          width: 100%;
          height: 100%;
          display: flex;
          flex-direction: column;
        }
        .label {
          position: absolute;
          top: 4px;
          left: 4px;
          padding: 2px 6px;
          background: rgba(0, 0, 0, 0.6);
          color: #fff;
          font: 12px system-ui, sans-serif;
          pointer-events: none;
          z-index: 1;
        }
        cheetah-player {
          flex: 1;
          min-height: 0;
        }
      `;
      shadow.appendChild(style);
      const container = document.createElement('div');
      container.className = 'container';
      shadow.appendChild(container);

      const label = document.createElement('div');
      label.className = 'label';
      container.appendChild(label);

      const player = document.createElement('cheetah-player') as HTMLElement;
      player.setAttribute('part', 'player');
      container.appendChild(player);
      this._playerEl = player;
    }
    this._syncLabel();
    this._syncPlayerAttributes();
    this._applyQuality();
  }

  disconnectedCallback(): void {
    // Do not remove the player element here; the inner <cheetah-player> owns
    // its own cleanup via the on-disconnect attribute, and removing the DOM
    // would prevent rebuilding when the cell is moved or reconnected.
  }

  attributeChangedCallback(name: ObservedAttribute, oldValue: string | null, newValue: string | null): void {
    if (oldValue === newValue) return;
    if (name === 'label') {
      this._syncLabel();
      return;
    }
    if (name === 'quality' && newValue) {
      const q = newValue as 'main' | 'sub' | 'auto';
      this._quality = ['main', 'sub', 'auto'].includes(q) ? q : 'auto';
      this._applyQuality();
      return;
    }
    if (name === 'main-src' || name === 'sub-src') {
      this._syncPlayerAttributes();
      this._applyQuality();
      return;
    }
    this._syncPlayerAttributes();
  }

  setQuality(quality: 'main' | 'sub' | 'auto'): void {
    this.quality = quality;
  }

  destroy(): void {
    this._destroyPlayer();
    if (this.shadowRoot) {
      this.shadowRoot.innerHTML = '';
      this._playerEl = undefined;
    }
  }

  private _syncLabel(): void {
    if (!this.shadowRoot) return;
    const label = this.shadowRoot.querySelector('.label');
    if (label) {
      label.textContent = this.getAttribute('label') ?? '';
    }
  }

  private _syncPlayerAttributes(): void {
    if (!this._playerEl) return;
    const player = this._playerEl;
    this._setOrRemove(player, 'live', this.hasAttribute('live') ? '' : undefined);
    this._setOrRemove(player, 'autoplay', this.hasAttribute('autoplay') ? '' : undefined);
    this._setOrRemove(player, 'controls', this.hasAttribute('controls') ? '' : undefined);
    this._setOrRemove(player, 'muted', this.hasAttribute('muted') ? '' : undefined);
    this._setOrRemove(player, 'volume', this.getAttribute('volume'));
    this._setOrRemove(player, 'worker-url', this.getAttribute('worker-url'));
    this._setOrRemove(player, 'wasm-url', this.getAttribute('wasm-url'));
    // Wall cells are owned by the wall and must release the player on removal.
    this._setOrRemove(player, 'on-disconnect', 'destroy');
  }

  private _applyQuality(): void {
    if (!this._playerEl) return;
    const quality = this._quality === 'auto' ? 'main' : this._quality;
    const src = quality === 'sub' ? this.subSrc : this.mainSrc;
    const player = this._playerEl as { src?: string | undefined };
    if (src !== undefined && player.src !== src) {
      player.src = src;
    }
  }

  private _setOrRemove(el: HTMLElement, name: string, value: string | null | undefined): void {
    if (value === undefined || value === null) {
      el.removeAttribute(name);
    } else {
      el.setAttribute(name, value);
    }
  }

  private _destroyPlayer(): void {
    if (this._playerEl && this._playerEl.parentNode) {
      this._playerEl.parentNode.removeChild(this._playerEl);
    }
    this._playerEl = undefined;
  }
}
