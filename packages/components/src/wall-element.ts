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

type WallLayout = number | 'custom';

function parseLayout(value: string | null): WallLayout {
  if (value === 'custom') return 'custom';
  const n = value ? Number(value) : 1;
  return LAYOUT_COLUMNS[n] !== undefined ? n : 1;
}

function parseDataGrid(value: string | null): { col: number; row: number; colSpan: number; rowSpan: number } | undefined {
  if (!value) return undefined;
  try {
    const parsed = JSON.parse(value) as { col?: number; row?: number; colSpan?: number; rowSpan?: number };
    const col = Number(parsed.col);
    const row = Number(parsed.row);
    if (!Number.isFinite(col) || col < 1 || !Number.isFinite(row) || row < 1) return undefined;
    const colSpan = Math.max(1, Math.floor(Number(parsed.colSpan) || 1));
    const rowSpan = Math.max(1, Math.floor(Number(parsed.rowSpan) || 1));
    return { col, row, colSpan, rowSpan };
  } catch {
    return undefined;
  }
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
  private _dragSource: CheetahWallCellElement | undefined;

  get layout(): WallLayout {
    return parseLayout(this.getAttribute('layout'));
  }

  set layout(value: WallLayout) {
    if (value === 'custom' || LAYOUT_COLUMNS[value] !== undefined) {
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
    this.addEventListener('dblclick', this._onDblClick);
    this.addEventListener('dragstart', this._onDragStart);
    this.addEventListener('dragover', this._onDragOver);
    this.addEventListener('drop', this._onDrop);

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
    this.removeEventListener('dblclick', this._onDblClick);
    this.removeEventListener('dragstart', this._onDragStart);
    this.removeEventListener('dragover', this._onDragOver);
    this.removeEventListener('drop', this._onDrop);
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

  setLayout(layout: 1 | 4 | 9 | 16 | 'custom'): void {
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

  getStats(): { cells: number; layout: WallLayout; selected?: string | undefined; fullscreen?: string | undefined } {
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
    const fullscreen = this.fullscreenCell;

    for (const [i, cell] of cells.entries()) {
      const id = cell.cellId ?? `cell-${i}`;
      if (cell.cellId !== id) cell.cellId = id;
      visibleIds.add(id);
      const visible = !fullscreen ? this._isVisibleInLayout(i, cell) : cell.cellId === fullscreen;
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
    const fullscreen = this.fullscreenCell;
    for (const [i, cell] of cells.entries()) {
      const id = cell.cellId ?? `cell-${i}`;
      const visible = !fullscreen ? this._isVisibleInLayout(i, cell) : cell.cellId === fullscreen;
      this._budget.setVisible(id, visible);
    }
  }

  private _updatePriorities(): void {
    if (!this._budget) return;
    const cells = this._cells();
    const fullscreen = this.fullscreenCell;
    const selected = this.selectedCell;
    for (const [i, cell] of cells.entries()) {
      const id = cell.cellId ?? `cell-${i}`;
      const isFullscreen = fullscreen === cell.cellId;
      const isSelected = selected === cell.cellId;
      const visible = !fullscreen ? this._isVisibleInLayout(i, cell) : isFullscreen;
      let priority = i + 1;
      if (isFullscreen) priority = 0;
      else if (isSelected) priority = 1;
      this._budget.setVisible(id, visible);
      this._budget.setPriority(id, priority);
    }
  }

  private _isVisibleInLayout(index: number, cell: CheetahWallCellElement): boolean {
    const layout = this.layout;
    if (layout === 'custom') {
      return parseDataGrid(cell.getAttribute('data-grid')) !== undefined;
    }
    return index < layout;
  }

  private _onDblClick = (event: MouseEvent): void => {
    const cell = this._cellFromEventTarget(event.target);
    if (!cell || !cell.cellId) return;
    event.stopPropagation();
    this.fullscreenCell = this.fullscreenCell === cell.cellId ? undefined : cell.cellId;
  };

  private _onDragStart = (event: DragEvent): void => {
    if (this.layout === 'custom' || this.fullscreenCell) return;
    const cell = this._cellFromEventTarget(event.target);
    if (!cell || !cell.cellId) return;
    this._dragSource = cell;
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'move';
      event.dataTransfer.setData('text/plain', cell.cellId);
    }
  };

  private _onDragOver = (event: DragEvent): void => {
    if (!this._dragSource) return;
    event.preventDefault();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = 'move';
    }
  };

  private _onDrop = (event: DragEvent): void => {
    event.preventDefault();
    const source = this._dragSource;
    this._dragSource = undefined;
    if (!source || !source.cellId) return;

    const target = this._cellFromEventTarget(event.target);
    if (!target || !target.cellId || target === source || !this.contains(target)) return;

    const oldIndex = this._cellIndex(source);
    const insertBefore = this._shouldInsertBefore(event, target);
    if (insertBefore) {
      this.insertBefore(source, target);
    } else {
      this.insertBefore(source, target.nextSibling);
    }
    const newIndex = this._cellIndex(source);

    this.dispatchEvent(
      new CustomEvent('wall:reorder', {
        detail: { cellId: source.cellId, oldIndex, newIndex },
        bubbles: true,
        composed: true,
      }),
    );
  };

  private _cellFromEventTarget(target: EventTarget | null): CheetahWallCellElement | undefined {
    if (!(target instanceof Element)) return undefined;
    let el: Element | null = target;
    while (el) {
      if (el.tagName.toLowerCase() === 'cheetah-wall-cell' && this.contains(el)) {
        return el as CheetahWallCellElement;
      }
      const root = el.getRootNode();
      if (root instanceof ShadowRoot && root.host) {
        el = root.host;
      } else {
        el = el.parentElement;
      }
    }
    return undefined;
  }

  private _cellIndex(cell: HTMLElement): number {
    return this._cells().findIndex((c) => c === cell);
  }

  private _shouldInsertBefore(event: DragEvent, target: HTMLElement): boolean {
    if (typeof target.getBoundingClientRect !== 'function') return true;
    const rect = target.getBoundingClientRect();
    const x = event.clientX;
    const y = event.clientY;
    const left = x - rect.left;
    const right = rect.right - x;
    const top = y - rect.top;
    const bottom = rect.bottom - y;
    const min = Math.min(left, right, top, bottom);
    return min === left || min === top;
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
    const cells = this._cells();

    if (this.layout === 'custom') {
      this._applyCustomGrid(grid, cells, fullscreen);
      return;
    }

    const layout = this.layout;
    const cols = fullscreen ? 1 : (LAYOUT_COLUMNS[layout] ?? 1);
    grid.style.gridTemplateColumns = `repeat(${cols}, 1fr)`;
    grid.style.gridTemplateRows = `repeat(${cols}, 1fr)`;
    for (const [i, cell] of cells.entries()) {
      const el = cell as HTMLElement;
      if (fullscreen) {
        el.style.display = cell.getAttribute('cell-id') === fullscreen ? 'block' : 'none';
        el.style.gridColumn = '';
        el.style.gridRow = '';
        el.removeAttribute('draggable');
      } else if (this._isVisibleInLayout(i, cell)) {
        el.style.display = 'block';
        el.style.gridColumn = '';
        el.style.gridRow = '';
        el.setAttribute('draggable', 'true');
      } else {
        el.style.display = 'none';
        el.removeAttribute('draggable');
      }
    }
  }

  private _applyCustomGrid(grid: HTMLElement, cells: CheetahWallCellElement[], fullscreen: string | undefined): void {
    let maxCol = 0;
    let maxRow = 0;
    for (const cell of cells) {
      const g = parseDataGrid(cell.getAttribute('data-grid'));
      if (!g) continue;
      maxCol = Math.max(maxCol, g.col + g.colSpan - 1);
      maxRow = Math.max(maxRow, g.row + g.rowSpan - 1);
    }
    if (maxCol === 0) maxCol = 1;
    if (maxRow === 0) maxRow = 1;

    grid.style.gridTemplateColumns = `repeat(${maxCol}, 1fr)`;
    grid.style.gridTemplateRows = `repeat(${maxRow}, 1fr)`;

    for (const cell of cells) {
      const el = cell as HTMLElement;
      el.removeAttribute('draggable');
      const id = cell.cellId;
      if (fullscreen && id !== fullscreen) {
        el.style.display = 'none';
        el.style.gridColumn = '';
        el.style.gridRow = '';
        continue;
      }
      const g = parseDataGrid(cell.getAttribute('data-grid'));
      if (!g) {
        el.style.display = 'none';
        continue;
      }
      el.style.display = 'block';
      el.style.gridColumn = `${g.col} / span ${g.colSpan}`;
      el.style.gridRow = `${g.row} / span ${g.rowSpan}`;
    }
  }
}
