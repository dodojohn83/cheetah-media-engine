/**
 * Lightweight metrics registry for the media runtime.
 *
 * Supports counters, gauges and histograms grouped into the engine's nine
 * observability categories. Histogram percentiles are derived from bucket
 * counts, not from an average, and the registry can be snapshotted or reset
 * without blocking the render queue.
 */

export type MetricCategory =
  | 'source'
  | 'demux'
  | 'timeline'
  | 'decode'
  | 'render'
  | 'audio'
  | 'record'
  | 'memory'
  | 'fallback';

export type MetricType = 'counter' | 'gauge' | 'histogram';

export interface MetricDefinition {
  readonly name: string;
  readonly type: MetricType;
  readonly category: MetricCategory;
  readonly unit?: string | undefined;
}

export interface CounterSnapshot {
  readonly type: 'counter';
  readonly value: number;
  readonly unit?: string | undefined;
}

export interface GaugeSnapshot {
  readonly type: 'gauge';
  readonly value: number;
  readonly unit?: string | undefined;
}

export interface HistogramBucket {
  readonly upperBound: number;
  readonly count: number;
}

export interface HistogramSnapshot {
  readonly type: 'histogram';
  readonly count: number;
  readonly sum: number;
  readonly min: number;
  readonly max: number;
  readonly buckets: readonly HistogramBucket[];
  readonly percentiles: { readonly p50: number; readonly p95: number; readonly p99: number };
  readonly unit?: string | undefined;
}

export type MetricSnapshot = CounterSnapshot | GaugeSnapshot | HistogramSnapshot;

export interface MetricsSnapshot {
  readonly timestamp: number;
  readonly metrics: { readonly [category in MetricCategory]?: Readonly<Record<string, MetricSnapshot>> };
}

const DEFAULT_HISTOGRAM_BUCKETS = [
  0, 1, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000, 30000, 60000,
];

interface BaseMetric {
  readonly definition: MetricDefinition;
  reset(): void;
  snapshot(): MetricSnapshot;
}

class Counter implements BaseMetric {
  private value = 0;

  constructor(readonly definition: MetricDefinition) {}

  inc(delta = 1): void {
    this.value += delta;
  }

  reset(): void {
    this.value = 0;
  }

  snapshot(): CounterSnapshot {
    return { type: 'counter', value: this.value, unit: this.definition.unit };
  }
}

class Gauge implements BaseMetric {
  private value = 0;

  constructor(readonly definition: MetricDefinition) {}

  set(value: number): void {
    this.value = value;
  }

  reset(): void {
    this.value = 0;
  }

  snapshot(): GaugeSnapshot {
    return { type: 'gauge', value: this.value, unit: this.definition.unit };
  }
}

class Histogram implements BaseMetric {
  private buckets: number[];
  private counts: number[];
  private total = 0;
  private sum = 0;
  private min = Infinity;
  private max = -Infinity;

  constructor(
    readonly definition: MetricDefinition,
    bucketUpperBounds: readonly number[] = DEFAULT_HISTOGRAM_BUCKETS,
  ) {
    this.buckets = [...bucketUpperBounds].sort((a, b) => a - b);
    this.counts = new Array(this.buckets.length).fill(0);
  }

  record(value: number): void {
    if (!Number.isFinite(value) || value < 0) return;
    this.total += 1;
    this.sum += value;
    this.min = Math.min(this.min, value);
    this.max = Math.max(this.max, value);
    for (let i = 0; i < this.buckets.length; i += 1) {
      const bucket = this.buckets[i]!;
      if (value <= bucket) {
        this.counts[i] = (this.counts[i] ?? 0) + 1;
        return;
      }
    }
    // Value exceeds the last finite bucket; add an implicit +Inf bucket at end.
    const lastIndex = this.counts.length - 1;
    this.counts[lastIndex] = (this.counts[lastIndex] ?? 0) + 1;
  }

  reset(): void {
    this.counts.fill(0);
    this.total = 0;
    this.sum = 0;
    this.min = Infinity;
    this.max = -Infinity;
  }

  snapshot(): HistogramSnapshot {
    const buckets: HistogramBucket[] = [];
    let cumulative = 0;
    for (let i = 0; i < this.buckets.length; i += 1) {
      cumulative += this.counts[i]!;
      buckets.push({ upperBound: this.buckets[i]!, count: cumulative });
    }
    return {
      type: 'histogram',
      count: this.total,
      sum: this.sum,
      min: this.total === 0 ? 0 : this.min,
      max: this.total === 0 ? 0 : this.max,
      buckets,
      percentiles: {
        p50: this.percentile(0.5),
        p95: this.percentile(0.95),
        p99: this.percentile(0.99),
      },
      unit: this.definition.unit,
    };
  }

  private percentile(p: number): number {
    if (this.total === 0) return 0;
    const target = p * this.total;
    let previousCount = 0;
    let previousBound = 0;
    for (let i = 0; i < this.buckets.length; i += 1) {
      const count = this.counts[i]!;
      const bucketUpper = this.buckets[i]!;
      const cumulative = previousCount + count;
      if (cumulative >= target) {
        const bucketLower = previousBound;
        if (count === 0 || bucketUpper === bucketLower) return bucketUpper;
        const fraction = (target - previousCount) / count;
        return bucketLower + fraction * (bucketUpper - bucketLower);
      }
      previousCount = cumulative;
      previousBound = bucketUpper;
    }
    return this.buckets[this.buckets.length - 1]!;
  }
}

export class MetricRegistry {
  private metrics = new Map<string, BaseMetric>();
  private readonly buckets: readonly number[];

  constructor(options: { readonly histogramBuckets?: readonly number[] } = {}) {
    this.buckets = options.histogramBuckets ?? DEFAULT_HISTOGRAM_BUCKETS;
  }

  counter(name: string, category: MetricCategory, unit?: string): Counter {
    return this.getOrCreate(name, category, 'counter', unit) as Counter;
  }

  gauge(name: string, category: MetricCategory, unit?: string): Gauge {
    return this.getOrCreate(name, category, 'gauge', unit) as Gauge;
  }

  histogram(name: string, category: MetricCategory, unit?: string): Histogram {
    return this.getOrCreate(name, category, 'histogram', unit) as Histogram;
  }

  snapshot(): MetricsSnapshot {
    const acc: Partial<Record<MetricCategory, Readonly<Record<string, MetricSnapshot>>>> = {};
    for (const metric of this.metrics.values()) {
      const { category, name } = metric.definition;
      const byName = acc[category] ?? {};
      acc[category] = { ...byName, [name]: metric.snapshot() };
    }
    return { timestamp: nowMs(), metrics: acc as MetricsSnapshot['metrics'] };
  }

  reset(category?: MetricCategory): void {
    for (const metric of this.metrics.values()) {
      if (category === undefined || metric.definition.category === category) {
        metric.reset();
      }
    }
  }

  private getOrCreate(
    name: string,
    category: MetricCategory,
    type: MetricType,
    unit?: string,
  ): BaseMetric {
    const existing = this.metrics.get(name);
    if (existing) {
      if (existing.definition.type !== type || existing.definition.category !== category) {
        throw new Error(`Metric ${name} already registered with different type or category`);
      }
      return existing;
    }
    const definition: MetricDefinition = { name, type, category, unit };
    let metric: BaseMetric;
    if (type === 'counter') {
      metric = new Counter(definition);
    } else if (type === 'gauge') {
      metric = new Gauge(definition);
    } else {
      metric = new Histogram(definition, this.buckets);
    }
    this.metrics.set(name, metric);
    return metric;
  }
}

function nowMs(): number {
  return typeof performance !== 'undefined' ? performance.now() : Date.now();
}
