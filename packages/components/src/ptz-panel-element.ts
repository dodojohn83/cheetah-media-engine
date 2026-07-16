import { createGb28181PtzCmd, type PtzAction, type PtzCommand } from '@cheetah-media/web';
import { detectLocale, getMessage, type MessageKey } from './i18n';

const OBSERVED_ATTRIBUTES = ['speed', 'auto-stop', 'target'] as const;

type ObservedAttribute = (typeof OBSERVED_ATTRIBUTES)[number];

export interface PtzEventDetail {
  readonly ptzCmd: string;
  readonly action: PtzAction;
  readonly speeds: PtzCommand['speeds'];
  readonly protocol: 'gb28181';
}

/**
 * A self-contained PTZ (pan-tilt-zoom) control panel.
 *
 * The panel generates GB28181 `PTZCmd` payloads and dispatches a `ptz` custom
 * event. When a `target` selector is provided, it also forwards the command to
 * the matching `cheetah-player` element via its `ptz()` API.
 */
export class CheetahPtzPanelElement extends HTMLElement {
  static get observedAttributes(): readonly string[] {
    return OBSERVED_ATTRIBUTES as unknown as readonly string[];
  }

  private _locale = detectLocale();
  private _connected = false;
  private _activeAction: PtzAction | undefined;
  private _stopTimer: ReturnType<typeof setTimeout> | undefined;

  get target(): string | undefined {
    return this.getAttribute('target') ?? undefined;
  }

  set target(value: string | undefined) {
    if (value === undefined) this.removeAttribute('target');
    else this.setAttribute('target', value);
  }

  get speed(): number {
    const raw = this.getAttribute('speed');
    const value = raw === null ? 128 : Number(raw);
    return Number.isFinite(value) ? Math.max(0, Math.min(255, Math.round(value))) : 128;
  }

  set speed(value: number) {
    this.setAttribute('speed', String(Math.max(0, Math.min(255, Math.round(value)))));
  }

  get autoStop(): boolean {
    return this.hasAttribute('auto-stop');
  }

  set autoStop(value: boolean) {
    this.toggleAttribute('auto-stop', value);
  }

  connectedCallback(): void {
    this._connected = true;
    if (!this.shadowRoot) {
      this._buildShadowRoot();
    }
  }

  disconnectedCallback(): void {
    this._connected = false;
    this._activeAction = undefined;
    if (this._stopTimer) {
      clearTimeout(this._stopTimer);
      this._stopTimer = undefined;
    }
  }

  attributeChangedCallback(name: ObservedAttribute, oldValue: string | null, newValue: string | null): void {
    if (!this._connected || oldValue === newValue) return;
    if (name === 'speed') {
      const input = this.shadowRoot?.querySelector('input[name="speed"]') as HTMLInputElement | undefined;
      if (input) input.value = String(this.speed);
    }
  }

  private _buildShadowRoot(): void {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = this._styles();

    const container = document.createElement('div');
    container.className = 'ptz-panel';
    container.setAttribute('role', 'group');
    container.setAttribute('aria-label', getMessage(this._locale, 'ptzTitle'));

    const speedRow = document.createElement('div');
    speedRow.className = 'ptz-row';
    const speedLabel = document.createElement('label');
    speedLabel.textContent = 'Speed';
    const speedInput = document.createElement('input');
    speedInput.type = 'range';
    speedInput.min = '0';
    speedInput.max = '255';
    speedInput.value = String(this.speed);
    speedInput.setAttribute('part', 'speed-slider');
    speedInput.name = 'speed';
    speedInput.addEventListener('input', () => {
      this.speed = Number(speedInput.value);
    });
    speedRow.appendChild(speedLabel);
    speedRow.appendChild(speedInput);
    container.appendChild(speedRow);

    const pad = document.createElement('div');
    pad.className = 'ptz-pad';

    const directions: [PtzAction, MessageKey, string][] = [
      ['upLeft', 'ptzUp', '↖'],
      ['up', 'ptzUp', '↑'],
      ['upRight', 'ptzUp', '↗'],
      ['left', 'ptzLeft', '←'],
      ['stop', 'ptzStop', '●'],
      ['right', 'ptzRight', '→'],
      ['downLeft', 'ptzDown', '↙'],
      ['down', 'ptzDown', '↓'],
      ['downRight', 'ptzDown', '↘'],
    ];

    for (const [action, key, symbol] of directions) {
      const btn = this._createButton(symbol, getMessage(this._locale, key), `ptz-${action}`);
      this._bindPress(btn, action);
      pad.appendChild(btn);
    }
    container.appendChild(pad);

    const zoomRow = document.createElement('div');
    zoomRow.className = 'ptz-row';
    const zoomIn = this._createButton('+', getMessage(this._locale, 'ptzZoomIn'), 'ptz-zoom-in');
    this._bindPress(zoomIn, 'zoomIn');
    const zoomOut = this._createButton('-', getMessage(this._locale, 'ptzZoomOut'), 'ptz-zoom-out');
    this._bindPress(zoomOut, 'zoomOut');
    zoomRow.appendChild(zoomOut);
    zoomRow.appendChild(zoomIn);
    container.appendChild(zoomRow);

    const presetRow = document.createElement('div');
    presetRow.className = 'ptz-row';
    const presetInput = document.createElement('input');
    presetInput.type = 'number';
    presetInput.min = '1';
    presetInput.max = '255';
    presetInput.value = '1';
    presetInput.setAttribute('part', 'preset-number');
    presetInput.setAttribute('aria-label', getMessage(this._locale, 'ptzPresetNumber'));
    presetRow.appendChild(presetInput);

    const setBtn = this._createButton(getMessage(this._locale, 'ptzPresetSet'), getMessage(this._locale, 'ptzPresetSet'), 'ptz-preset-set');
    setBtn.addEventListener('click', () => this._sendPreset('presetSet', presetInput));
    presetRow.appendChild(setBtn);

    const callBtn = this._createButton(getMessage(this._locale, 'ptzPresetCall'), getMessage(this._locale, 'ptzPresetCall'), 'ptz-preset-call');
    callBtn.addEventListener('click', () => this._sendPreset('presetCall', presetInput));
    presetRow.appendChild(callBtn);

    const delBtn = this._createButton(getMessage(this._locale, 'ptzPresetDelete'), getMessage(this._locale, 'ptzPresetDelete'), 'ptz-preset-delete');
    delBtn.addEventListener('click', () => this._sendPreset('presetDel', presetInput));
    presetRow.appendChild(delBtn);
    container.appendChild(presetRow);

    shadow.appendChild(container);

    this.addEventListener('keydown', this._onKeyDown);
  }

  private _styles(): string {
    return `<style>
      :host {
        display: inline-block;
        font-family: system-ui, sans-serif;
        --ptz-bg: #222;
        --ptz-fg: #fff;
        --ptz-accent: #0af;
      }
      .ptz-panel {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
        padding: 0.5rem;
        background: var(--ptz-bg);
        color: var(--ptz-fg);
        border-radius: 0.25rem;
      }
      .ptz-row {
        display: flex;
        gap: 0.5rem;
        align-items: center;
      }
      .ptz-row input {
        flex: 1;
      }
      .ptz-pad {
        display: grid;
        grid-template-columns: repeat(3, 2rem);
        grid-template-rows: repeat(3, 2rem);
        gap: 0.25rem;
        justify-content: center;
      }
      button {
        background: #333;
        color: inherit;
        border: 1px solid #555;
        border-radius: 0.25rem;
        cursor: pointer;
        font-size: 1rem;
        line-height: 1;
      }
      button:active {
        background: var(--ptz-accent);
        color: #000;
      }
      button[part="ptz-stop"] {
        background: #600;
      }
      input[type="number"] {
        width: 4rem;
      }
    </style>`;
  }

  private _createButton(label: string, ariaLabel: string, part: string): HTMLButtonElement {
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = label;
    button.setAttribute('part', part);
    button.setAttribute('aria-label', ariaLabel);
    return button;
  }

  private _bindPress(button: HTMLButtonElement, action: PtzAction): void {
    const start = (): void => {
      this._activeAction = action;
      this._send(action);
    };
    const end = (): void => {
      if (this._activeAction === action && this.autoStop && action !== 'stop') {
        this._send('stop');
      }
      this._activeAction = undefined;
    };

    button.addEventListener('mousedown', start);
    button.addEventListener('touchstart', (e) => {
      e.preventDefault();
      start();
    });
    button.addEventListener('mouseup', end);
    button.addEventListener('mouseleave', end);
    button.addEventListener('touchend', end);
    button.addEventListener('touchcancel', end);
  }

  private _sendPreset(action: PtzAction, input: HTMLInputElement): void {
    const point = Math.max(1, Math.min(255, Number(input.value)));
    if (!Number.isFinite(point)) return;
    input.value = String(point);
    this._send(action, point);
  }

  private _send(action: PtzAction, presetPoint?: number): void {
    const isZoom = action === 'zoomIn' || action === 'zoomOut';

    const command: PtzCommand = {
      action,
      channel: 1,
      speeds: isZoom
        ? { zoom: this.speed }
        : action === 'stop'
          ? {}
          : { horizontal: this.speed, vertical: this.speed },
      ...(presetPoint !== undefined ? { presetPoint } : {}),
    };

    try {
      const ptzCmd = createGb28181PtzCmd(command);
      const detail: PtzEventDetail = {
        ptzCmd,
        action,
        speeds: command.speeds,
        protocol: 'gb28181',
      };

      const target = this._resolveTarget();
      const ptzMethod =
        target === undefined
          ? undefined
          : (target as unknown as { ptz?: (cmd: PtzCommand) => Promise<void> | void }).ptz;
      if (target !== undefined && typeof ptzMethod === 'function') {
        void Promise.resolve(ptzMethod.call(target, command)).catch(() => undefined);
      }

      this.dispatchEvent(
        new CustomEvent('ptz', {
          detail,
          bubbles: true,
          composed: true,
        }),
      );
    } catch {
      // Invalid preset point or malformed command; ignore.
    }
  }

  private _resolveTarget(): HTMLElement | undefined {
    const selector = this.target;
    if (!selector) return undefined;
    if (typeof document === 'undefined') return undefined;
    const el = document.querySelector(selector);
    return el instanceof HTMLElement ? el : undefined;
  }

  private _onKeyDown = (event: KeyboardEvent): void => {
    if (event.altKey || event.ctrlKey || event.metaKey) return;

    const map: Record<string, PtzAction | undefined> = {
      ArrowUp: 'up',
      ArrowDown: 'down',
      ArrowLeft: 'left',
      ArrowRight: 'right',
      Equal: 'zoomIn',
      Minus: 'zoomOut',
      Space: 'stop',
    };

    const action = map[event.code];
    if (!action) return;
    event.preventDefault();
    this._send(action);
  };
}
