//! PCR clock recovery, drift, jitter, and live edge tracking.

/// 27 MHz clock ticks per second.
const PCR_HZ: u64 = 27_000_000;

/// Diagnostic state for the transport clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockState {
    pub pcr_hz: u64,
    pub last_pcr: Option<u64>,
    pub last_wall_ms: Option<u64>,
    pub jitter_hz: Option<u64>,
    pub drift_hz_per_s: Option<i64>,
    pub live_edge_ms: Option<u64>,
}

impl Default for ClockState {
    fn default() -> Self {
        Self {
            pcr_hz: PCR_HZ,
            last_pcr: None,
            last_wall_ms: None,
            jitter_hz: None,
            drift_hz_per_s: None,
            live_edge_ms: None,
        }
    }
}

/// PCR clock recovery using a simple low-pass filter.
#[derive(Debug, Default)]
pub struct PcrClock {
    state: ClockState,
    /// Last PCR in 27 MHz ticks.
    last_pcr: Option<u64>,
    /// Last local arrival time in milliseconds.
    last_arrival_ms: Option<u64>,
    /// Smoothed jitter estimate.
    jitter_hz: u64,
    /// Smoothed drift in Hz per second.
    drift_hz_per_s: i64,
    /// Count of samples used for smoothing.
    sample_count: u64,
}

impl PcrClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a new PCR value and optional wall clock (ms since an arbitrary epoch).
    ///
    /// `pcr` is in 27 MHz ticks. If `wall_ms` is `None`, wall is assumed equal to PCR.
    pub fn feed(&mut self, pcr: u64, wall_ms: Option<u64>) -> ClockState {
        let wall = wall_ms.unwrap_or_else(|| pcr_to_ms(pcr));

        if let (Some(last_pcr), Some(last_wall)) = (self.last_pcr, self.last_arrival_ms)
            && pcr > last_pcr
            && wall > last_wall
        {
            let delta_pcr = pcr - last_pcr;
            let delta_wall = (wall - last_wall) * (PCR_HZ / 1000);
            // Difference between observed PCR spacing and wall spacing.
            let diff: i64 = delta_pcr as i64 - delta_wall as i64;
            // Update jitter estimate (EWMA-ish absolute deviation).
            self.jitter_hz = ((self.jitter_hz * self.sample_count) + diff.unsigned_abs())
                / (self.sample_count + 1);
            if self.sample_count > 0 {
                // Drift as Hz per second over the interval.
                let interval_s = (wall - last_wall).max(1);
                self.drift_hz_per_s = (diff as i128 * 1000 / interval_s as i128) as i64;
            }
        }
        self.sample_count += 1;
        self.last_pcr = Some(pcr);
        self.last_arrival_ms = Some(wall);

        self.state = ClockState {
            pcr_hz: PCR_HZ,
            last_pcr: self.last_pcr,
            last_wall_ms: self.last_arrival_ms,
            jitter_hz: if self.sample_count > 1 {
                Some(self.jitter_hz)
            } else {
                None
            },
            drift_hz_per_s: if self.sample_count > 1 {
                Some(self.drift_hz_per_s)
            } else {
                None
            },
            live_edge_ms: Some(wall),
        };
        self.state
    }

    /// Return the latest clock state.
    pub fn state(&self) -> ClockState {
        self.state
    }
}

/// Convert a 27 MHz PCR value to milliseconds.
pub fn pcr_to_ms(pcr: u64) -> u64 {
    // 27 MHz / 300 = 90 kHz tick per second. pcr is already 27 MHz base*300+ext.
    // pcr / 27_000 gives seconds, remainder gives fractional.
    pcr / (PCR_HZ / 1000)
}

/// Convert milliseconds to a 27 MHz PCR value (no wrap handling).
pub fn ms_to_pcr(ms: u64) -> u64 {
    ms * (PCR_HZ / 1000)
}
