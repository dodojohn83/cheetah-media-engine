import {
  BudgetController,
  type CellAllocation,
  type CellDemand,
  type ResourceBudgetConfig,
  type StreamProfile,
} from '@cheetah-media/web';
import type { CheetahWallCellElement } from './wall-cell-element';

const OBSERVED_ATTRIBUTES = ['layout', 'selected-cell', 'fullscreen-cell', 'max-hardware-decoders'] as const;

type ObservedAttribute = (typeof OBSERVED_ATTRIBUTES)[number];

const LAYOUT_COLUMNS: Record<number, number> = {
  1: 1,
  4: 2,
  9: 3,
  16: 4,
};

function parseLayout(value: string | null): number {
  const n = value ? Number(value) : 1;
  return LAYOUT_COLUMNS[n] !== undefined ? n : 1;
}

function parseResolution(value: string | null): { width: number; height: number } {
  if (!value) return { width: 1920, height: 1080 };
  const parts = value.split('x').map((s) => Number(s.trim()));
  const w = parts[0];
  const h = parts[1];
  if (w !== undefined && h !== undefined && Number.isFinite(w) && Number.isFinite(h) && w > 0 && h > 0) {
    return { width: w, height: h };
  }
  return { width: 1920, height: 1080 };
}

export class CheetahWallElement extends HTMLElement {
  static get observedAttributes(): readonly string[] {
    return OBSERVED_ATTRIBUTES as unknown as readonly string[];
  }

  private _budget: BudgetController | undefined;
  private _registeredIds = new Set<string>();
  private _resizeObserver: ResizeObserver | undefined;
  private _mutationObserver: MutationObserver | undefined;

  get layout(): number {
    return parseLayout(this.getAttribute('layout'));
  }

  set layout(value: number) {
    if (LAYOUT_COLUMNS[value] !== undefined) {
      this.setAttribute('layout', String(value));
    }
  }

  get selectedCell(): string | undefined {
    return this.getAttribute('selected-cell') ?? undefined;
  }

  set selectedCell(value: string | undefined) {
    if (value === undefined) this.removeAttribute('selected-cell');
    else this.setAttribute('selected-cell', value);
  }

  get fullscreenCell(): string | undefined {
    return this.getAttribute('fullscreen-cell') ?? undefined;
  }

  set fullscreenCell(value: string | undefined) {
    if (value === undefined) this.removeAttribute('fullscreen-cell');
    else this.setAttribute('fullscreen-cell', value);
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
        }
        .grid {
          display: grid;
          width: 100%;
          height: 100%;
          gap: 2px;
        }
        ::slotted(cheetah-wall-cell) {
          display: block;
          min-width: 0;
          min-height: 0;
          overflow: hidden;
        }
      `;
      shadow.appendChild(style);
      const grid = document.createElement('div');
      grid.className = 'grid';
      grid.setAttribute('part', 'grid');
      const slot = document.createElement('slot');
      grid.appendChild(slot);
      shadow.appendChild(grid);
    }

    this._updateGrid();
    this._setupBudget();
    this._setupObservers();
    this._registerCells();
  }

  disconnectedCallback(): void {
    this._disconnectObservers();
    if (this._budget) {
      this._budget = undefined;
    }
    this._registeredIds.clear();
    // Do not destroy child cells here; cells manage their own lifecycle on
    // disconnect/reconnect. Destroying them here would break DOM moves.
  }

  attributeChangedCallback(name: ObservedAttribute, oldValue: string | null, newValue: string | null): void {
    if (oldValue === newValue) return;
    if (name === 'layout') {
      this._updateGrid();
      this._updateCellVisibility();
      return;
    }
    if (name === 'selected-cell' || name === 'fullscreen-cell') {
      this._updateGrid();
      this._updatePriorities();
      return;
    }
    if (name === 'max-hardware-decoders') {
      this._updateBudgetConfig();
      return;
    }
  }

  setLayout(layout: 1 | 4 | 9 | 16): void {
    this.layout = layout;
  }

  focusCell(cellId: string): void {
    this.selectedCell = cellId;
  }

  clear(): void {
    for (const cell of this._cells()) {
      cell.destroy();
    }
    this.innerHTML = '';
    this._updatePriorities();
  }

  getCellById(id: string): CheetahWallCellElement | undefined {
    for (const cell of this._cells()) {
      if (cell.cellId === id) return cell;
    }
    return undefined;
  }

  getStats(): { cells: number; layout: number; selected?: string | undefined; fullscreen?: string | undefined } {
    return {
      cells: this._cells().length,
      layout: this.layout,
      selected: this.selectedCell,
      fullscreen: this.fullscreenCell,
    };
  }

  private _cells(): CheetahWallCellElement[] {
    return Array.from(this.querySelectorAll('cheetah-wall-cell')) as CheetahWallCellElement[];
  }

  private _setupBudget(): void {
    if (this._budget) return;
    this._budget = new BudgetController(this._buildBudgetConfig());
    this._budget.onChange((allocations) => this._applyAllocations(allocations));
  }

  private _updateBudgetConfig(): void {
    if (!this._budget) return;
    this._budget.setConfig(this._buildBudgetConfig());
  }

  private _buildBudgetConfig(): ResourceBudgetConfig {
    const raw = this.getAttribute('max-hardware-decoders');
    const max = raw ? Number(raw) : NaN;
    const finite = Number.isFinite(max);
    return {
      mainDwellMs: 2000,
      subDwellMs: 1000,
      ...(finite ? { maxHardwareDecoders: max } : {}),
    };
  }

  private _setupObservers(): void {
    if (typeof ResizeObserver !== 'undefined') {
      this._resizeObserver = new ResizeObserver(() => this._updateGrid());
      this._resizeObserver.observe(this);
    }
    if (typeof MutationObserver !== 'undefined') {
      this._mutationObserver = new MutationObserver(() => {
        this._registerCells();
        this._updateGrid();
      });
      this._mutationObserver.observe(this, { childList: true });
    }
  }

  private _disconnectObservers(): void {
    if (this._resizeObserver) {
      this._resizeObserver.disconnect();
      this._resizeObserver = undefined;
    }
    if (this._mutationObserver) {
      this._mutationObserver.disconnect();
      this._mutationObserver = undefined;
    }
  }

  private _registerCells(): void {
    if (!this._budget) return;
    const cells = this._cells();
    const visibleIds = new Set<string>();
    const layout = this.layout;
    const fullscreen = this.fullscreenCell;

    for (const [i, cell] of cells.entries()) {
      const id = cell.cellId ?? `cell-${i}`;
      if (cell.cellId !== id) cell.cellId = id;
      visibleIds.add(id);
      const visible = !fullscreen ? i < layout : cell.cellId === fullscreen;
      const demand = this._cellDemand(cell, i, visible);
      if (this._registeredIds.has(id)) {
        this._budget.updateCell(demand);
      } else {
        this._registeredIds.add(id);
        this._budget.addCell(demand);
      }
    }

    for (const id of this._registeredIds) {
      if (!visibleIds.has(id)) {
        this._budget.removeCell(id);
        this._registeredIds.delete(id);
      }
    }
  }

  private _cellDemand(cell: CheetahWallCellElement, index: number, visible: boolean): CellDemand {
    const resolution = parseResolution(cell.getAttribute('resolution'));
    const mainFps = Number(cell.getAttribute('main-fps')) || 25;
    const subFps = Number(cell.getAttribute('sub-fps')) || 15;
    const mainBitrate = Number(cell.getAttribute('main-bitrate')) || 4000;
    const subBitrate = Number(cell.getAttribute('sub-bitrate')) || 500;
    const backend = cell.getAttribute('backend') === 'software' ? 'software' : 'hardware';
    const codec = cell.getAttribute('codec') ?? 'h265';

    const main: StreamProfile = {
      resolution,
      fps: mainFps,
      codec,
      estimatedMbps: mainBitrate / 1000,
      backend,
    };
    const sub: StreamProfile = {
      resolution: { width: Math.round(resolution.width / 2), height: Math.round(resolution.height / 2) },
      fps: subFps,
      codec,
      estimatedMbps: subBitrate / 1000,
      backend,
    };

    const fullscreen = this.fullscreenCell === cell.cellId;
    const selected = this.selectedCell === cell.cellId;
    let priority = index + 1;
    if (fullscreen) priority = 0;
    else if (selected) priority = Math.min(priority, 1);

    return {
      id: cell.cellId ?? `cell-${index}`,
      priority,
      visible,
      main,
      sub,
      audio: cell.hasAttribute('audio'),
    };
  }

  private _updateCellVisibility(): void {
    if (!this._budget) return;
    const cells = this._cells();
    const layout = this.layout;
    const fullscreen = this.fullscreenCell;
    for (const [i, cell] of cells.entries()) {
      const id = cell.cellId ?? `cell-${i}`;
      const visible = !fullscreen ? i < layout : cell.cellId === fullscreen;
      this._budget.setVisible(id, visible);
    }
  }

  private _updatePriorities(): void {
    if (!this._budget) return;
    const cells = this._cells();
    const layout = this.layout;
    const fullscreen = this.fullscreenCell;
    const selected = this.selectedCell;
    for (const [i, cell] of cells.entries()) {
      const id = cell.cellId ?? `cell-${i}`;
      const isFullscreen = fullscreen === cell.cellId;
      const isSelected = selected === cell.cellId;
      const visible = !fullscreen ? i < layout : isFullscreen;
      let priority = i + 1;
      if (isFullscreen) priority = 0;
      else if (isSelected) priority = 1;
      this._budget.setVisible(id, visible);
      this._budget.setPriority(id, priority);
    }
  }

  private _applyAllocations(allocations: ReadonlyMap<string, CellAllocation>): void {
    const cells = this._cells();
    for (const cell of cells) {
      const id = cell.cellId;
      if (!id) continue;
      const allocation = allocations.get(id);
      if (!allocation) continue;
      cell.setQuality(allocation.quality);
    }
  }

  private _updateGrid(): void {
    if (!this.shadowRoot) return;
    const grid = this.shadowRoot.querySelector('.grid') as HTMLElement | null;
    if (!grid) return;
    const fullscreen = this.fullscreenCell;
    const cols = fullscreen ? 1 : (LAYOUT_COLUMNS[this.layout] ?? 1);
    grid.style.gridTemplateColumns = `repeat(${cols}, 1fr)`;
    grid.style.gridTemplateRows = `repeat(${cols}, 1fr)`;
    const cells = this._cells();
    for (const [i, cell] of cells.entries()) {
      const el = cell as HTMLElement;
      if (fullscreen) {
        el.style.display = cell.getAttribute('cell-id') === fullscreen ? 'block' : 'none';
        el.style.gridColumn = '';
        el.style.gridRow = '';
      } else if (i < this.layout) {
        el.style.display = 'block';
        el.style.gridColumn = '';
        el.style.gridRow = '';
      } else {
        el.style.display = 'none';
      }
    }
  }
}
