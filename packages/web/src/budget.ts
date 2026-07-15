/**
 * Global resource budget controller for multiview (CheetahWall) scenarios.
 *
 * The controller collects per-cell demand profiles and returns an allocation
 * that respects global limits for decoders, pixel rate, network bandwidth and
 * audio outputs. It does not touch media directly; the wall component applies
 * the allocation by loading the appropriate source or switching variants.
 */

export interface Resolution {
  readonly width: number;
  readonly height: number;
}

export interface StreamProfile {
  readonly resolution: Resolution;
  readonly fps: number;
  readonly codec: string;
  readonly estimatedMbps: number;
  readonly backend: 'hardware' | 'software';
}

export interface CellDemand {
  readonly id: string;
  /** Lower numbers mean higher priority. */
  readonly priority: number;
  readonly visible: boolean;
  readonly main: StreamProfile;
  readonly sub: StreamProfile;
  readonly audio: boolean;
}

export interface CellAllocation {
  readonly id: string;
  readonly allowed: boolean;
  readonly quality: 'main' | 'sub' | 'pause';
  readonly resolution: Resolution;
  readonly fps: number;
  readonly backend: 'hardware' | 'software' | null;
  readonly reason: string;
}

export interface ResourceBudgetConfig {
  readonly maxHardwareDecoders?: number;
  readonly maxSoftwareDecoders?: number;
  readonly maxTotalPixelRate?: number;
  readonly maxNetworkMbps?: number;
  readonly maxAudioOutputs?: number;
  /** Minimum milliseconds a cell must stay on main before being degraded to sub. */
  readonly mainDwellMs?: number;
  /** Minimum milliseconds a cell must stay on sub before being promoted to main. */
  readonly subDwellMs?: number;
}

interface ActiveCell {
  demand: CellDemand;
  basePriority: number;
  fullscreen: boolean;
  currentQuality: 'main' | 'sub' | 'pause';
  qualitySince: number;
}

const DEFAULT_CONFIG: Required<ResourceBudgetConfig> = {
  maxHardwareDecoders: Number.POSITIVE_INFINITY,
  maxSoftwareDecoders: Number.POSITIVE_INFINITY,
  maxTotalPixelRate: Number.POSITIVE_INFINITY,
  maxNetworkMbps: Number.POSITIVE_INFINITY,
  maxAudioOutputs: Number.POSITIVE_INFINITY,
  mainDwellMs: 0,
  subDwellMs: 0,
};

function resolveProfile(demand: CellDemand, quality: 'main' | 'sub'): StreamProfile {
  return quality === 'main' ? demand.main : demand.sub;
}

function pixelRate(profile: StreamProfile): number {
  return profile.resolution.width * profile.resolution.height * profile.fps;
}

interface Usage {
  hardwareDecoders: number;
  softwareDecoders: number;
  pixelRate: number;
  networkMbps: number;
  audioOutputs: number;
}

function emptyUsage(): Usage {
  return {
    hardwareDecoders: 0,
    softwareDecoders: 0,
    pixelRate: 0,
    networkMbps: 0,
    audioOutputs: 0,
  };
}

function addUsage(usage: Usage, demand: CellDemand, quality: 'main' | 'sub'): Usage {
  const profile = resolveProfile(demand, quality);
  if (profile.backend === 'hardware') {
    usage.hardwareDecoders += 1;
  } else {
    usage.softwareDecoders += 1;
  }
  usage.pixelRate += pixelRate(profile);
  usage.networkMbps += profile.estimatedMbps;
  if (demand.audio) {
    usage.audioOutputs += 1;
  }
  return usage;
}

function exceeds(usage: Usage, config: Required<ResourceBudgetConfig>): string | null {
  if (Number.isFinite(config.maxHardwareDecoders) && usage.hardwareDecoders > config.maxHardwareDecoders) {
    return `hardware decoders ${usage.hardwareDecoders} > ${config.maxHardwareDecoders}`;
  }
  if (Number.isFinite(config.maxSoftwareDecoders) && usage.softwareDecoders > config.maxSoftwareDecoders) {
    return `software decoders ${usage.softwareDecoders} > ${config.maxSoftwareDecoders}`;
  }
  if (Number.isFinite(config.maxTotalPixelRate) && usage.pixelRate > config.maxTotalPixelRate) {
    return `pixel rate ${usage.pixelRate} > ${config.maxTotalPixelRate}`;
  }
  if (Number.isFinite(config.maxNetworkMbps) && usage.networkMbps > config.maxNetworkMbps) {
    return `network ${usage.networkMbps.toFixed(2)} Mbps > ${config.maxNetworkMbps}`;
  }
  if (Number.isFinite(config.maxAudioOutputs) && usage.audioOutputs > config.maxAudioOutputs) {
    return `audio outputs ${usage.audioOutputs} > ${config.maxAudioOutputs}`;
  }
  return null;
}

export class BudgetController {
  private config: Required<ResourceBudgetConfig>;
  private readonly cells = new Map<string, ActiveCell>();
  private onChangeHandler: ((allocations: ReadonlyMap<string, CellAllocation>) => void) | undefined;

  constructor(config: ResourceBudgetConfig = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  setConfig(config: ResourceBudgetConfig): void {
    this.config = { ...DEFAULT_CONFIG, ...config };
    this.allocate();
  }

  addCell(demand: CellDemand): void {
    this.cells.set(demand.id, {
      demand,
      basePriority: demand.priority,
      fullscreen: false,
      currentQuality: 'pause',
      qualitySince: 0,
    });
    this.allocate();
  }

  removeCell(id: string): void {
    if (this.cells.delete(id)) {
      this.allocate();
    }
  }

  updateCell(demand: CellDemand): void {
    const cell = this.cells.get(demand.id);
    if (cell) {
      cell.demand = demand;
      this.allocate();
    }
  }

  setPriority(id: string, priority: number): void {
    const cell = this.cells.get(id);
    if (cell) {
      cell.basePriority = priority;
      if (!cell.fullscreen) {
        cell.demand = { ...cell.demand, priority };
      }
      this.allocate();
    }
  }

  setVisible(id: string, visible: boolean): void {
    const cell = this.cells.get(id);
    if (cell) {
      cell.demand = { ...cell.demand, visible };
      this.allocate();
    }
  }

  setFullscreen(id: string, fullscreen: boolean): void {
    const cell = this.cells.get(id);
    if (cell && cell.fullscreen !== fullscreen) {
      cell.fullscreen = fullscreen;
      const priority = fullscreen ? 0 : cell.basePriority;
      cell.demand = { ...cell.demand, priority };
      this.allocate();
    }
  }

  onChange(handler: (allocations: ReadonlyMap<string, CellAllocation>) => void): void {
    this.onChangeHandler = handler;
  }

  allocate(): ReadonlyMap<string, CellAllocation> {
    const now = performance.now();
    const sorted = Array.from(this.cells.values()).sort((a, b) => {
      if (a.demand.priority !== b.demand.priority) {
        return a.demand.priority - b.demand.priority;
      }
      return a.demand.id.localeCompare(b.demand.id);
    });

    const allocations = new Map<string, CellAllocation>();
    const usage = emptyUsage();

    for (const cell of sorted) {
      if (!cell.demand.visible) {
        cell.currentQuality = 'pause';
        cell.qualitySince = now;
        allocations.set(cell.demand.id, this.makeAllocation(cell, 'pause', 'cell not visible'));
        continue;
      }

      const oldQuality = cell.currentQuality;
      const preferred = this.preferredQuality(cell, now);
      const trial: Usage = { ...usage };
      addUsage(trial, cell.demand, preferred);
      const over = exceeds(trial, this.config);

      if (!over) {
        cell.currentQuality = preferred;
        if (preferred !== oldQuality) {
          cell.qualitySince = now;
        }
        Object.assign(usage, trial);
        allocations.set(cell.demand.id, this.makeAllocation(cell, preferred, 'allocated'));
        continue;
      }

      const subTrial: Usage = { ...usage };
      addUsage(subTrial, cell.demand, 'sub');
      const subOver = exceeds(subTrial, this.config);

      if (!subOver) {
        cell.currentQuality = 'sub';
        if ('sub' !== oldQuality) {
          cell.qualitySince = now;
        }
        Object.assign(usage, subTrial);
        allocations.set(cell.demand.id, this.makeAllocation(cell, 'sub', `main over budget (${over})`));
        continue;
      }

      cell.currentQuality = 'pause';
      if ('pause' !== oldQuality) {
        cell.qualitySince = now;
      }
      allocations.set(cell.demand.id, this.makeAllocation(cell, 'pause', `main and sub over budget (${over})`));
    }

    if (this.onChangeHandler) {
      this.onChangeHandler(allocations);
    }
    return allocations;
  }

  private preferredQuality(cell: ActiveCell, now: number): 'main' | 'sub' {
    if (cell.currentQuality === 'pause') {
      return 'sub';
    }
    const dwell = now - cell.qualitySince;
    if (cell.currentQuality === 'sub' && dwell < this.config.subDwellMs) {
      return 'sub';
    }
    if (cell.currentQuality === 'main' && dwell < this.config.mainDwellMs) {
      return 'main';
    }
    return 'main';
  }

  private makeAllocation(cell: ActiveCell, quality: 'main' | 'sub' | 'pause', reason: string): CellAllocation {
    if (quality === 'pause') {
      return {
        id: cell.demand.id,
        allowed: false,
        quality: 'pause',
        resolution: { width: 0, height: 0 },
        fps: 0,
        backend: null,
        reason,
      };
    }
    const profile = resolveProfile(cell.demand, quality);
    return {
      id: cell.demand.id,
      allowed: true,
      quality,
      resolution: profile.resolution,
      fps: profile.fps,
      backend: profile.backend,
      reason,
    };
  }
}
