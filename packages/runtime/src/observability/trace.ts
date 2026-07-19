/**
 * Lightweight trace spans for correlating async media operations.
 *
 * Traces are identified by player/epoch/sequence and are cheap enough to keep
 * in memory for diagnostic export. They never hold media payloads.
 */

export interface TraceSpan {
  readonly id: string;
  readonly name: string;
  readonly playerId: string;
  readonly epoch: number;
  readonly sequence: number;
  readonly startTime: number;
  readonly endTime?: number;
  readonly parentId?: string;
  readonly children: readonly TraceSpan[];
}

export interface TraceContext {
  readonly playerId: string;
  readonly traceId: string;
  readonly root: TraceSpan;
}

let traceCounter = 0n;

function makeId(): string {
  traceCounter += 1n;
  return `t-${traceCounter.toString()}`;
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 && Number.isInteger(value);
}

export function startTrace(playerId: string, name: string, epoch = 0, sequence = 0): TraceContext {
  if (typeof playerId !== 'string' || playerId.length === 0) {
    throw new Error('playerId must be a non-empty string');
  }
  if (typeof name !== 'string' || name.length === 0) {
    throw new Error('name must be a non-empty string');
  }
  if (!isNonNegativeInteger(epoch)) {
    throw new Error('epoch must be a finite non-negative integer');
  }
  if (!isNonNegativeInteger(sequence)) {
    throw new Error('sequence must be a finite non-negative integer');
  }
  const traceId = makeId();
  const root: TraceSpan = {
    id: makeId(),
    name,
    playerId,
    epoch,
    sequence,
    startTime: nowMs(),
    children: [],
  };
  return { playerId, traceId, root };
}

export function endTrace(context: TraceContext): TraceContext {
  return { ...context, root: endSpan(context.root) };
}

export function childSpan(parent: TraceSpan, name: string, sequence = 0): TraceSpan {
  if (!parent || typeof parent !== 'object') {
    throw new Error('parent must be a TraceSpan');
  }
  if (typeof name !== 'string' || name.length === 0) {
    throw new Error('name must be a non-empty string');
  }
  if (!isNonNegativeInteger(sequence)) {
    throw new Error('sequence must be a finite non-negative integer');
  }
  return {
    id: makeId(),
    name,
    playerId: parent.playerId,
    epoch: parent.epoch,
    sequence,
    parentId: parent.id,
    startTime: nowMs(),
    children: [],
  };
}

export function endSpan(span: TraceSpan): TraceSpan {
  if (span.endTime !== undefined) return span;
  return { ...span, endTime: nowMs() };
}

export function addChild(parent: TraceSpan, child: TraceSpan): TraceSpan {
  return { ...parent, children: [...parent.children, child] };
}

function nowMs(): number {
  return typeof performance !== 'undefined' ? performance.now() : Date.now();
}
