//! Brick-wall peak limiter.
//!
//! The model is the one every mastering limiter uses: a **ceiling**, plus
//! **make-up gain** applied *before* the limiter so that the gain reduction
//! does the work rather than a final clip.
//!
//! ```text
//! detect on ─ follow envelope ─ apply gain ──────────────► output
//!  │
//!  └─ level = |x| · makeup ;   gain = min(1, ceiling / level)
//! ```
//!
//! Attack is instantaneous and release is a one-pole time constant. That
//! combination is what makes the limiter *brick-wall*: the moment a sample
//! would exceed the ceiling, the gain has already dropped far enough, so the
//! output can never overshoot. The cost is that very fast gain changes distort
//! transients, which is the honest trade-off of a lookahead-free design — and
//! the reason the release is exposed as a parameter.
//!
//! Because the ceiling is applied after make-up gain, a make-up setting that is
//! too aggressive is limited rather than clipped. It is not possible to make
//! this effect emit a sample above the ceiling, which is a property the tests
//! assert directly.

use pipewire_effects_core::{
    AudioBlock, AudioEffect, AudioFormat, EffectCategory, EffectDef, EffectKind, ParamSet,
    ParamSpec, ParamUnit,
};

use crate::util::{db_to_linear, one_pole_coefficient};

/// Index of the `threshold` parameter.
const PARAM_THRESHOLD: usize = 0;
/// Index of the `makeup` parameter.
const PARAM_MAKEUP: usize = 1;
/// Index of the `release` parameter.
const PARAM_RELEASE: usize = 2;
/// Index of the `linked` parameter.
const PARAM_LINKED: usize = 3;

/// The effect type.
pub const KIND: EffectKind = EffectKind::new("limiter");

/// Registry entry.
pub const DEF: EffectDef = EffectDef::new(
    KIND,
    "Limiter",
    "Brick-wall peak limiter with make-up gain and a linked stereo detector.",
    EffectCategory::Dynamics,
    LimiterEffect::boxed,
);

/// A brick-wall peak limiter.
#[derive(Debug)]
pub struct LimiterEffect {
    parameters: ParamSet,
    /// Current gain, per channel. Always `<= 1.0`.
    envelopes: Vec<f32>,
}

impl LimiterEffect {
    /// Creates a limiter at −1 dBFS with no make-up gain.
    #[must_use]
    pub fn new() -> Self {
        let mut effect = Self {
            parameters: ParamSet::new(vec![
                ParamSpec::float(
                    "threshold",
                    "Ceiling",
                    -30.0,
                    0.0,
                    -1.0,
                    0.1,
                    ParamUnit::Decibel,
                )
                .with_doc("The loudest sample the limiter will let through."),
                ParamSpec::float("makeup", "Make-up", 0.0, 24.0, 0.0, 0.1, ParamUnit::Decibel)
                    .with_doc("Gain applied before limiting, to restore loudness."),
                ParamSpec::float_log(
                    "release",
                    "Release",
                    5.0,
                    2_000.0,
                    150.0,
                    ParamUnit::Milliseconds,
                )
                .with_doc("How quickly the limiter recovers once the signal drops."),
                ParamSpec::boolean("linked", "Link channels", true).with_doc(
                    "Reduce all channels by the same amount, preserving the stereo image.",
                ),
            ]),
            envelopes: Vec::new(),
        };
        effect.prepare(AudioFormat::default());
        effect
    }

    /// Registry factory.
    #[must_use]
    pub fn boxed() -> Box<dyn AudioEffect> {
        Box::new(Self::new())
    }

    /// The ceiling as a linear amplitude.
    #[must_use]
    pub fn ceiling(&self) -> f32 {
        db_to_linear(self.parameters.value_f32(PARAM_THRESHOLD))
    }
}

impl Default for LimiterEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEffect for LimiterEffect {
    fn kind(&self) -> EffectKind {
        KIND
    }

    fn parameters(&self) -> &ParamSet {
        &self.parameters
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.envelopes = vec![1.0; format.channels_usize().max(1)];
    }

    fn reset(&mut self) {
        self.envelopes.fill(1.0);
    }

    fn process(&mut self, block: &mut AudioBlock<'_>) {
        let channels = block.channels();
        if channels == 0 || channels != self.envelopes.len() {
            return;
        }

        let ceiling = self.ceiling();
        let makeup = db_to_linear(self.parameters.value_f32(PARAM_MAKEUP));
        let linked = self.parameters.value_bool(PARAM_LINKED);
        let release = one_pole_coefficient(
            self.parameters.value_f32(PARAM_RELEASE),
            block.format().sample_rate_f32(),
        );

        for frame in block.frames_iter_mut() {
            // With a linked detector every channel is reduced by the same
            // amount, which keeps the stereo image intact instead of pulling
            // one side towards the centre when only that side is loud.
            let common_level = if linked {
                frame.iter().fold(0.0_f32, |acc, s| acc.max(s.abs())) * makeup
            } else {
                0.0
            };

            for (channel, sample) in frame.iter_mut().enumerate() {
                let level = if linked {
                    common_level
                } else {
                    sample.abs() * makeup
                };

                let target = if level > ceiling {
                    ceiling / level
                } else {
                    1.0
                };
                let envelope = &mut self.envelopes[channel];

                if target < *envelope {
                    // Instantaneous attack is what makes this brick-wall.
                    *envelope = target;
                } else {
                    *envelope += (target - *envelope) * release;
                }

                *sample *= makeup * *envelope;
            }
        }
    }

    fn summary(&self) -> String {
        format!(
            "Ceiling {} · Make-up {}",
            self.parameters.format(PARAM_THRESHOLD),
            self.parameters.format(PARAM_MAKEUP)
        )
    }
}
