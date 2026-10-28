//! Eight-band parametric equaliser.
//!
//! Each band is one biquad section whose type, centre frequency, gain and Q are
//! all parameters. Bands are cascaded in order, so the response is the sum of
//! the individual responses in decibels — which is exactly how a graphic
//! equaliser curve is drawn, and how a user expects an EQ to behave.
//!
//! Two things are worth knowing about the implementation:
//!
//! * **Coefficients are recomputed once per block**, not per sample. Designing
//!   a biquad involves a `sin`, a `cos` and a `powf`, which is far too much
//!   work to do 48,000 times a second per band. The filter *state* carries
//!   across blocks, so a per-block coefficient update is a smooth-sounding
//!   approximation rather than a discontinuity.
//! * **Bands that cannot do anything are skipped.** A peaking band at 0 dB is
//!   the identity, and processing it anyway would only add noise and cost.
//!
//! The default layout is eight transparent bands: a low shelf, six peaks spread
//! across the spectrum, and a high shelf. Adding the effect therefore changes
//! nothing until a slider moves.

use pipewire_effects_core::{
    AudioBlock, AudioEffect, AudioFormat, EffectCategory, EffectDef, EffectKind, ParamSet,
    ParamSpec, ParamUnit,
};

use crate::biquad::{BiquadCoefficients, BiquadState};
use crate::util::db_to_linear;

/// The number of bands every instance has.
pub const BAND_COUNT: usize = 8;

/// Index of the global pre-amplifier parameter.
const PARAM_PREAMP: usize = 0;

/// Parameters per band: enabled, type, frequency, gain, Q.
const PARAMS_PER_BAND: usize = 5;

/// The index of a band's first parameter.
const fn band_base(band: usize) -> usize {
    PARAM_PREAMP + 1 + band * PARAMS_PER_BAND
}

/// The filter shapes a band can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BandType {
    /// A bell: `gain` at the centre frequency.
    Peak,
    /// `gain` below the corner.
    LowShelf,
    /// `gain` above the corner.
    HighShelf,
    /// Attenuates below the corner.
    HighPass,
    /// Attenuates above the corner.
    LowPass,
    /// Rejects a narrow band.
    Notch,
    /// Passes a narrow band at unity gain.
    BandPass,
    /// Flat magnitude, frequency-dependent phase.
    AllPass,
}

impl BandType {
    /// The order must match the parameter's option list below.
    const OPTIONS: [BandType; 8] = [
        BandType::Peak,
        BandType::LowShelf,
        BandType::HighShelf,
        BandType::HighPass,
        BandType::LowPass,
        BandType::Notch,
        BandType::BandPass,
        BandType::AllPass,
    ];

    /// The labels shown in the interface, in the same order as [`BandType::OPTIONS`].
    const LABELS: [&'static str; 8] = [
        "Peak",
        "Low Shelf",
        "High Shelf",
        "High Pass",
        "Low Pass",
        "Notch",
        "Band Pass",
        "All Pass",
    ];

    fn from_index(index: usize) -> Self {
        Self::OPTIONS.get(index).copied().unwrap_or(BandType::Peak)
    }

    /// Whether the band's gain parameter has any effect.
    const fn uses_gain(self) -> bool {
        matches!(self, Self::Peak | Self::LowShelf | Self::HighShelf)
    }

    fn design(self, frequency: f32, sample_rate: f32, q: f32, gain_db: f32) -> BiquadCoefficients {
        match self {
            Self::Peak => BiquadCoefficients::peaking(frequency, sample_rate, q, gain_db),
            Self::LowShelf => {
                BiquadCoefficients::low_shelf(frequency, sample_rate, shelf_slope(q), gain_db)
            }
            Self::HighShelf => {
                BiquadCoefficients::high_shelf(frequency, sample_rate, shelf_slope(q), gain_db)
            }
            Self::HighPass => BiquadCoefficients::high_pass(frequency, sample_rate, q),
            Self::LowPass => BiquadCoefficients::low_pass(frequency, sample_rate, q),
            Self::Notch => BiquadCoefficients::notch(frequency, sample_rate, q),
            Self::BandPass => BiquadCoefficients::band_pass(frequency, sample_rate, q),
            Self::AllPass => BiquadCoefficients::all_pass(frequency, sample_rate, q),
        }
    }
}

/// Maps a quality factor onto the shelf slope parameter.
///
/// Shelves are designed in terms of a slope, not a Q, and that slope is only
/// defined up to `1.0`, where the shelf is as steep as a second-order section
/// can be. A band's shared `q` parameter is therefore compressed into
/// `0.1..=1.0` rather than passed straight through. Q still has an effect: it
/// just saturates once the shelf is at maximum steepness.
fn shelf_slope(q: f32) -> f32 {
    (q * 0.75).clamp(0.1, 1.0)
}

/// The centre frequency each band starts at, spread across the spectrum.
const DEFAULT_FREQUENCIES: [f32; BAND_COUNT] = [
    60.0, 150.0, 400.0, 1_000.0, 2_500.0, 5_000.0, 8_000.0, 12_000.0,
];

/// The shape each band starts with.
const DEFAULT_TYPES: [BandType; BAND_COUNT] = [
    BandType::LowShelf,
    BandType::Peak,
    BandType::Peak,
    BandType::Peak,
    BandType::Peak,
    BandType::Peak,
    BandType::Peak,
    BandType::HighShelf,
];

/// The effect type.
pub const KIND: EffectKind = EffectKind::new("parametric-eq");

/// Registry entry.
pub const DEF: EffectDef = EffectDef::new(
    KIND,
    "Parametric Equalizer",
    "Eight bands with per-band shape, frequency, gain and Q.",
    EffectCategory::Equalizer,
    ParametricEqEffect::boxed,
);

/// An eight-band parametric equaliser.
#[derive(Debug)]
pub struct ParametricEqEffect {
    parameters: ParamSet,
    /// One coefficient set per band, refreshed each block.
    coefficients: [BiquadCoefficients; BAND_COUNT],
    /// `BAND_COUNT * channels` sections, in band-major order.
    states: Vec<BiquadState>,
    channels: usize,
}

impl ParametricEqEffect {
    /// Creates a transparent eight-band equaliser.
    #[must_use]
    pub fn new() -> Self {
        let mut specs = Vec::with_capacity(PARAM_PREAMP + 1 + BAND_COUNT * PARAMS_PER_BAND);
        specs.push(
            ParamSpec::float(
                "preamp",
                "Pre-amplifier",
                -24.0,
                24.0,
                0.0,
                0.1,
                ParamUnit::Decibel,
            )
            .with_doc("Applied before the bands, to compensate for boosts."),
        );

        for band in 0..BAND_COUNT {
            let number = band + 1;
            let prefix = format!("band{number}_");
            specs.push(
                ParamSpec::boolean(format!("{prefix}enabled"), format!("Band {number}"), true)
                    .with_doc("Whether this band is applied."),
            );
            specs.push(
                ParamSpec::enumerate(
                    format!("{prefix}type"),
                    "Shape",
                    &BandType::LABELS,
                    DEFAULT_TYPES[band] as usize,
                )
                .with_doc("The filter shape this band applies."),
            );
            specs.push(
                ParamSpec::float_log(
                    format!("{prefix}frequency"),
                    "Frequency",
                    20.0,
                    20_000.0,
                    DEFAULT_FREQUENCIES[band],
                    ParamUnit::Hertz,
                )
                .with_doc("The band's centre or corner frequency."),
            );
            specs.push(
                ParamSpec::float(
                    format!("{prefix}gain"),
                    "Gain",
                    -24.0,
                    24.0,
                    0.0,
                    0.1,
                    ParamUnit::Decibel,
                )
                .with_doc("Boost or cut. Ignored by bands that have no gain."),
            );
            specs.push(
                ParamSpec::float_log(
                    format!("{prefix}q"),
                    "Q",
                    0.1,
                    10.0,
                    0.707,
                    ParamUnit::Quality,
                )
                .with_doc("Bandwidth. Higher values are narrower. Shelves use it as a slope."),
            );
        }

        let mut effect = Self {
            parameters: ParamSet::new(specs),
            coefficients: [BiquadCoefficients::IDENTITY; BAND_COUNT],
            states: Vec::new(),
            channels: 0,
        };
        effect.prepare(AudioFormat::default());
        effect
    }

    /// Registry factory.
    #[must_use]
    pub fn boxed() -> Box<dyn AudioEffect> {
        Box::new(Self::new())
    }

    /// Reads one band's settings.
    fn band_settings(&self, band: usize) -> (bool, BandType, f32, f32, f32) {
        let base = band_base(band);
        (
            self.parameters.value_bool(base),
            BandType::from_index(self.parameters.value_enum(base + 1)),
            self.parameters.value_f32(base + 2),
            self.parameters.value_f32(base + 3),
            self.parameters.value_f32(base + 4),
        )
    }

    /// Whether any band would currently change the signal.
    #[must_use]
    pub fn is_transparent(&self) -> bool {
        if self.parameters.value_f32(PARAM_PREAMP).abs() > f32::EPSILON {
            return false;
        }
        (0..BAND_COUNT).all(|band| {
            let (enabled, shape, _, gain_db, _) = self.band_settings(band);
            !enabled || (shape.uses_gain() && gain_db.abs() < 1e-3)
        })
    }
}

impl Default for ParametricEqEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEffect for ParametricEqEffect {
    fn kind(&self) -> EffectKind {
        KIND
    }

    fn parameters(&self) -> &ParamSet {
        &self.parameters
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.channels = format.channels_usize().max(1);
        self.states = vec![BiquadState::new(); BAND_COUNT * self.channels];
        self.coefficients = [BiquadCoefficients::IDENTITY; BAND_COUNT];
    }

    fn reset(&mut self) {
        for state in &mut self.states {
            state.reset();
        }
    }

    fn process(&mut self, block: &mut AudioBlock<'_>) {
        let channels = block.channels();
        if channels == 0 || channels != self.channels || self.states.len() < BAND_COUNT * channels {
            // The engine always calls `prepare` before processing; if it has
            // not, processing would write out of bounds, so pass audio through
            // instead.
            return;
        }
        let sample_rate = block.format().sample_rate_f32();

        let preamp_db = self.parameters.value_f32(PARAM_PREAMP);
        if preamp_db.abs() > 1e-3 {
            block.apply_gain(db_to_linear(preamp_db));
        }

        for band in 0..BAND_COUNT {
            let (enabled, shape, frequency, gain_db, q) = self.band_settings(band);
            if !enabled {
                continue;
            }
            if shape.uses_gain() && gain_db.abs() < 1e-3 {
                // A shelving or peaking band at 0 dB is exactly the identity.
                // Skipping it keeps the response bit-accurate and saves work.
                continue;
            }

            let coefficients = shape.design(frequency, sample_rate, q, gain_db);
            self.coefficients[band] = coefficients;

            let offset = band * channels;
            for frame in block.frames_iter_mut() {
                for (channel, sample) in frame.iter_mut().enumerate() {
                    *sample = self.states[offset + channel].process(&coefficients, *sample);
                }
            }

            for channel in 0..channels {
                self.states[offset + channel].sanitise();
            }
        }
    }

    fn summary(&self) -> String {
        let mut parts = Vec::new();
        let preamp = self.parameters.value_f32(PARAM_PREAMP);
        if preamp.abs() > 1e-3 {
            parts.push(self.parameters.format(PARAM_PREAMP));
        }
        for band in 0..BAND_COUNT {
            let (enabled, shape, _, gain_db, _) = self.band_settings(band);
            if enabled && shape.uses_gain() && gain_db.abs() >= 1e-3 {
                let base = band_base(band);
                parts.push(format!(
                    "{:.0} Hz {}",
                    self.parameters.value_f32(base + 2),
                    self.parameters.format(base + 3)
                ));
            }
        }
        if parts.is_empty() {
            String::from("Flat")
        } else {
            parts.join(" · ")
        }
    }
}
