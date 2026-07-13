import { describe, it, expect } from 'vitest';
import { createPlayer } from './index';

describe('web sdk', () => {
  it('creates a player', () => {
    const player = createPlayer();
    expect(player.version).toBeDefined();
  });
});
