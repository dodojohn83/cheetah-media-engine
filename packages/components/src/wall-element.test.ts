import { describe, expect, it } from 'vitest';
import './index';
import type { CheetahWallCellElement, CheetahWallElement } from './index';

describe('CheetahWallElement', () => {
  it('creates a grid layout based on the layout attribute', () => {
    const wall = document.createElement('cheetah-wall') as CheetahWallElement;
    wall.setAttribute('layout', '4');
    document.body.appendChild(wall);

    const grid = wall.shadowRoot?.querySelector('.grid') as HTMLElement | undefined;
    expect(grid).toBeDefined();
    expect(grid?.style.gridTemplateColumns).toBe('repeat(2, 1fr)');

    wall.remove();
  });

  it('registers visible cells and hides extra cells for a 1-cell layout', async () => {
    const wall = document.createElement('cheetah-wall') as CheetahWallElement;
    wall.setAttribute('layout', '1');
    for (let i = 0; i < 4; i += 1) {
      const cell = document.createElement('cheetah-wall-cell') as CheetahWallCellElement;
      cell.setAttribute('cell-id', `c${i}`);
      wall.appendChild(cell);
    }
    document.body.appendChild(wall);

    await new Promise((resolve) => setTimeout(resolve, 50));

    const cells = wall.querySelectorAll('cheetah-wall-cell');
    expect(cells.length).toBe(4);
    expect((cells[0] as HTMLElement).style.display).not.toBe('none');
    for (let i = 1; i < 4; i += 1) {
      expect((cells[i] as HTMLElement).style.display).toBe('none');
    }

    wall.remove();
  });

  it('focuses a cell when selected-cell is set', () => {
    const wall = document.createElement('cheetah-wall') as CheetahWallElement;
    document.body.appendChild(wall);
    wall.focusCell('cam-1');
    expect(wall.getAttribute('selected-cell')).toBe('cam-1');
    wall.remove();
  });

  it('clears all cells and child elements', () => {
    const wall = document.createElement('cheetah-wall') as CheetahWallElement;
    for (let i = 0; i < 4; i += 1) {
      const cell = document.createElement('cheetah-wall-cell') as CheetahWallCellElement;
      wall.appendChild(cell);
    }
    document.body.appendChild(wall);
    wall.clear();
    expect(wall.querySelectorAll('cheetah-wall-cell').length).toBe(0);
    wall.remove();
  });

  it('continues to manage cells after being moved in the DOM', () => {
    const wall = document.createElement('cheetah-wall') as CheetahWallElement;
    const cell = document.createElement('cheetah-wall-cell') as CheetahWallCellElement;
    cell.cellId = 'moved';
    wall.appendChild(cell);
    document.body.appendChild(wall);

    const player = cell.shadowRoot?.querySelector('cheetah-player');
    expect(player).toBeDefined();

    const newContainer = document.createElement('div');
    document.body.appendChild(newContainer);
    newContainer.appendChild(wall);

    expect(wall.getCellById('moved')).toBeDefined();
    expect(wall.getStats().cells).toBe(1);
    expect(cell.shadowRoot?.querySelector('cheetah-player')).toBeDefined();

    wall.remove();
    newContainer.remove();
  });

  it('hides dynamically added cells that exceed the layout limit', async () => {
    const wall = document.createElement('cheetah-wall') as CheetahWallElement;
    wall.setAttribute('layout', '4');
    document.body.appendChild(wall);
    await new Promise((resolve) => setTimeout(resolve, 50));

    for (let i = 0; i < 5; i += 1) {
      const cell = document.createElement('cheetah-wall-cell') as CheetahWallCellElement;
      cell.setAttribute('cell-id', `dyn${i}`);
      wall.appendChild(cell);
    }
    await new Promise((resolve) => setTimeout(resolve, 50));

    const cells = wall.querySelectorAll('cheetah-wall-cell');
    for (let i = 0; i < 5; i += 1) {
      const display = (cells[i] as HTMLElement).style.display;
      if (i < 4) expect(display).toBe('block');
      else expect(display).toBe('none');
    }

    wall.remove();
  });

  it('pauses and resumes a wall cell without losing its source', async () => {
    const wall = document.createElement('cheetah-wall') as CheetahWallElement;
    const cell = document.createElement('cheetah-wall-cell') as CheetahWallCellElement;
    cell.setAttribute('cell-id', 'p1');
    cell.setAttribute('main-src', 'https://example.com/main.flv');
    wall.appendChild(cell);
    document.body.appendChild(wall);
    await new Promise((resolve) => setTimeout(resolve, 50));

    cell.setQuality('main');
    expect(cell.getAttribute('quality')).toBe('main');

    cell.setQuality('pause');
    expect(cell.getAttribute('quality')).toBe('pause');

    cell.setQuality('main');
    expect(cell.getAttribute('quality')).toBe('main');

    wall.remove();
  });

  it('shows only the fullscreen cell and spans the full wall', async () => {
    const wall = document.createElement('cheetah-wall') as CheetahWallElement;
    wall.setAttribute('layout', '4');
    for (let i = 0; i < 4; i += 1) {
      const cell = document.createElement('cheetah-wall-cell') as CheetahWallCellElement;
      cell.setAttribute('cell-id', `c${i}`);
      wall.appendChild(cell);
    }
    document.body.appendChild(wall);
    await new Promise((resolve) => setTimeout(resolve, 50));

    wall.setAttribute('fullscreen-cell', 'c1');
    await new Promise((resolve) => setTimeout(resolve, 50));

    const grid = wall.shadowRoot?.querySelector('.grid') as HTMLElement | undefined;
    expect(grid?.style.gridTemplateColumns).toBe('repeat(1, 1fr)');

    const cells = wall.querySelectorAll('cheetah-wall-cell');
    for (let i = 0; i < 4; i += 1) {
      const display = (cells[i] as HTMLElement).style.display;
      if (i === 1) expect(display).toBe('block');
      else expect(display).toBe('none');
    }

    wall.remove();
  });

  it('toggles fullscreen-cell on double-click', async () => {
    const wall = document.createElement('cheetah-wall') as CheetahWallElement;
    wall.setAttribute('layout', '4');
    for (let i = 0; i < 4; i += 1) {
      const cell = document.createElement('cheetah-wall-cell') as CheetahWallCellElement;
      cell.setAttribute('cell-id', `c${i}`);
      wall.appendChild(cell);
    }
    document.body.appendChild(wall);
    await new Promise((resolve) => setTimeout(resolve, 50));

    const cells = wall.querySelectorAll('cheetah-wall-cell');
    cells[2]!.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    expect(wall.getAttribute('fullscreen-cell')).toBe('c2');

    cells[2]!.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    expect(wall.hasAttribute('fullscreen-cell')).toBe(false);

    wall.remove();
  });

  it('reorders cells via drag and drop and emits wall:reorder', async () => {
    const wall = document.createElement('cheetah-wall') as CheetahWallElement;
    wall.setAttribute('layout', '4');
    const cellIds = ['a', 'b', 'c', 'd'];
    for (const id of cellIds) {
      const cell = document.createElement('cheetah-wall-cell') as CheetahWallCellElement;
      cell.setAttribute('cell-id', id);
      wall.appendChild(cell);
    }
    document.body.appendChild(wall);
    await new Promise((resolve) => setTimeout(resolve, 50));

    const cells = wall.querySelectorAll('cheetah-wall-cell');
    const source = cells[0] as CheetahWallCellElement;
    const target = cells[2] as CheetahWallCellElement;

    let detail: { cellId: string; oldIndex: number; newIndex: number } | undefined;
    wall.addEventListener('wall:reorder', (event) => {
      detail = (event as CustomEvent).detail as { cellId: string; oldIndex: number; newIndex: number };
    });

    // Simulate dragstart to set internal source.
    source.dispatchEvent(new DragEvent('dragstart', { bubbles: true }));
    // Drop on target; use a rect that puts the pointer on the left edge to insert before.
    Object.defineProperty(target, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, right: 100, bottom: 100 }),
      configurable: true,
    });
    target.dispatchEvent(new DragEvent('drop', { bubbles: true, clientX: 10, clientY: 50 }));

    expect(detail).toBeDefined();
    expect(detail?.cellId).toBe('a');
    expect(detail?.oldIndex).toBe(0);
    expect(detail?.newIndex).toBe(2);

    wall.remove();
  });

  it('renders an irregular layout from data-grid attributes', async () => {
    const wall = document.createElement('cheetah-wall') as CheetahWallElement;
    wall.setAttribute('layout', 'custom');

    const a = document.createElement('cheetah-wall-cell') as CheetahWallCellElement;
    a.setAttribute('cell-id', 'a');
    a.setAttribute('data-grid', JSON.stringify({ col: 1, row: 1, colSpan: 2, rowSpan: 2 }));
    wall.appendChild(a);

    const b = document.createElement('cheetah-wall-cell') as CheetahWallCellElement;
    b.setAttribute('cell-id', 'b');
    b.setAttribute('data-grid', JSON.stringify({ col: 3, row: 1 }));
    wall.appendChild(b);

    const c = document.createElement('cheetah-wall-cell') as CheetahWallCellElement;
    c.setAttribute('cell-id', 'c');
    c.setAttribute('data-grid', JSON.stringify({ col: 3, row: 2 }));
    wall.appendChild(c);

    document.body.appendChild(wall);
    await new Promise((resolve) => setTimeout(resolve, 50));

    const grid = wall.shadowRoot?.querySelector('.grid') as HTMLElement | undefined;
    expect(grid?.style.gridTemplateColumns).toBe('repeat(3, 1fr)');
    expect(grid?.style.gridTemplateRows).toBe('repeat(2, 1fr)');

    expect((a as HTMLElement).style.gridColumn).toBe('1 / span 2');
    expect((a as HTMLElement).style.gridRow).toBe('1 / span 2');
    expect((b as HTMLElement).style.gridColumn).toBe('3 / span 1');
    expect((c as HTMLElement).style.gridRow).toBe('2 / span 1');

    wall.remove();
  });
});
