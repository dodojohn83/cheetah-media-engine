export const styles = /*css*/ `
  :host {
    --cheetah-surface-bg: #000;
    --cheetah-text: #fff;
    --cheetah-accent: #0af;
    --cheetah-error: #f44;
    --cheetah-overlay-bg: rgba(0, 0, 0, 0.7);
    --cheetah-control-bg: rgba(0, 0, 0, 0.5);
    --cheetah-button-size: 32px;
    --cheetah-control-height: 40px;
    display: block;
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--cheetah-surface-bg);
    color: var(--cheetah-text);
    font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    outline: none;
  }

  :host(:focus-visible) {
    box-shadow: inset 0 0 0 2px var(--cheetah-accent);
  }

  .surface {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }

  ::slotted(video),
  ::slotted(canvas) {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    background: var(--cheetah-overlay-bg);
    padding: 16px;
    text-align: center;
    pointer-events: none;
    opacity: 0;
    transition: opacity 150ms ease;
  }

  .overlay.active {
    opacity: 1;
    pointer-events: auto;
  }

  .overlay-message {
    font-size: 14px;
    line-height: 1.4;
    max-width: 80%;
  }

  .overlay-button {
    appearance: none;
    border: 1px solid currentColor;
    background: transparent;
    color: inherit;
    padding: 6px 12px;
    border-radius: 4px;
    cursor: pointer;
    font: inherit;
  }

  .overlay-button:hover,
  .overlay-button:focus-visible {
    background: var(--cheetah-accent);
    border-color: var(--cheetah-accent);
    color: #fff;
  }

  .controls {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: var(--cheetah-control-height);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    background: var(--cheetah-control-bg);
    opacity: 0;
    transition: opacity 150ms ease;
    pointer-events: none;
  }

  :host([controls]) .controls,
  :host(:hover) .controls,
  :host(:focus-within) .controls {
    opacity: 1;
    pointer-events: auto;
  }

  .control-button {
    appearance: none;
    border: none;
    background: transparent;
    color: inherit;
    width: var(--cheetah-button-size);
    height: var(--cheetah-button-size);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    cursor: pointer;
    font: inherit;
  }

  .control-button:hover,
  .control-button:focus-visible {
    background: rgba(255, 255, 255, 0.15);
  }

  .status {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    text-align: right;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
`;
