import { describe, it, expect } from 'vitest';
import { createRuntime, RUNTIME_VERSION } from './index';

describe('runtime', () => {
  it('reports version', () => {
    const runtime = createRuntime();
    expect(runtime.version).toBe(RUNTIME_VERSION);
  });
});
