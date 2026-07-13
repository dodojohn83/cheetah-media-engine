import { createRuntime, EngineRuntime } from '@cheetah-media/runtime';

export interface Player {
  load(url: string): Promise<void>;
  play(): void;
  pause(): void;
  stop(): void;
  destroy(): void;
}

export function createPlayer(): Player & EngineRuntime {
  const runtime = createRuntime();
  return {
    ...runtime,
    load: async (_url: string) => { /* TODO */ },
    play: () => { /* TODO */ },
    pause: () => { /* TODO */ },
    stop: () => { /* TODO */ },
    destroy: () => { /* TODO */ },
  };
}
