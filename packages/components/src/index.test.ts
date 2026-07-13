import { describe, it, expect } from 'vitest';
import { createPlayerComponent } from './index';

describe('components', () => {
  it('creates a component', () => {
    const component = createPlayerComponent();
    expect(component.player).toBeDefined();
  });
});
