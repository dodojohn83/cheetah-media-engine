import { type EngineRuntime } from '@cheetah-media/runtime';
import { CheetahPlayerImpl, type PlayerConfig, type CheetahPlayer } from './player';

/** @internal Factory for tests that need to inject a mock runtime. */
export function createPlayerWithRuntime(
  config: PlayerConfig,
  runtimeFactory: (opts: { readonly workerUrl?: string | undefined; readonly wasmUrl?: string | undefined }) => EngineRuntime,
): CheetahPlayer {
  return new CheetahPlayerImpl(config, runtimeFactory);
}
