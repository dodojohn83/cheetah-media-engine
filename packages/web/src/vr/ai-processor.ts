import type { ProcessableFrame, AiFrameBudget, AiFrameResult } from './types';

/**
 * Pluggable AI frame processor extension.
 *
 * The player delivers decoded frames to a registered processor. The processor
 * must respect the supplied budget: when `canAllocate` is false or the deadline
 * is too tight, it should return `undefined` (skip). The default
 * `NoopAiFrameProcessor` always skips.
 */
export interface AiFrameProcessor {
  /** Whether the processor is currently active and consuming frames. */
  readonly active: boolean;

  /** Optional initialization/config hook; returns `true` when active. */
  initialize(): boolean;

  /**
   * Process a frame.
   *
   * Implementations must complete within `budget.deadlineMs` and should skip
   * when `budget.canAllocate` is false.
   */
  process(frame: ProcessableFrame, budget: AiFrameBudget): AiFrameResult | undefined | Promise<AiFrameResult | undefined>;

  /** Release resources. */
  destroy(): void;
}

/** Default no-op AI processor that always skips processing. */
export class NoopAiFrameProcessor implements AiFrameProcessor {
  get active(): boolean {
    return false;
  }

  initialize(): boolean {
    return false;
  }

  process(): undefined {
    return undefined;
  }

  destroy(): void {
    // Nothing to release.
  }
}
