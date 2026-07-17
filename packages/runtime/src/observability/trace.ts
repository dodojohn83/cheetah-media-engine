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

let traceCounter = 0;

function makeId(): string {
  traceCounter += 1;
  return `t-${traceCounter}`;
}

export function startTrace(playerId: string, name: string, epoch = 0, sequence = 0): TraceContext {
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
