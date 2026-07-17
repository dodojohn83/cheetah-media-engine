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
  try {
    const parsed = new URL(url);
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

export function redactHeaders(headers?: Record<string, string>): Record<string, string> {
  if (!headers) return {};
  const sensitive = new Set([
    'authorization',
    'cookie',
    'x-api-key',
    'x-auth-token',
    'x-secret',
  ]);
  const redacted: Record<string, string> = {};
  for (const [key, value] of Object.entries(headers)) {
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
  const maxEventCount = options.maxEventCount ?? 500;
  const maxSizeBytes = options.maxSizeBytes ?? 256 * 1024;

  const recentEvents = extras.events ? extras.events.slice(-maxEventCount) : [];
  const trace = options.maxTraceDepth !== undefined && extras.trace
    ? truncateTrace(extras.trace, options.maxTraceDepth)
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
  if (config === null || typeof config !== 'object') return config;
  if (Array.isArray(config)) {
    return config.map(redactConfig);
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
    } else if (typeof value === 'string' && looksLikeUrl(value)) {
      result[key] = sanitizeUrl(value);
    } else if (typeof value === 'object' && value !== null) {
      if (lower === 'headers') {
        result[key] = redactHeaders(value as Record<string, string>);
      } else {
        result[key] = redactConfig(value);
      }
    } else {
      result[key] = value;
    }
  }
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

function fitToSize(bundle: DiagnosticsBundle, maxSizeBytes: number): DiagnosticsBundle {
  let events = bundle.recentEvents;
  let current = bundle;
  while (events.length > 0 && roughSize(JSON.stringify(current)) > maxSizeBytes - SIZE_SAFETY_MARGIN) {
    events = events.slice(Math.max(1, Math.floor(events.length / 4)));
    current = { ...bundle, recentEvents: events };
  }
  if (events.length !== bundle.recentEvents.length) {
    return current;
  }
  return bundle;
}

function roughSize(json: string): number {
  // One UTF-16 code unit per character is an upper bound for ASCII/JSON.
  return json.length * 2;
}

function nowMs(): number {
  return typeof performance !== 'undefined' ? performance.now() : Date.now();
}
