/**
 * Runtime fallback state machine for the browser media route.
 *
 * The fallback controller owns the current `PlaybackPlan` and coordinates
 * switching from one backend candidate to the next.  It does not contain
 * backend-specific logic; it delegates to the `MediaBackend` instances created
 * by a caller-supplied `MediaBackendFactory`.
 */

import type { Backend, PlaybackPlan, PlanCandidate } from './planner';

export interface FallbackState {
  readonly currentCandidate: PlanCandidate | undefined;
  readonly triedThisEpoch: ReadonlySet<Backend>;
  readonly exhausted: boolean;
  readonly recoveryStartMs: number;
}

export interface BackendContext {
  readonly candidate: PlanCandidate;
  readonly reason: string;
}

export interface MediaBackend {
  /**
   * Configure the backend with the candidate. Return a promise that resolves
   * when the backend is ready to accept data, or rejects if configuration
   * fails.
   */
  configure(): Promise<void>;
  /**
   * Stop the backend and release resources.
   */
  stop(): Promise<void>;
  /**
   * Current backend identity, used to avoid WebCodecs↔MSE loops.
   */
  readonly identity: Backend;
}

export type MediaBackendFactory = (ctx: BackendContext) => MediaBackend;

export interface BackendChangeEvent {
  readonly from: Backend | undefined;
  readonly to: Backend;
  readonly reason: string;
  readonly recoveryMs: number;
}

export type FallbackEvent =
  | { type: 'backendchange'; payload: BackendChangeEvent }
  | { type: 'unsupported'; payload: { reason: string; attemptChain: readonly { backend: Backend; reason: string }[] } }
  | { type: 'degraded'; payload: { reason: string } };

export interface FallbackOptions {
  readonly plan: PlaybackPlan;
  readonly factory: MediaBackendFactory;
  readonly onEvent?: (event: FallbackEvent) => void;
}

export class FallbackController {
  private plan: PlaybackPlan;
  private factory: MediaBackendFactory;
  private onEvent: ((event: FallbackEvent) => void) | undefined = undefined;
  private current: MediaBackend | undefined = undefined;
  private currentCandidate: PlanCandidate | undefined = undefined;
  private tried = new Map<Backend, number>();
  private attemptReasons = new Map<Backend, string>();
  private epoch = 0;
  private recoveryStartMs = 0;
  private stopped = false;

  constructor(options: FallbackOptions) {
    this.plan = options.plan;
    this.factory = options.factory;
    this.onEvent = options.onEvent;
  }

  /**
   * Start a new epoch.  This clears per-epoch tried counters so previously
   * failed backends can be retried after a major stream discontinuity, but
   * it does not reset the lifetime attempt counters.
   */
  newEpoch(): void {
    this.epoch += 1;
    this.tried.clear();
    this.attemptReasons.clear();
    this.recoveryStartMs = performance.now();
  }

  /**
   * Try the next candidate in the plan.  Returns the configured backend on
   * success, or `undefined` if the plan is exhausted.
   */
  async configureNext(reason = 'initial'): Promise<MediaBackend | undefined> {
    if (this.stopped) return undefined;

    const candidate = this.plan.candidates.find((c) => {
      const video = c.videoBackend;
      const audio = c.audioBackend;
      if (video && this.tried.get(video) !== undefined) return false;
      if (audio && this.tried.get(audio) !== undefined) return false;
      return true;
    });

    if (!candidate) {
      this.emit({
        type: 'unsupported',
        payload: {
          reason: `all candidates failed: ${reason}`,
          attemptChain: this.buildAttemptChain(),
        },
      });
      return undefined;
    }

    const backend = await this.activate(candidate, reason);
    if (backend) {
      this.current = backend;
      this.currentCandidate = candidate;
      return backend;
    }

    return this.configureNext(`${reason} -> ${this.describe(candidate)} failed`);
  }

  /**
   * Report a failure from the currently active backend and fall back to the
   * next candidate that has not been tried this epoch.
   */
  async reportFailure(failureReason: string): Promise<MediaBackend | undefined> {
    if (this.stopped) return undefined;
    if (this.current) {
      const identity = this.current.identity;
      this.tried.set(identity, (this.tried.get(identity) ?? 0) + 1);
      this.attemptReasons.set(identity, failureReason);
    }
    await this.stopCurrent();
    return this.configureNext(failureReason);
  }

  /**
   * Stop the current backend and mark the controller as stopped.
   */
  async stop(): Promise<void> {
    this.stopped = true;
    await this.stopCurrent();
  }

  /**
   * Replace the current plan (e.g. after a capability refresh) and reset the
   * per-epoch tried set so the new candidates can be attempted.
   */
  setPlan(plan: PlaybackPlan): void {
    this.plan = plan;
    this.tried.clear();
    this.attemptReasons.clear();
  }

  getState(): FallbackState {
    return {
      currentCandidate: this.currentCandidate,
      triedThisEpoch: new Set(this.tried.keys()),
      exhausted: this.plan.candidates.every((c) => {
        return (c.videoBackend !== undefined && this.tried.has(c.videoBackend)) ||
          (c.audioBackend !== undefined && this.tried.has(c.audioBackend));
      }),
      recoveryStartMs: this.recoveryStartMs,
    };
  }

  private async activate(candidate: PlanCandidate, reason: string): Promise<MediaBackend | undefined> {
    const ctx: BackendContext = { candidate, reason };
    const from = this.current?.identity;
    await this.stopCurrent();
    this.recoveryStartMs = performance.now();

    const backend = this.factory(ctx);
    try {
      await backend.configure();
    } catch (err) {
      // Mark both backend identities as tried for this epoch and record reason.
      const message = err instanceof Error ? err.message : String(err);
      if (candidate.videoBackend) {
        this.tried.set(candidate.videoBackend, 1);
        this.attemptReasons.set(candidate.videoBackend, message);
      }
      if (candidate.audioBackend) {
        this.tried.set(candidate.audioBackend, 1);
        this.attemptReasons.set(candidate.audioBackend, message);
      }
      try {
        await backend.stop();
      } catch {
        // ignore cleanup errors
      }
      return undefined;
    }

    const to = backend.identity;
    this.current = backend;
    this.currentCandidate = candidate;

    this.emit({
      type: 'backendchange',
      payload: {
        from,
        to,
        reason,
        recoveryMs: performance.now() - this.recoveryStartMs,
      },
    });

    if (this.plan.degraded) {
      this.emit({
        type: 'degraded',
        payload: { reason: `route degraded: ${this.plan.reasonChain.join(' -> ')}` },
      });
    }

    return backend;
  }

  private async stopCurrent(): Promise<void> {
    if (!this.current) return;
    try {
      await this.current.stop();
    } catch {
      // ignore cleanup errors
    }
    this.current = undefined;
    this.currentCandidate = undefined;
  }

  private buildAttemptChain(): { backend: Backend; reason: string }[] {
    const chain: { backend: Backend; reason: string }[] = [];
    for (const [backend, reason] of this.attemptReasons) {
      chain.push({ backend, reason });
    }
    for (const u of this.plan.unsupported) {
      if (!this.attemptReasons.has(u.backend)) {
        chain.push({ backend: u.backend, reason: u.reason });
      }
    }
    return chain;
  }

  private describe(candidate: PlanCandidate): string {
    const parts: string[] = [];
    if (candidate.videoBackend) parts.push(candidate.videoBackend);
    if (candidate.audioBackend) parts.push(candidate.audioBackend);
    return parts.join('+') || 'none';
  }

  private emit(event: FallbackEvent): void {
    this.onEvent?.(event);
  }
}
