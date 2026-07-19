import {
  createPlayer,
  type CheetahPlayer,
  type CheetahPlayerEvent,
  type CompositeRecordingOptions,
  type CompositeRecordingResult,
  type CompositeRecordingProgress,
  type CompositeWatermark,
  type DownloadProgress,
} from '@cheetah-media/web';
import { detectLocale, getMessage, type MessageKey } from './i18n';
import { styles } from './styles';
import { createWatermarkOverlay, parseWatermarks, type Watermark, type WatermarkOverlay } from './watermark';
import { clearMetadataOverlay, parseMetadataPayload, renderMetadataOverlay, type MetadataShape } from './metadata-overlay';

const OBSERVED_ATTRIBUTES = [
  'src',
  'live',
  'autoplay',
  'controls',
  'muted',
  'volume',
  'worker-url',
  'wasm-url',
  'stats-interval',
  'max-event-history',
  'on-disconnect',
  'watermarks',
  'recordingactive',
  'download',
] as const;

type ObservedAttribute = (typeof OBSERVED_ATTRIBUTES)[number];

interface ErrorDetail {
  readonly code: number;
  readonly stage: string;
  readonly message: string;
  readonly recoverable: boolean;
}

export class CheetahPlayerElement extends HTMLElement {
  static get observedAttributes(): readonly string[] {
    return OBSERVED_ATTRIBUTES as unknown as readonly string[];
  }

  private _player: CheetahPlayer | undefined;
  private _connected = false;
  private _loadedSrc: string | undefined;
  private _loading = false;
  private _loadGeneration = 0;
  private _recording = false;
  private _locale = detectLocale();
  private _lastError: ErrorDetail | undefined;
  private _autoplayTimer: ReturnType<typeof setTimeout> | undefined;
  private _resizeObserver: ResizeObserver | undefined;
  private _video: HTMLVideoElement | undefined;

  private _playButton!: HTMLButtonElement;
  private _muteButton!: HTMLButtonElement;
  private _snapshotButton!: HTMLButtonElement;
  private _downloadButton!: HTMLButtonElement;
  private _recordButton!: HTMLButtonElement;
  private _fullscreenButton!: HTMLButtonElement;
  private _volumeSlider!: HTMLInputElement;
  private _overlay!: HTMLDivElement;
  private _overlayMessage!: HTMLDivElement;
  private _retryButton!: HTMLButtonElement;
  private _autoplayButton!: HTMLButtonElement;
  private _status!: HTMLDivElement;
  private _liveRegion!: HTMLDivElement;
  private _watermarkOverlay!: WatermarkOverlay;
  private _metadataSvg!: SVGSVGElement;

  get src(): string | undefined {
    return this.getAttribute('src') ?? undefined;
  }

  set src(value: string | undefined) {
    if (value === undefined) {
      this.removeAttribute('src');
    } else {
      this.setAttribute('src', value);
    }
  }

  get live(): boolean {
    return this.hasAttribute('live');
  }

  set live(value: boolean) {
    this.toggleAttribute('live', value);
  }

  get autoplay(): boolean {
    return this.hasAttribute('autoplay');
  }

  set autoplay(value: boolean) {
    this.toggleAttribute('autoplay', value);
  }

  get controls(): boolean {
    return this.hasAttribute('controls');
  }

  set controls(value: boolean) {
    this.toggleAttribute('controls', value);
  }

  get muted(): boolean {
    return this.hasAttribute('muted');
  }

  set muted(value: boolean) {
    this.toggleAttribute('muted', value);
  }

  get volume(): number {
    const raw = this.getAttribute('volume');
    const value = raw === null ? 1 : Number(raw);
    return Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 1;
  }

  set volume(value: number) {
    this.setAttribute('volume', String(Math.min(1, Math.max(0, value))));
  }

  get workerUrl(): string | undefined {
    return this.getAttribute('worker-url') ?? undefined;
  }

  set workerUrl(value: string | undefined) {
    if (value === undefined) this.removeAttribute('worker-url');
    else this.setAttribute('worker-url', value);
  }

  get wasmUrl(): string | undefined {
    return this.getAttribute('wasm-url') ?? undefined;
  }

  set wasmUrl(value: string | undefined) {
    if (value === undefined) this.removeAttribute('wasm-url');
    else this.setAttribute('wasm-url', value);
  }

  get statsIntervalMs(): number | undefined {
    const raw = this.getAttribute('stats-interval');
    if (raw === null) return undefined;
    const value = Number(raw);
    return Number.isFinite(value) && value >= 16 ? value : undefined;
  }

  set statsIntervalMs(value: number | undefined) {
    if (value === undefined) this.removeAttribute('stats-interval');
    else this.setAttribute('stats-interval', String(value));
  }

  get maxEventHistory(): number | undefined {
    const raw = this.getAttribute('max-event-history');
    if (raw === null) return undefined;
    const value = Number(raw);
    return Number.isFinite(value) && value >= 0 ? value : undefined;
  }

  set maxEventHistory(value: number | undefined) {
    if (value === undefined) this.removeAttribute('max-event-history');
    else this.setAttribute('max-event-history', String(value));
  }

  get watermarks(): Watermark[] {
    return parseWatermarks(this.getAttribute('watermarks')) ?? [];
  }

  set watermarks(value: Watermark[]) {
    if (!Array.isArray(value) || value.length === 0) {
      this.removeAttribute('watermarks');
    } else {
      this.setAttribute('watermarks', JSON.stringify(value));
    }
  }

  get recordingactive(): boolean {
    return this.hasAttribute('recordingactive');
  }

  set recordingactive(value: boolean) {
    this.toggleAttribute('recordingactive', value);
  }

  get download(): string | undefined {
    return this.getAttribute('download') ?? undefined;
  }

  set download(value: string | undefined) {
    if (value === undefined || value === '') {
      this.removeAttribute('download');
    } else {
      this.setAttribute('download', value);
    }
  }

  get onDisconnect(): 'stop' | 'destroy' {
    const value = this.getAttribute('on-disconnect');
    return value === 'destroy' ? 'destroy' : 'stop';
  }

  set onDisconnect(value: 'stop' | 'destroy') {
    this.setAttribute('on-disconnect', value);
  }

  setWatermarks(watermarks: Watermark[]): void {
    this.watermarks = watermarks;
  }

  get player(): CheetahPlayer | undefined {
    return this._player;
  }

  get locale(): string {
    return this._locale;
  }

  set locale(value: string) {
    this._locale = value;
  }

  connectedCallback(): void {
    this._connected = true;
    if (!this.shadowRoot) {
      this._buildShadowRoot();
    }
    this.setAttribute('tabindex', this.getAttribute('tabindex') ?? '0');
    this.setAttribute('role', 'application');
    this.setAttribute('aria-label', getMessage(this._locale, 'controls'));
    this._setupResizeObserver();
    this._bindKeyboard();

    if (this.src && (!this._player || this._loadedSrc !== this.src)) {
      void this._load(this.src);
    }
  }

  disconnectedCallback(): void {
    this._connected = false;
    this._clearAutoplayTimer();
    this._disconnectKeyboard();
    this._disconnectResizeObserver();

    if (this._player) {
      void this._cleanupPlayer();
    }
    this._updateState('idle');
  }

  private async _cleanupPlayer(forceDestroy = false): Promise<void> {
    const player = this._player;
    if (!player) return;
    this._player = undefined;
    this._loadedSrc = undefined;
    try {
      if (forceDestroy || this.onDisconnect === 'destroy') {
        await player.destroy();
      } else {
        try {
          await player.stop();
        } catch {
          // A player that never started a worker may reject stop; destroy it.
          await player.destroy();
        }
      }
    } catch {
      // Final cleanup is best-effort.
    }
  }

  adoptedCallback(): void {
    // No-op: element may move documents; player lifecycle is preserved.
  }

  attributeChangedCallback(name: ObservedAttribute, oldValue: string | null, newValue: string | null): void {
    if (oldValue === newValue) return;

    if (name === 'src') {
      const src = newValue || undefined;
      if (this._connected) {
        void this._load(src);
      }
      return;
    }

    if (name === 'live' && newValue !== oldValue && this._connected && this.src) {
      // Live flag changes require a reload to take effect on the transport.
      // Reset loaded state so the same URL can be reloaded with the new transport config.
      this._loadedSrc = undefined;
      this._loading = false;
      void this._load(this.src);
      return;
    }

    if (name === 'volume' || name === 'muted') {
      if (this._video) {
        this._video.muted = this.muted;
        this._video.volume = this.volume;
      }
      this._updateVolumeUI();
      return;
    }

    if (name === 'controls') {
      // Controls visibility is handled entirely by CSS via :host([controls]).
      return;
    }

    if (name === 'watermarks') {
      this._updateWatermarks();
      return;
    }

    if (name === 'download') {
      if (this._downloadButton) {
        this._downloadButton.style.display = newValue ? '' : 'none';
      }
      return;
    }
  }

  private _buildShadowRoot(): void {
    const shadow = this.attachShadow({ mode: 'open' });

    const style = document.createElement('style');
    style.textContent = styles;
    shadow.appendChild(style);

    const surface = document.createElement('div');
    surface.className = 'surface';
    surface.setAttribute('part', 'surface');
    const slot = document.createElement('slot');
    slot.name = 'surface';
    // Default media surface for the MSE playback session. A light-DOM
    // slotted surface (video/canvas) still wins for snapshot/composite when present.
    const video = document.createElement('video');
    video.setAttribute('part', 'video');
    video.playsInline = true;
    video.muted = this.muted;
    video.style.width = '100%';
    video.style.height = '100%';
    video.style.objectFit = 'contain';
    video.style.background = '#000';
    this._video = video;
    surface.appendChild(video);
    surface.appendChild(slot);
    shadow.appendChild(surface);

    this._overlay = document.createElement('div');
    this._overlay.className = 'overlay';
    this._overlay.setAttribute('part', 'overlay');

    this._overlayMessage = document.createElement('div');
    this._overlayMessage.className = 'overlay-message';
    this._overlay.appendChild(this._overlayMessage);

    this._retryButton = this._createButton(getMessage(this._locale, 'retry'), 'overlay-button', () => this._retry());
    this._retryButton.setAttribute('part', 'overlay-button');
    this._overlay.appendChild(this._retryButton);

    this._autoplayButton = this._createButton(getMessage(this._locale, 'play'), 'overlay-button', () => this._handleAutoplayClick());
    this._autoplayButton.setAttribute('part', 'overlay-button');
    this._overlay.appendChild(this._autoplayButton);

    shadow.appendChild(this._overlay);

    this._watermarkOverlay = createWatermarkOverlay();
    shadow.appendChild(this._watermarkOverlay.root);

    this._metadataSvg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    this._metadataSvg.classList.add('overlay-svg');
    this._metadataSvg.setAttribute('part', 'overlay-svg');
    this._metadataSvg.setAttribute('viewBox', '0 0 1 1');
    this._metadataSvg.setAttribute('preserveAspectRatio', 'xMidYMid meet');
    this._metadataSvg.setAttribute('aria-hidden', 'true');
    shadow.appendChild(this._metadataSvg);

    const controls = document.createElement('div');
    controls.className = 'controls';
    controls.setAttribute('part', 'controls');
    controls.setAttribute('role', 'toolbar');
    controls.setAttribute('aria-label', getMessage(this._locale, 'controls'));

    this._playButton = this._createButton(getMessage(this._locale, 'play'), 'control-button play-button', () => this._togglePlay());
    controls.appendChild(this._playButton);

    this._muteButton = this._createButton(getMessage(this._locale, 'mute'), 'control-button mute-button', () => this._toggleMute());
    controls.appendChild(this._muteButton);

    this._volumeSlider = document.createElement('input');
    this._volumeSlider.type = 'range';
    this._volumeSlider.min = '0';
    this._volumeSlider.max = '1';
    this._volumeSlider.step = '0.05';
    this._volumeSlider.value = String(this.volume);
    this._volumeSlider.className = 'control-button volume-slider';
    this._volumeSlider.setAttribute('part', 'volume-slider');
    this._volumeSlider.setAttribute('aria-label', getMessage(this._locale, 'volume'));
    this._volumeSlider.addEventListener('input', () => this._onVolumeInput());
    controls.appendChild(this._volumeSlider);

    this._snapshotButton = this._createButton(getMessage(this._locale, 'snapshot'), 'control-button snapshot-button', () => this._takeSnapshot());
    controls.appendChild(this._snapshotButton);

    this._downloadButton = this._createButton(getMessage(this._locale, 'downloadStart'), 'control-button download-button', () => this._toggleDownload());
    this._downloadButton.style.display = this.download ? '' : 'none';
    controls.appendChild(this._downloadButton);

    this._recordButton = this._createButton(getMessage(this._locale, 'recordStart'), 'control-button record-button', () => this._toggleRecording());
    controls.appendChild(this._recordButton);

    this._fullscreenButton = this._createButton(getMessage(this._locale, 'fullscreen'), 'control-button fullscreen-button', () => this._toggleFullscreen());
    controls.appendChild(this._fullscreenButton);

    this._status = document.createElement('div');
    this._status.className = 'status';
    this._status.setAttribute('part', 'status');
    this._status.setAttribute('aria-live', 'off');
    controls.appendChild(this._status);

    shadow.appendChild(controls);

    this._liveRegion = document.createElement('div');
    this._liveRegion.className = 'sr-only';
    this._liveRegion.setAttribute('aria-live', 'polite');
    this._liveRegion.setAttribute('aria-atomic', 'true');
    shadow.appendChild(this._liveRegion);

    // The watermarks attribute may have been processed before the overlay
    // existed; refresh it now that the shadow root is built.
    this._updateWatermarks();
  }

  private _createButton(label: string, className: string, onClick: () => void): HTMLButtonElement {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = className;
    button.setAttribute('part', className.split(' ')[1] ?? 'button');
    button.setAttribute('aria-label', label);
    button.textContent = label;
    button.addEventListener('click', onClick);
    return button;
  }

  private _bindKeyboard(): void {
    this.addEventListener('keydown', this._onKeyDown);
  }

  private _disconnectKeyboard(): void {
    this.removeEventListener('keydown', this._onKeyDown);
  }

  private _onKeyDown = (event: KeyboardEvent): void => {
    if (event.altKey || event.ctrlKey || event.metaKey) return;

    switch (event.code) {
      case 'Space':
      case 'KeyK':
        event.preventDefault();
        this._togglePlay();
        return;
      case 'KeyF':
        event.preventDefault();
        this._toggleFullscreen();
        return;
      case 'KeyM':
        event.preventDefault();
        this._toggleMute();
        return;
      case 'KeyS':
        event.preventDefault();
        this._takeSnapshot();
        return;
      case 'KeyR':
        event.preventDefault();
        this._toggleRecording();
        return;
      case 'ArrowUp':
        event.preventDefault();
        this.volume = Math.min(1, this.volume + 0.1);
        return;
      case 'ArrowDown':
        event.preventDefault();
        this.volume = Math.max(0, this.volume - 0.1);
        return;
      default:
        return;
    }
  };

  private _setupResizeObserver(): void {
    if (typeof ResizeObserver === 'undefined') return;
    this._resizeObserver = new ResizeObserver((entries) => this._onResize(entries));
    this._resizeObserver.observe(this);
  }

  private _disconnectResizeObserver(): void {
    if (this._resizeObserver) {
      this._resizeObserver.disconnect();
      this._resizeObserver = undefined;
    }
  }

  private _pendingResizeFrame: ReturnType<typeof requestAnimationFrame> | undefined;
  private _pendingResizeEntry: ResizeObserverEntry | undefined;

  private _onResize(entries: ResizeObserverEntry[]): void {
    const entry = entries[0];
    if (!entry) return;
    this._pendingResizeEntry = entry;
    if (this._pendingResizeFrame) return;
    this._pendingResizeFrame = requestAnimationFrame(() => {
      this._pendingResizeFrame = undefined;
      const pending = this._pendingResizeEntry;
      this._pendingResizeEntry = undefined;
      if (!pending) return;
      const { width, height } = pending.contentRect;
      const widthProp = `${width}px`;
      const heightProp = `${height}px`;
      // Avoid re-triggering the ResizeObserver by only updating when changed.
      if (this.style.getPropertyValue('--surface-width') !== widthProp) {
        this.style.setProperty('--surface-width', widthProp);
      }
      if (this.style.getPropertyValue('--surface-height') !== heightProp) {
        this.style.setProperty('--surface-height', heightProp);
      }
    });
  }

  private _buildConfig(): import('@cheetah-media/web').PlayerConfig {
    const runtimeConfig: { workerUrl?: string; wasmUrl?: string } = {};
    if (this.workerUrl) runtimeConfig.workerUrl = this.workerUrl;
    if (this.wasmUrl) runtimeConfig.wasmUrl = this.wasmUrl;

    const diagnosticsConfig: { statsIntervalMs?: number; maxEventHistory?: number } = {};
    if (this.statsIntervalMs !== undefined) diagnosticsConfig.statsIntervalMs = this.statsIntervalMs;
    if (this.maxEventHistory !== undefined) diagnosticsConfig.maxEventHistory = this.maxEventHistory;

    const audioConfig: { enabled?: boolean; volume?: number } = {};
    if (this.muted) audioConfig.enabled = false;
    if (this.volume !== 1) audioConfig.volume = this.volume;

    return {
      transport: { lowLatency: this.live },
      ...(Object.keys(runtimeConfig).length > 0 ? { runtime: runtimeConfig } : {}),
      ...(Object.keys(diagnosticsConfig).length > 0 ? { diagnostics: diagnosticsConfig } : {}),
      ...(Object.keys(audioConfig).length > 0 ? { audio: audioConfig } : {}),
    };
  }

  private async _load(src: string | undefined): Promise<void> {
    if (!this.shadowRoot) {
      this._buildShadowRoot();
    }

    if (this._loading && this._loadedSrc === src) {
      return;
    }

    if (!src) {
      this._updateState('idle');
      if (this._player) {
        await this._cleanupPlayer();
      }
      this._loading = false;
      this._loadedSrc = undefined;
      return;
    }

    if (this._loadedSrc === src && this._player && this.getAttribute('data-state') !== 'failed') {
      return;
    }

    const generation = ++this._loadGeneration;

    if (this._player) {
      await this._cleanupPlayer(true);
    }

    if (this._loadGeneration !== generation || !this._connected) {
      // A newer _load replaced us, or the element was disconnected during cleanup.
      return;
    }

    this._loadedSrc = src;
    this._loading = true;
    this._lastError = undefined;
    this._updateState('loading');

    const config = this._buildConfig();
    const player = createPlayer(config);
    this._player = player;
    this._bindPlayer();
    if (this._video) {
      this._video.muted = this.muted;
      this._video.volume = this.volume;
      player.attachMediaElement(this._video);
    }

    try {
      await player.load(src, { isLive: this.live });
      if (this._loadGeneration !== generation || this._player !== player || !this._connected) {
        // Superseded or disconnected while loading; discard the stale player without
        // touching the current element state, which belongs to a newer load.
        await player.destroy().catch(() => undefined);
        return;
      }
      this._loading = false;
      if (this.autoplay) {
        this._tryAutoplay();
      }
    } catch (cause) {
      if (this._loadGeneration !== generation || this._player !== player || !this._connected) {
        // Superseded or disconnected while loading; discard the stale player without
        // touching the current element state, which belongs to a newer load.
        await player.destroy().catch(() => undefined);
        return;
      }
      this._loading = false;
      this._lastError = {
        code: 6999,
        stage: 'component',
        message: cause instanceof Error ? cause.message : 'Load failed',
        recoverable: true,
      };
      try {
        await player.destroy();
      } catch {
        // ignored
      }
      this._player = undefined;
      this._loadedSrc = undefined;
      this._updateState('failed');
    }
  }

  private _bindPlayer(): void {
    if (!this._player) return;

    this._player.addEventListener('statechange', (event) => this._onStateChange(event as CheetahPlayerEvent<'statechange'>));
    this._player.addEventListener('error', (event) => this._onError(event as CheetahPlayerEvent<'error'>));
    this._player.addEventListener('stats', (event) => this._onStats(event as CheetahPlayerEvent<'stats'>));
    this._player.addEventListener('firstframe', (event) => this._dispatch('firstframe', event));
    this._player.addEventListener('tracks', (event) => this._dispatch('tracks', event));
    this._player.addEventListener('backendchange', (event) => this._dispatch('backendchange', event));
    this._player.addEventListener('variantchange', (event) => this._dispatch('variantchange', event));
    this._player.addEventListener('buffering', (event) => this._dispatch('buffering', event));
    this._player.addEventListener('warning', (event) => this._dispatch('warning', event));
    this._player.addEventListener('recording', (event) => this._dispatch('recording', event));
    this._player.addEventListener('compositeRecording', (event) => this._onCompositeRecording(event as CheetahPlayerEvent<'compositeRecording'>));
    this._player.addEventListener('download', (event) => this._onDownload(event as CheetahPlayerEvent<'download'>));
    this._player.addEventListener('metadata', (event) => this._onMetadata(event as CheetahPlayerEvent<'metadata'>));
  }

  private _onStateChange(event: CheetahPlayerEvent<'statechange'>): void {
    if (!this._connected || !this._player || this._player.id !== event.playerId) return;
    const to = typeof event.details?.to === 'string' ? (event.details.to as import('@cheetah-media/web').PlayerState) : 'idle';
    this._updateState(to);
    if (to === 'playing') {
      this._clearAutoplayTimer();
      this._hideOverlay();
    }
  }

  private _onError(event: CheetahPlayerEvent<'error'>): void {
    if (!this._connected || !this._player || this._player.id !== event.playerId) return;
    const error = event.details?.error as ErrorDetail | undefined;
    this._lastError = error;
    this._updateState('failed');
    this._dispatch('error', event);
  }

  private _onStats(event: CheetahPlayerEvent<'stats'>): void {
    const details = event.details ?? {};
    const latencyMs = typeof details.latencyMs === 'number' ? details.latencyMs : undefined;
    const bufferedMs = typeof details.bufferedMs === 'number' ? details.bufferedMs : undefined;

    const latencyText = latencyMs !== undefined ? `${latencyMs.toFixed(0)}ms` : '-';
    const bufferedText = bufferedMs !== undefined ? `${bufferedMs.toFixed(0)}ms` : '-';
    this._status.textContent = `${getMessage(this._locale, 'latencyStatus')}: ${latencyText} | ${getMessage(this._locale, 'buffered')}: ${bufferedText}`;

    this._dispatch('stats', event);
  }

  private _onCompositeRecording(event: CheetahPlayerEvent<'compositeRecording'>): void {
    const details = event.details ?? {};
    const active = details.active === true && details.result === undefined;
    this._recording = active;
    this.recordingactive = active;
    this._recordButton.textContent = active
      ? getMessage(this._locale, 'recordStop')
      : getMessage(this._locale, 'recordStart');
    this._recordButton.setAttribute('aria-label', this._recordButton.textContent);

    const progress = details.progress as CompositeRecordingProgress | undefined;
    this.dispatchEvent(
      new CustomEvent('recordingprogress', {
        detail: { progress, result: details.result as CompositeRecordingResult | undefined, error: details.error, playerEvent: event },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private _onDownload(event: CheetahPlayerEvent<'download'>): void {
    const details = event.details ?? {};
    const active = details.active === true;
    const progress = details.progress as DownloadProgress | undefined;
    this._downloadButton.textContent = active
      ? getMessage(this._locale, 'downloadStop')
      : getMessage(this._locale, 'downloadStart');
    this._downloadButton.setAttribute('aria-label', this._downloadButton.textContent);

    this.dispatchEvent(
      new CustomEvent('downloadprogress', {
        detail: { progress, completed: details.completed === true, error: details.error, playerEvent: event },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private _onMetadata(event: CheetahPlayerEvent<'metadata'>): void {
    const details = event.details ?? {};
    let shapes: MetadataShape[] = [];

    if (Array.isArray(details.items)) {
      for (const item of details.items) {
        if (item && typeof item === 'object') {
          const value = (item as Record<string, unknown>).value;
          shapes = shapes.concat(parseMetadataPayload(value));
        }
      }
    } else if (details.shapes !== undefined) {
      shapes = parseMetadataPayload(details.shapes);
    }

    if (shapes.length === 0) {
      clearMetadataOverlay(this._metadataSvg);
    } else {
      renderMetadataOverlay(shapes, this._metadataSvg);
    }

    this._dispatch('metadata', event);
  }

  private _updateState(state: import('@cheetah-media/web').PlayerState | 'autoplay-blocked'): void {
    this.setAttribute('data-state', state);
    this._updatePlayButton(state);
    this._updateVolumeUI();

    if (state === 'loading') {
      this._showOverlay(getMessage(this._locale, 'loading'), { retry: false, autoplay: false });
    } else if (state === 'preroll') {
      this._showOverlay(getMessage(this._locale, 'preroll'), { retry: false, autoplay: false });
    } else if (state === 'rebuffering') {
      this._showOverlay(getMessage(this._locale, 'rebuffering'), { retry: false, autoplay: false });
    } else if (state === 'failed') {
      const message = this._lastError?.message ?? getMessage(this._locale, 'failed');
      const retry = this._lastError?.recoverable ?? false;
      this._showOverlay(message, { retry, autoplay: false });
    } else if (state === 'autoplay-blocked') {
      this._showOverlay(getMessage(this._locale, 'autoplayBlocked'), { retry: false, autoplay: true });
    } else {
      this._hideOverlay();
    }

    const announceKey = state === 'autoplay-blocked' ? 'autoplayBlocked' : state;
    this._announce(getMessage(this._locale, announceKey as MessageKey));
  }

  private _updatePlayButton(state: string): void {
    if (state === 'playing') {
      this._playButton.textContent = getMessage(this._locale, 'pause');
      this._playButton.setAttribute('aria-label', getMessage(this._locale, 'pause'));
    } else {
      this._playButton.textContent = getMessage(this._locale, 'play');
      this._playButton.setAttribute('aria-label', getMessage(this._locale, 'play'));
    }
  }

  private _updateVolumeUI(): void {
    if (this._volumeSlider) {
      this._volumeSlider.value = String(this.volume);
    }
    if (this._muteButton) {
      const label = this.muted ? getMessage(this._locale, 'unmute') : getMessage(this._locale, 'mute');
      this._muteButton.textContent = label;
      this._muteButton.setAttribute('aria-label', label);
    }
  }

  private _showOverlay(message: string, options: { retry: boolean; autoplay: boolean }): void {
    this._overlayMessage.textContent = message;
    this._retryButton.style.display = options.retry ? '' : 'none';
    this._autoplayButton.style.display = options.autoplay ? '' : 'none';
    this._overlay.classList.add('active');
  }

  private _hideOverlay(): void {
    this._overlay.classList.remove('active');
  }

  private _updateWatermarks(): void {
    if (this._watermarkOverlay) {
      this._watermarkOverlay.setWatermarks(this.watermarks);
    }
  }

  private _announce(text: string): void {
    if (this._liveRegion) {
      this._liveRegion.textContent = text;
    }
  }

  private _dispatch(type: string, event: CheetahPlayerEvent): void {
    this.dispatchEvent(new CustomEvent(type, { detail: event, bubbles: true, composed: true }));
  }

  private _togglePlay(): void {
    if (!this._player) return;
    const state = this._player.state;
    // Ignore play/pause toggles while the player is still bootstrapping; the
    // runtime may not be ready and key presses during this window should not crash.
    if (state === 'loading' || state === 'preroll' || state === 'idle') return;
    if (state === 'playing') {
      this._player.pause();
    } else {
      this._player.play();
    }
  }

  private _toggleMute(): void {
    this.muted = !this.muted;
    this._updateVolumeUI();
  }

  private _onVolumeInput(): void {
    const value = Number(this._volumeSlider.value);
    if (Number.isFinite(value)) {
      this.volume = value;
      this._updateVolumeUI();
    }
  }

  private async _takeSnapshot(): Promise<void> {
    if (!this._player) return;
    try {
      const imageData = await this._player.snapshot({
        maxWidth: this.clientWidth,
        maxHeight: this.clientHeight,
      });
      this.dispatchEvent(
        new CustomEvent('snapshot', { detail: { imageData }, bubbles: true, composed: true }),
      );
    } catch (cause) {
      // Snapshot failures are emitted by the player as error events.
    }
  }

  private _recordingSource(): HTMLVideoElement | HTMLCanvasElement | undefined {
    const surface =
      this.querySelector('[slot="surface"]') ??
      this.querySelector('video, canvas') ??
      this._video;
    if (surface instanceof HTMLVideoElement || surface instanceof HTMLCanvasElement) {
      return surface;
    }
    return undefined;
  }

  private async _loadImage(url: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error(`Failed to load watermark image: ${url}`));
      img.src = url;
    });
  }

  private async _buildCompositeWatermarks(
    source: HTMLVideoElement | HTMLCanvasElement,
  ): Promise<CompositeWatermark[] | undefined> {
    const marks = this.watermarks;
    if (marks.length === 0) return undefined;
    const width = source instanceof HTMLVideoElement ? source.videoWidth || source.clientWidth : source.width;
    const height = source instanceof HTMLVideoElement ? source.videoHeight || source.clientHeight : source.height;
    const outW = width || this.clientWidth || 640;
    const outH = height || this.clientHeight || 480;
    const result: CompositeWatermark[] = [];
    for (const mark of marks) {
      const x = (mark.x ?? 0) / 100 * outW;
      const y = (mark.y ?? 0) / 100 * outH;
      if (mark.type === 'text') {
        result.push({
          type: 'text',
          text: mark.content,
          x,
          y,
          font: (mark as { font?: string }).font ?? '24px sans-serif',
          color: (mark as { color?: string }).color ?? 'rgba(255,255,255,0.8)',
        });
      } else if (mark.type === 'image') {
        const img = await this._loadImage(mark.content);
        const w = this._parseLength(mark.width, outW);
        const h = this._parseLength(mark.height, outH);
        result.push({
          type: 'image',
          image: img,
          x,
          y,
          width: w ?? img.naturalWidth,
          height: h ?? img.naturalHeight,
        });
      }
    }
    return result.length > 0 ? result : undefined;
  }

  private _parseLength(value: string | undefined, base: number): number | undefined {
    if (value === undefined) return undefined;
    const trimmed = value.trim();
    if (trimmed.endsWith('%')) {
      const n = Number(trimmed.slice(0, -1));
      return Number.isFinite(n) ? (n / 100) * base : undefined;
    }
    const n = Number(trimmed.replace(/px$/i, ''));
    return Number.isFinite(n) ? n : undefined;
  }

  private async _toggleRecording(): Promise<void> {
    if (!this._player) return;
    try {
      if (this._recording) {
        await this._player.stopCompositeRecording();
        this._recording = false;
        this.recordingactive = false;
      } else {
        const source = this._recordingSource();
        if (!source) {
          throw new Error('No video or canvas surface available for composite recording');
        }
        const watermarks = await this._buildCompositeWatermarks(source);
        const opts: CompositeRecordingOptions = {
          source,
          filename: 'recording',
          ...(watermarks !== undefined ? { watermarks } : {}),
        };
        await this._player.startCompositeRecording(opts);
        this._recording = true;
        this.recordingactive = true;
      }
      this._recordButton.textContent = this._recording
        ? getMessage(this._locale, 'recordStop')
        : getMessage(this._locale, 'recordStart');
      this._recordButton.setAttribute('aria-label', this._recordButton.textContent);
    } catch (cause) {
      this._recording = false;
      this.recordingactive = false;
    }
  }

  private async _toggleDownload(): Promise<void> {
    if (!this._player) return;
    try {
      if (this._player.downloadActive) {
        await this._player.stopDownload();
      } else {
        const url = this.download;
        if (!url) {
          throw new Error('No download URL set on the player element');
        }
        await this._player.startDownload({ url, filename: 'download' });
      }
    } catch (cause) {
      // Download failures are emitted by the player as error events.
    }
  }

  private _toggleFullscreen(): void {
    if (typeof this.requestFullscreen !== 'function') return;
    if (document.fullscreenElement === this) {
      void document.exitFullscreen();
    } else {
      void this.requestFullscreen();
    }
  }

  private _tryAutoplay(): void {
    if (!this._player || this._player.state === 'playing') return;
    this._player.play();
    this._clearAutoplayTimer();
    this._autoplayTimer = setTimeout(() => {
      if (this._connected && this._player && this._player.state !== 'playing') {
        this._updateState('autoplay-blocked');
      }
    }, 1000);
  }

  private _handleAutoplayClick(): void {
    this._hideOverlay();
    if (this._player) {
      this._player.play();
    }
  }

  private _retry(): void {
    if (this.src) {
      void this._load(this.src);
    }
  }

  private _clearAutoplayTimer(): void {
    if (this._autoplayTimer !== undefined) {
      clearTimeout(this._autoplayTimer);
      this._autoplayTimer = undefined;
    }
  }
}
