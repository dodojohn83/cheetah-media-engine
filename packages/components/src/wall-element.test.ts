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
});
