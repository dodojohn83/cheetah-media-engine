import { createPlayer } from '@cheetah-media/web';

export function createBenchPlayer() {
  return createPlayer();
}

export const BENCHMARKS = {
  playerCreation: () => {
    createPlayer();
  },
};
