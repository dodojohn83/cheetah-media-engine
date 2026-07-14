//! Timebase, timestamps, durations, and `MediaTime`.

use crate::MediaError;

/// A rational timebase representing seconds per tick as `num / den`.
///
/// `TimeBase` is always stored in reduced form with `den != 0` and `num > 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeBase {
    num: u64,
    den: u64,
}

impl TimeBase {
    /// The default 1 kHz timebase used when no other timebase is provided.
    pub const DEFAULT: Self = Self { num: 1, den: 1000 };

    /// A 90 kHz timebase commonly used in MPEG-TS and HLS.
    pub const TS_90K: Self = Self {
        num: 1,
        den: 90_000,
    };

    /// Create a new `TimeBase` from a reduced fraction `num / den`.
    ///
    /// Returns `None` if `num == 0` or `den == 0`.
    pub const fn new(num: u64, den: u64) -> Option<Self> {
        if num == 0 || den == 0 {
            return None;
        }
        let g = gcd_u64(num, den);
        Some(Self {
            num: num / g,
            den: den / g,
        })
    }

    /// Create a timebase where `den` ticks equal one second (`num` is 1).
    ///
    /// Equivalent to `TimeBase::new(1, ticks_per_second)`.
    pub const fn from_timescale(ticks_per_second: u32) -> Option<Self> {
        Self::new(1, ticks_per_second as u64)
    }

    /// Numerator of the reduced fraction.
    pub const fn num(self) -> u64 {
        self.num
    }

    /// Denominator of the reduced fraction (always non-zero).
    pub const fn den(self) -> u64 {
        self.den
    }

    /// Number of ticks per second, if the numerator is 1.
    pub const fn ticks_per_second(self) -> Option<u64> {
        if self.num == 1 { Some(self.den) } else { None }
    }

    /// Rescale `value` from `self` to `target` using checked arithmetic.
    ///
    /// The calculation is `value * self.num * target.den / (self.den * target.num)`.
    /// Truncation is toward zero, consistent with integer division.
    pub fn rescale_i64(self, value: i64, target: Self) -> Result<i64, MediaError> {
        if self == target {
            return Ok(value);
        }
        // Compute with i128 intermediate to detect overflow and improve precision.
        let num = i128::from(value)
            .checked_mul(i128::from(self.num))
            .and_then(|v| v.checked_mul(i128::from(target.den)))
            .ok_or(MediaError::InternalInvariant {
                msg: "time rescale numerator overflow",
            })?;
        let den = i128::from(self.den)
            .checked_mul(i128::from(target.num))
            .ok_or(MediaError::InternalInvariant {
                msg: "time rescale denominator overflow",
            })?;
        let result = num / den;
        if result > i128::from(i64::MAX) || result < i128::from(i64::MIN) {
            return Err(MediaError::InternalInvariant {
                msg: "time rescale result out of i64 range",
            });
        }
        Ok(result as i64)
    }
}

impl Default for TimeBase {
    fn default() -> Self {
        Self::DEFAULT
    }
}

const fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// A single timestamp measured in ticks of some `TimeBase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Create a new `Timestamp`.
    pub const fn new(ticks: i64) -> Self {
        Self(ticks)
    }

    /// The raw tick value.
    pub const fn ticks(self) -> i64 {
        self.0
    }

    /// Extend a wrapped `n`-bit timestamp using a previous unwrapped value as a hint.
    ///
    /// `wrap_bits` must be in `1..=62`. The function returns the unwrapped value
    /// that is closest to `previous` while preserving the low `wrap_bits` bits.
    /// This gives a deterministic interpretation of timestamp wrap without floating
    /// point arithmetic.
    pub fn unwrapped_around(self, previous: Self, wrap_bits: u8) -> Self {
        assert!((1..=62).contains(&wrap_bits), "wrap_bits must be in 1..=62");
        let value = i128::from(self.0);
        let prev = i128::from(previous.0);
        let mask = (1i128 << wrap_bits) - 1;
        let low = value & mask;
        let prev_low = prev & mask;
        let half = 1i128 << (wrap_bits - 1);
        let delta = low - prev_low;
        let adjust = if delta > half {
            -(1i128 << wrap_bits)
        } else if delta < -half {
            1i128 << wrap_bits
        } else {
            0
        };
        let result = (prev & !mask) + low + adjust;
        match i64::try_from(result) {
            Ok(ticks) => Self::new(ticks),
            Err(_) => self,
        }
    }

    /// True if this timestamp is within `threshold` ticks before `other`, indicating
    /// a possible wrap or discontinuity when `self` is the new value and `other` is
    /// the previous unwrapped value.
    pub const fn looks_wrapped_before(self, other: Self, threshold: i64) -> bool {
        self.0 < other.0 && (other.0 as i128 - self.0 as i128) > threshold as i128
    }
}

/// A duration measured in ticks of some `TimeBase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MediaDuration(i64);

impl MediaDuration {
    /// Create a new duration in ticks.
    pub const fn new(ticks: i64) -> Self {
        Self(ticks)
    }

    /// The raw tick value. Negative durations represent reverse playback offsets.
    pub const fn ticks(self) -> i64 {
        self.0
    }

    /// Checked addition of two durations.
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.0.checked_add(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked subtraction.
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.0.checked_sub(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }
}

/// A presentation/decoding timestamp pair with an optional duration.
///
/// All contained timestamps share the same `TimeBase`. Unknown values are represented
/// explicitly with `Option::None` instead of sentinel magic numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MediaTime {
    /// Presentation timestamp.
    pub pts: Option<Timestamp>,
    /// Decode timestamp.
    pub dts: Option<Timestamp>,
    /// Sample duration.
    pub duration: Option<Timestamp>,
    /// Timebase shared by all timestamp fields.
    pub timebase: TimeBase,
}

impl MediaTime {
    /// Create a `MediaTime` from optional timestamps and a timebase.
    pub const fn new(
        pts: Option<Timestamp>,
        dts: Option<Timestamp>,
        duration: Option<Timestamp>,
        timebase: TimeBase,
    ) -> Self {
        Self {
            pts,
            dts,
            duration,
            timebase,
        }
    }

    /// Convenience constructor when both PTS and DTS are known and equal.
    pub fn from_pts_dts(pts: Timestamp, dts: Timestamp, timebase: TimeBase) -> Self {
        Self::new(Some(pts), Some(dts), None, timebase)
    }

    /// Convenience constructor from raw tick values.
    pub fn from_ticks(
        pts: Option<i64>,
        dts: Option<i64>,
        duration: Option<i64>,
        timebase: TimeBase,
    ) -> Self {
        Self::new(
            pts.map(Timestamp::new),
            dts.map(Timestamp::new),
            duration.map(Timestamp::new),
            timebase,
        )
    }

    /// Convenience constructor from an integer timescale.
    pub fn from_timescale(
        pts: Option<i64>,
        dts: Option<i64>,
        duration: Option<i64>,
        ticks_per_second: u32,
    ) -> Result<Self, MediaError> {
        let timebase =
            TimeBase::from_timescale(ticks_per_second).ok_or(MediaError::InvalidInput {
                code: 1001,
                context: Some("timescale must be non-zero"),
            })?;
        Ok(Self::from_ticks(pts, dts, duration, timebase))
    }

    /// True when either PTS or DTS is known.
    pub const fn has_timestamp(&self) -> bool {
        self.pts.is_some() || self.dts.is_some()
    }

    /// Return the presentation timestamp in milliseconds, if it is known.
    pub fn pts_ms(&self) -> Option<i64> {
        self.pts
            .and_then(|t| self.timebase.rescale_i64(t.ticks(), TimeBase::DEFAULT).ok())
    }

    /// Return the decode timestamp in milliseconds, if it is known.
    pub fn dts_ms(&self) -> Option<i64> {
        self.dts
            .and_then(|t| self.timebase.rescale_i64(t.ticks(), TimeBase::DEFAULT).ok())
    }

    /// Rescale all known timestamps to a new timebase.
    pub fn rescale(&self, target: TimeBase) -> Result<Self, MediaError> {
        Ok(Self::new(
            self.pts
                .map(|t| self.timebase.rescale_i64(t.ticks(), target))
                .transpose()?
                .map(Timestamp::new),
            self.dts
                .map(|t| self.timebase.rescale_i64(t.ticks(), target))
                .transpose()?
                .map(Timestamp::new),
            self.duration
                .map(|t| self.timebase.rescale_i64(t.ticks(), target))
                .transpose()?
                .map(Timestamp::new),
            target,
        ))
    }

    /// Checked addition of a time offset to PTS/DTS.
    ///
    /// The sample `duration` is preserved; only timestamp positions are shifted.
    /// Returns `None` if any known timestamp overflows.
    pub fn checked_add(&self, rhs: MediaDuration) -> Option<Self> {
        let pts = match self.pts {
            Some(t) => Some(t.0.checked_add(rhs.0).map(Timestamp::new)?),
            None => None,
        };
        let dts = match self.dts {
            Some(t) => Some(t.0.checked_add(rhs.0).map(Timestamp::new)?),
            None => None,
        };
        Some(Self::new(pts, dts, self.duration, self.timebase))
    }

    /// Checked subtraction of a time offset from PTS/DTS.
    pub fn checked_sub(&self, rhs: MediaDuration) -> Option<Self> {
        let pts = match self.pts {
            Some(t) => Some(t.0.checked_sub(rhs.0).map(Timestamp::new)?),
            None => None,
        };
        let dts = match self.dts {
            Some(t) => Some(t.0.checked_sub(rhs.0).map(Timestamp::new)?),
            None => None,
        };
        Some(Self::new(pts, dts, self.duration, self.timebase))
    }

    /// True if `self` comes before `other` in presentation order.
    ///
    /// Returns `None` when either `pts` is unknown.
    pub fn is_before(&self, other: &Self) -> Option<bool> {
        Some(self.pts? < other.pts?)
    }

    /// Detect a 33-bit MPEG-style timestamp wrap between `previous` and `self`.
    ///
    /// Both PTS and DTS share the same 33-bit counter and are unwrapped using the
    /// corresponding previous values. If `previous.dts` is unknown, `previous.pts`
    /// is used as the reference for DTS.
    pub fn unwrapped_33bit(&self, previous: &Self) -> Self {
        let mut result = *self;
        if let (Some(pts), Some(prev_pts)) = (self.pts, previous.pts) {
            let unwrapped = pts.unwrapped_around(prev_pts, 33);
            if unwrapped != pts {
                result.pts = Some(unwrapped);
            }
        }
        if let Some(dts) = self.dts {
            let prev_dts = previous.dts.or(previous.pts);
            if let Some(prev) = prev_dts {
                let unwrapped = dts.unwrapped_around(prev, 33);
                if unwrapped != dts {
                    result.dts = Some(unwrapped);
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timebase_reduces() {
        let tb = TimeBase::new(1001, 30000).expect("valid");
        assert_eq!(tb.num(), 1001);
        assert_eq!(tb.den(), 30000);
    }

    #[test]
    fn timebase_rejects_zero() {
        assert!(TimeBase::new(0, 1000).is_none());
        assert!(TimeBase::new(1000, 0).is_none());
    }

    #[test]
    fn rescale_round_trip() {
        let tb1 = TimeBase::from_timescale(1000).unwrap();
        let tb2 = TimeBase::from_timescale(90000).unwrap();
        let value = 1000i64;
        let in_90k = tb1.rescale_i64(value, tb2).unwrap();
        let back = tb2.rescale_i64(in_90k, tb1).unwrap();
        assert_eq!(back, value);
    }

    #[test]
    fn rescale_29_97_to_90k() {
        // 1 tick at 1001/30000 -> 1 * 1001 * 90000 / (30000 * 1) = 3003
        let ntsc = TimeBase::new(1001, 30000).unwrap();
        let tb90k = TimeBase::from_timescale(90000).unwrap();
        assert_eq!(ntsc.rescale_i64(1, tb90k).unwrap(), 3003);
    }

    #[test]
    fn rescale_overflow_fails() {
        let tb = TimeBase::from_timescale(1).unwrap();
        let target = TimeBase::from_timescale(2).unwrap();
        assert!(tb.rescale_i64(i64::MAX, target).is_err());
    }

    #[test]
    fn media_time_default_ms() {
        let t = MediaTime::from_ticks(Some(3000), Some(3000), None, TimeBase::DEFAULT);
        assert_eq!(t.pts_ms(), Some(3000));
    }

    #[test]
    fn media_time_unknown_is_none() {
        let t = MediaTime::new(None, None, None, TimeBase::DEFAULT);
        assert_eq!(t.pts_ms(), None);
    }

    #[test]
    fn media_time_checked_add_overflow_returns_none() {
        let t = MediaTime::from_ticks(Some(i64::MAX), None, None, TimeBase::DEFAULT);
        assert!(t.checked_add(MediaDuration::new(1)).is_none());
    }

    #[test]
    fn media_time_checked_add_preserves_duration() {
        let t = MediaTime::from_ticks(Some(100), Some(100), Some(40), TimeBase::DEFAULT);
        let shifted = t.checked_add(MediaDuration::new(50)).expect("no overflow");
        assert_eq!(shifted.pts.map(|p| p.ticks()), Some(150));
        assert_eq!(shifted.duration.map(|d| d.ticks()), Some(40));
    }

    #[test]
    fn media_time_33bit_wrap() {
        let half = 1i64 << 32;
        let prev = MediaTime::from_ticks(Some(half + 1), Some(half + 1), None, TimeBase::DEFAULT);
        let wrapped = MediaTime::from_ticks(Some(0), Some(0), None, TimeBase::DEFAULT);
        let unwrapped = wrapped.unwrapped_33bit(&prev);
        assert_eq!(unwrapped.pts.unwrap().ticks(), half * 2);
        assert_eq!(unwrapped.dts.unwrap().ticks(), half * 2);
    }

    #[test]
    fn timestamp_unwrapped_around() {
        let prev = Timestamp::new((1i64 << 33) - 100);
        let low = Timestamp::new(100);
        let unwrapped = low.unwrapped_around(prev, 33);
        assert_eq!(unwrapped.ticks(), (1i64 << 33) + 100);
    }

    #[test]
    fn timestamp_unwrapped_around_max_bits() {
        let prev = Timestamp::new((1i64 << 62) - 100);
        let low = Timestamp::new(100);
        let unwrapped = low.unwrapped_around(prev, 62);
        assert_eq!(unwrapped.ticks(), (1i64 << 62) + 100);
    }
}
