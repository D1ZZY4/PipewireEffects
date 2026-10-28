//! Level conversions and parameter smoothing.
//!
//! Two things in here matter for audible quality:
//!
//! * **Decibels.** Every level the user sees or sets is in dB, because that is
//!   how hearing works. The DSP works in linear gain, so the conversions are
//!   used on the boundary and nowhere else.
//! * **Ramping.** A gain that jumps between blocks clicks. [`Ramp`] spreads a
//!   change linearly across the whole block, which is inaudible at any block
//!   size PipeWire will realistically hand us.

use std::f32::consts::PI;

/// The smallest level we are willing to call "audible".
///
/// `linear_to_db` clamps here rather than returning `-inf`, so that a silent
/// signal still produces a finite number the interface can draw.
pub const SILENCE_FLOOR_DB: f32 = -180.0;

/// Converts a level in decibels to a linear amplitude multiplier.
#[must_use]
pub fn db_to_linear(db: f32) -> f32 {
    if db <= SILENCE_FLOOR_DB {
        0.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}

/// Converts a linear amplitude multiplier to decibels.
#[must_use]
pub fn linear_to_db(linear: f32) -> f32 {
    if !linear.is_finite() || linear <= 0.0 {
        SILENCE_FLOOR_DB
    } else {
        (20.0 * linear.log10()).max(SILENCE_FLOOR_DB)
    }
}

/// Scales a linear amplitude by a number of decibels.
#[must_use]
pub fn apply_db(linear: f32, db: f32) -> f32 {
    linear * db_to_linear(db)
}

/// The one-pole coefficient that achieves a given time constant.
///
/// Shared by [`Smoother`] and by effects that keep their own per-channel
/// envelopes but still want the same time-constant semantics.
#[must_use]
pub fn one_pole_coefficient(time_ms: f32, sample_rate: f32) -> f32 {
    let samples = (time_ms.max(0.0) * 0.001 * sample_rate).max(1.0);
    1.0 - (-1.0 / samples).exp()
}

/// A simple one-pole smoother, for parameters that are read per sample.
///
/// The time constant is expressed in milliseconds so that effect parameters
/// can expose it directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Smoother {
    value: f32,
    coefficient: f32,
}

impl Smoother {
    /// Creates a smoother starting at `value`, with the given time constant.
    #[must_use]
    pub fn new(value: f32, time_ms: f32, sample_rate: f32) -> Self {
        let mut smoother = Self {
            value,
            coefficient: 1.0,
        };
        smoother.set_time_constant(time_ms, sample_rate);
        smoother
    }

    /// Recomputes the filter coefficient for a new time constant.
    pub fn set_time_constant(&mut self, time_ms: f32, sample_rate: f32) {
        self.coefficient = one_pole_coefficient(time_ms, sample_rate);
    }

    /// The current smoothed value.
    #[must_use]
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Jumps immediately to `value`.
    pub fn reset(&mut self, value: f32) {
        self.value = value;
    }

    /// Advances one sample towards `target` and returns the new value.
    pub fn process(&mut self, target: f32) -> f32 {
        self.value += (target - self.value) * self.coefficient;
        self.value
    }

    /// Advances `samples` steps towards `target` at once.
    ///
    /// Cheaper than calling [`Smoother::process`] in a loop when the caller
    /// only needs the value once per block.
    pub fn advance(&mut self, target: f32, samples: usize) -> f32 {
        let retained = (1.0 - self.coefficient).powi(i32::try_from(samples).unwrap_or(i32::MAX));
        self.value = target + (self.value - target) * retained;
        self.value
    }

    /// Whether the smoother has effectively reached `target`.
    #[must_use]
    pub fn is_settled(&self, target: f32, tolerance: f32) -> bool {
        (self.value - target).abs() <= tolerance
    }
}

/// A linear ramp across one block.
///
/// [`Ramp::begin`] freezes the start and end values for the current block and
/// derives the per-frame increment; [`Ramp::next`] then walks the range. This
/// gives click-free gain changes without a per-sample one-pole filter, and it
/// reaches the target exactly at the end of the block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    current: f32,
    target: f32,
    step: f32,
}

impl Ramp {
    /// Creates a ramp that starts — and stays — at `value`.
    #[must_use]
    pub const fn new(value: f32) -> Self {
        Self {
            current: value,
            target: value,
            step: 0.0,
        }
    }

    /// The value the ramp is moving towards.
    #[must_use]
    pub const fn target(&self) -> f32 {
        self.target
    }

    /// The value the ramp is currently emitting.
    #[must_use]
    pub const fn current(&self) -> f32 {
        self.current
    }

    /// Sets the destination for the next block.
    pub const fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Whether the ramp has nothing left to do.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.step == 0.0 && (self.current - self.target).abs() <= f32::EPSILON
    }

    /// Jumps to a value with no ramping.
    pub const fn reset(&mut self, value: f32) {
        self.current = value;
        self.target = value;
        self.step = 0.0;
    }

    /// Prepares the ramp for a block of `frames` frames.
    pub fn begin(&mut self, frames: usize) {
        if frames == 0 {
            self.current = self.target;
            self.step = 0.0;
            return;
        }
        #[allow(clippy::cast_precision_loss)]
        let frames = frames as f32;
        self.step = (self.target - self.current) / frames;
    }

    /// Returns the value for the current frame and advances.
    pub fn next_value(&mut self) -> f32 {
        let value = self.current;
        if self.step != 0.0 {
            self.current += self.step;
        }
        value
    }

    /// Snaps to the target once the block has been consumed.
    pub fn finish(&mut self) {
        self.current = self.target;
        self.step = 0.0;
    }
}

/// The angular frequency of `frequency` in radians per sample.
///
/// The frequency is clamped to the audible part of the spectrum so that a
/// malformed parameter cannot produce a `NaN` filter.
#[must_use]
pub fn angular_frequency(frequency: f32, sample_rate: f32) -> f32 {
    if !frequency.is_finite() || sample_rate <= 0.0 {
        return 0.0;
    }
    let nyquist = sample_rate * 0.5;
    let clamped = frequency.clamp(1.0, (nyquist - 1.0).max(2.0));
    (2.0 * PI * clamped / sample_rate).clamp(1e-6, PI - 1e-6)
}

/// Replaces a subnormal or non-finite value with zero.
///
/// Two problems, one guard:
///
/// * **Denormals** are dramatically slower to compute with on mainstream CPUs,
///   and any filter fed silence eventually decays into them.
/// * **A single non-finite sample** — a glitch upstream, or a hostile parameter
///   combination — would otherwise park a recursive filter in a state that
///   produces `NaN` for the rest of the stream's life.
///
/// Applied to filter state once per block rather than per sample, so it costs
/// nothing measurable in the inner loop.
#[must_use]
pub fn sanitise_value(value: f32) -> f32 {
    if value.is_finite() && !value.is_subnormal() {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decibel_conversions_match_known_values() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
        assert!((db_to_linear(6.0) - 1.995_262).abs() < 1e-5);
        assert!((db_to_linear(-6.0) - 0.501_187).abs() < 1e-5);
        assert!((db_to_linear(20.0) - 10.0).abs() < 1e-5);
        assert!((linear_to_db(0.5) + 6.0206).abs() < 1e-3);
        assert!((linear_to_db(10.0) - 20.0).abs() < 1e-4);
    }

    #[test]
    fn silence_maps_to_the_floor_rather_than_negative_infinity() {
        assert_eq!(linear_to_db(0.0), SILENCE_FLOOR_DB);
        assert_eq!(linear_to_db(-1.0), SILENCE_FLOOR_DB);
        assert_eq!(linear_to_db(f32::NAN), SILENCE_FLOOR_DB);
        assert_eq!(db_to_linear(-1_000.0), 0.0);
    }

    #[test]
    fn decibel_conversions_round_trip() {
        for db in [-60.0_f32, -12.0, -0.5, 0.0, 3.0, 24.0] {
            assert!((linear_to_db(db_to_linear(db)) - db).abs() < 1e-3);
        }
    }

    #[test]
    fn a_ramp_reaches_its_target_exactly_at_the_end_of_the_block() {
        let mut ramp = Ramp::new(0.0);
        ramp.set_target(1.0);
        ramp.begin(100);
        for _ in 0..100 {
            ramp.next_value();
        }
        ramp.finish();
        assert!((ramp.current() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_ramp_interpolates_monotonically() {
        let mut ramp = Ramp::new(0.0);
        ramp.set_target(1.0);
        ramp.begin(4);
        assert!(ramp.next_value() < ramp.next_value());
        assert!(ramp.next_value() < ramp.next_value());
    }

    #[test]
    fn an_empty_block_leaves_the_ramp_at_its_target() {
        let mut ramp = Ramp::new(0.0);
        ramp.set_target(2.0);
        ramp.begin(0);
        assert!((ramp.current() - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_smoother_converges_towards_its_target() {
        let mut smoother = Smoother::new(0.0, 1.0, 48_000.0);
        for _ in 0..48_000 {
            smoother.process(1.0);
        }
        assert!(smoother.is_settled(1.0, 1e-3));
    }

    #[test]
    fn advancing_by_a_whole_block_matches_sample_by_sample_stepping() {
        let mut stepped = Smoother::new(0.0, 10.0, 48_000.0);
        let mut blocked = Smoother::new(0.0, 10.0, 48_000.0);
        for _ in 0..480 {
            stepped.process(1.0);
        }
        blocked.advance(1.0, 480);
        assert!((stepped.value() - blocked.value()).abs() < 1e-3);
    }

    #[test]
    fn angular_frequency_is_clamped_away_from_the_extremes() {
        assert!(angular_frequency(0.0, 48_000.0) > 0.0);
        assert!(angular_frequency(1e9, 48_000.0) < PI);
        assert_eq!(angular_frequency(f32::NAN, 48_000.0), 0.0);
    }

    #[test]
    fn subnormal_and_non_finite_values_are_zeroed() {
        assert_eq!(sanitise_value(1e-42), 0.0);
        assert_eq!(sanitise_value(f32::NAN), 0.0);
        assert_eq!(sanitise_value(f32::INFINITY), 0.0);
        assert_eq!(sanitise_value(f32::NEG_INFINITY), 0.0);
        assert_eq!(sanitise_value(1e-3), 1e-3);
        assert_eq!(sanitise_value(0.0), 0.0);
    }
}
