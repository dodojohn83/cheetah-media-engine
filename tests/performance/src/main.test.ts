import { describe, it, expect } from 'vitest';
import { createBenchPlayer } from './main';

describe('performance bench', () => {
  it('creates a player for benchmarking', () => {
    const player = createBenchPlayer();
    expect(player).toBeDefined();
  });
});
