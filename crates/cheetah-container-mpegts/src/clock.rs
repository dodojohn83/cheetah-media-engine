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
            let delta_wall = (wall - last_wall).saturating_mul(PCR_HZ / 1000);
            // Difference between observed PCR spacing and wall spacing.
            let diff = (delta_pcr as i128).saturating_sub(delta_wall as i128);
            let abs_diff = u64::try_from(diff.saturating_abs()).unwrap_or(u64::MAX);
            // Update jitter estimate (EWMA-ish absolute deviation).
            let count = self.sample_count;
            let denom = count.saturating_add(1).max(1);
            let numerator = (self.jitter_hz as u128)
                .saturating_mul(count as u128)
                .saturating_add(abs_diff as u128);
            self.jitter_hz = u64::try_from(numerator / denom as u128).unwrap_or(u64::MAX);
            if self.sample_count > 0 {
                // Drift as Hz per second over the interval.
                let interval_s = (wall - last_wall).max(1);
                let drift = diff.saturating_mul(1000) / (interval_s as i128);
                self.drift_hz_per_s =
                    i64::try_from(drift).unwrap_or(if drift >= 0 { i64::MAX } else { i64::MIN });
            }
        }
        self.sample_count = self.sample_count.saturating_add(1);
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
