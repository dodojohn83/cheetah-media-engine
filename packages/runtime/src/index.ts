export const RUNTIME_VERSION = '0.1.0';

export interface EngineRuntime {
  version: string;
}

export function createRuntime(): EngineRuntime {
  return { version: RUNTIME_VERSION };
}
