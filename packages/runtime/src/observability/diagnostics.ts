/**
 * Diagnostic bundle builder.
 *
 * Collects a redacted snapshot of version, capability, configuration,
 * metrics, event history and trace data. URL and secret redaction is performed
 * before the snapshot is returned so that no sensitive session data leaks.
 */

import type { MetricsSnapshot } from './metrics';
import type { TraceSpan } from './trace';

export interface DiagnosticsEvent {
  readonly type: string;
  readonly timestamp: number;
  readonly epoch?: number;
  readonly details?: Record<string, unknown>;
}

export const DIAGNOSTICS_VERSION = '0.1.0';

export interface DiagnosticsOptions {
  readonly maxEventCount?: number;
  readonly maxTraceDepth?: number;
  readonly maxSizeBytes?: number;
}

export interface DiagnosticsBundle {
  readonly version: string;
  readonly timestamp: number;
  readonly playerId: string;
  readonly state: string;
  readonly epoch: number;
  readonly config: unknown;
  readonly metrics?: MetricsSnapshot | undefined;
  readonly recentEvents: readonly DiagnosticsEvent[];
  readonly trace?: TraceSpan | undefined;
}

const SIZE_SAFETY_MARGIN = 1024;

export function sanitizeUrl(url: string): string {
  if (typeof url !== 'string') {
    throw new Error('sanitizeUrl url must be a string');
  }
  try {
    const parsed = new URL(url);
    // Only http/https URLs are safe to retain at origin+path level. Other
    // schemes (data:, blob:, javascript:, file:) may embed sensitive payloads
    // directly in their path/query and must not be echoed back.
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
      return '<redacted>';
    }
    let result = `${parsed.protocol}//${parsed.host}`;
    if (parsed.pathname) {
      // Keep the origin and path only; strip query, fragment and any username/password.
      result += parsed.pathname;
    }
    return result;
  } catch {
    // If the URL is invalid, return a redacted marker instead of the raw string.
    return '<redacted>';
  }
}

export function redactHeaders(
  headers?: Record<string, string> | Iterable<readonly [string, string]>,
): Record<string, string> {
  if (!headers || typeof headers !== 'object') return {};
  const sensitive = new Set([
    'authorization',
    'cookie',
    'x-api-key',
    'x-auth-token',
    'x-secret',
  ]);
  const redacted: Record<string, string> = {};
  const entries =
    typeof (headers as Iterable<unknown>)[Symbol.iterator] === 'function'
      ? Array.from(headers as Iterable<readonly [string, string]>)
      : Object.entries(headers);
  for (const [key, value] of entries) {
    if (typeof key !== 'string' || typeof value !== 'string') continue;
    redacted[key] = sensitive.has(key.toLowerCase()) ? '<redacted>' : value;
  }
  return redacted;
}

export function buildDiagnostics(
  playerId: string,
  state: string,
  epoch: number,
  config: unknown,
  extras: {
    readonly metrics?: MetricsSnapshot;
    readonly events?: readonly DiagnosticsEvent[];
    readonly trace?: TraceSpan;
  } = {},
  options: DiagnosticsOptions = {},
): DiagnosticsBundle {
  if (typeof playerId !== 'string' || playerId.length === 0) {
    throw new Error('playerId must be a non-empty string');
  }
  if (typeof state !== 'string' || state.length === 0) {
    throw new Error('state must be a non-empty string');
  }
  if (!Number.isFinite(epoch) || epoch < 0 || !Number.isInteger(epoch)) {
    throw new Error('epoch must be a finite non-negative integer');
  }
  const maxEventCount = options.maxEventCount ?? 500;
  const maxSizeBytes = options.maxSizeBytes ?? 256 * 1024;
  const maxTraceDepth = options.maxTraceDepth;
  if (maxEventCount !== undefined && (!Number.isFinite(maxEventCount) || maxEventCount < 0 || !Number.isInteger(maxEventCount))) {
    throw new Error('maxEventCount must be a finite non-negative integer');
  }
  if (maxSizeBytes !== undefined && (!Number.isFinite(maxSizeBytes) || maxSizeBytes < 0 || !Number.isInteger(maxSizeBytes))) {
    throw new Error('maxSizeBytes must be a finite non-negative integer');
  }
  if (maxTraceDepth !== undefined && (!Number.isFinite(maxTraceDepth) || maxTraceDepth < 0 || !Number.isInteger(maxTraceDepth))) {
    throw new Error('maxTraceDepth must be a finite non-negative integer');
  }

  const recentEvents = extras.events ? (maxEventCount === 0 ? [] : extras.events.slice(-maxEventCount)) : [];
  const trace = maxTraceDepth !== undefined && extras.trace
    ? truncateTrace(extras.trace, maxTraceDepth)
    : extras.trace;

  const bundle: DiagnosticsBundle = {
    version: DIAGNOSTICS_VERSION,
    timestamp: nowMs(),
    playerId,
    state,
    epoch,
    config: redactConfig(config),
    recentEvents,
    ...(extras.metrics !== undefined && { metrics: extras.metrics }),
    ...(trace !== undefined && { trace }),
  };

  return fitToSize(bundle, maxSizeBytes);
}

function redactConfig(config: unknown): unknown {
  return redactConfigImpl(config, new WeakSet());
}

function redactConfigImpl(config: unknown, seen: WeakSet<object>): unknown {
  if (config === null || typeof config !== 'object') return config;
  if (seen.has(config)) return '<circular>';
  seen.add(config);
  if (Array.isArray(config)) {
    const result = config.map((item) => redactConfigImpl(item, seen));
    seen.delete(config);
    return result;
  }
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(config as Record<string, unknown>)) {
    const lower = key.toLowerCase();
    if (
      lower.includes('token') ||
      lower.includes('secret') ||
      lower.includes('credential') ||
      lower.includes('password') ||
      lower.includes('apikey') ||
      lower.includes('auth')
    ) {
      result[key] = '<redacted>';
    } else if (typeof value === 'string' && (lower.endsWith('url') || lower.endsWith('uri'))) {
      // Always run explicit URL/URI fields through sanitizeUrl, even if the
      // scheme is not http(s). This prevents data:, blob:, file: and other
      // scheme payloads from leaking in diagnostic bundles.
      result[key] = sanitizeUrl(value);
    } else if (typeof value === 'string' && looksLikeUrl(value)) {
      result[key] = sanitizeUrl(value);
    } else if (typeof value === 'object' && value !== null) {
      if (lower === 'headers') {
        result[key] = redactHeaders(value as Record<string, string>);
      } else {
        result[key] = redactConfigImpl(value, seen);
      }
    } else {
      result[key] = value;
    }
  }
  seen.delete(config);
  return result;
}

function looksLikeUrl(value: string): boolean {
  return value.length < 2048 && /^https?:\/\//i.test(value);
}

function truncateTrace(span: TraceSpan, depth: number): TraceSpan {
  if (depth <= 0) {
    return { ...span, children: [] };
  }
  return {
    ...span,
    children: span.children.map((child) => truncateTrace(child, depth - 1)),
  };
}

function degradedBundle(bundle: DiagnosticsBundle): DiagnosticsBundle {
  return {
    ...bundle,
    recentEvents: [],
    metrics: undefined,
    trace: undefined,
    config: '<unserializable>',
  };
}

function fitToSize(bundle: DiagnosticsBundle, maxSizeBytes: number): DiagnosticsBundle {
  let events = bundle.recentEvents;
  let current = bundle;
  while (events.length > 0) {
    let json: string;
    try {
      json = JSON.stringify(current);
    } catch {
      // Non-serializable values (BigInt, circular references) make size trimming
      // impossible. Return a degraded but serializable bundle instead of crashing.
      return degradedBundle(bundle);
    }
    if (roughSize(json) <= maxSizeBytes - SIZE_SAFETY_MARGIN) {
      break;
    }
    events = events.slice(Math.max(1, Math.floor(events.length / 4)));
    current = { ...bundle, recentEvents: events };
  }
  // Always do a final serializability and size check, even when there are no
  // events to trim. This catches BigInt/circular values in config/metrics/trace.
  try {
    const json = JSON.stringify(current);
    if (roughSize(json) <= maxSizeBytes - SIZE_SAFETY_MARGIN) {
      if (events.length !== bundle.recentEvents.length) {
        return current;
      }
      return bundle;
    }
  } catch {
    // fall through to degraded bundle
  }
  return degradedBundle(bundle);
}

function roughSize(json: string): number {
  // One UTF-16 code unit per character is an upper bound for ASCII/JSON.
  return json.length * 2;
}

function nowMs(): number {
  return typeof performance !== 'undefined' ? performance.now() : Date.now();
}
