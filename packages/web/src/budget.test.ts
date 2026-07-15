import { describe, expect, it, vi } from 'vitest';
import { BudgetController } from './budget';

function demand(id: string, priority: number, visible = true, mainFps = 25, subFps = 15) {
  return {
    id,
    priority,
    visible,
    main: {
      resolution: { width: 1920, height: 1080 },
      fps: mainFps,
      codec: 'h265',
      estimatedMbps: 4,
      backend: 'hardware' as const,
    },
    sub: {
      resolution: { width: 960, height: 540 },
      fps: subFps,
      codec: 'h265',
      estimatedMbps: 0.5,
      backend: 'hardware' as const,
    },
    audio: false,
  };
}

describe('BudgetController', () => {
  it('allocates main to all cells when budget is unlimited', () => {
    const ctrl = new BudgetController();
    const onChange = vi.fn();
    ctrl.onChange(onChange);
    ctrl.addCell(demand('a', 1));
    ctrl.addCell(demand('b', 2));
    const allocs = ctrl.allocate();
    expect(allocs.get('a')?.quality).toBe('main');
    expect(allocs.get('b')?.quality).toBe('main');
    expect(onChange).toHaveBeenCalled();
  });

  it('degrades lower-priority cells to sub when network bandwidth is limited', () => {
    const ctrl = new BudgetController({ maxNetworkMbps: 5 });
    ctrl.addCell(demand('a', 1));
    ctrl.addCell(demand('b', 2));
    const allocs = ctrl.allocate();
    expect(allocs.get('a')?.quality).toBe('main');
    expect(allocs.get('b')?.quality).toBe('sub');
  });

  it('pauses cells when both main and sub exceed limits', () => {
    const ctrl = new BudgetController({ maxHardwareDecoders: 0 });
    ctrl.addCell(demand('a', 1));
    const allocs = ctrl.allocate();
    expect(allocs.get('a')?.quality).toBe('pause');
    expect(allocs.get('a')?.allowed).toBe(false);
  });

  it('promotes a fullscreen cell to main and degrades others', () => {
    const ctrl = new BudgetController({ maxNetworkMbps: 5 });
    ctrl.addCell(demand('a', 1));
    ctrl.addCell(demand('b', 2));
    ctrl.setFullscreen('b', true);
    const allocs = ctrl.allocate();
    expect(allocs.get('b')?.quality).toBe('main');
    expect(allocs.get('a')?.quality).toBe('sub');
  });

  it('hides cells marked not visible', () => {
    const ctrl = new BudgetController();
    ctrl.addCell(demand('a', 1, true));
    ctrl.setVisible('a', false);
    const allocs = ctrl.allocate();
    expect(allocs.get('a')?.quality).toBe('pause');
    expect(allocs.get('a')?.reason).toContain('not visible');
  });

  it('removes a cell and reallocates', () => {
    const ctrl = new BudgetController({ maxHardwareDecoders: 1 });
    ctrl.addCell(demand('a', 1));
    ctrl.addCell(demand('b', 2));
    ctrl.allocate();
    ctrl.removeCell('a');
    const allocs = ctrl.allocate();
    expect(allocs.get('b')?.quality).toBe('main');
    expect(allocs.has('a')).toBe(false);
  });
});
