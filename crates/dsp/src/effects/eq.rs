use pipewire_effects_core::{
    AudioBlock, AudioEffect, AudioFormat, EffectCategory, EffectDef, EffectKind, ParamSet,
    ParamSpec, ParamUnit,
};

use crate::biquad::{BiquadCoefficients, BiquadState};
use crate::util::db_to_linear;

pub const BAND_COUNT: usize = 8;

const PARAM_PREAMP: usize = 0;

const PARAMS_PER_BAND: usize = 5;

const fn band_base(band: usize) -> usize {
    PARAM_PREAMP + 1 + band * PARAMS_PER_BAND
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BandType {
    Peak,
    LowShelf,
    HighShelf,
    HighPass,
    LowPass,
    Notch,
    BandPass,
    AllPass,
}

impl BandType {
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

fn shelf_slope(q: f32) -> f32 {
    (q * 0.75).clamp(0.1, 1.0)
}

const DEFAULT_FREQUENCIES: [f32; BAND_COUNT] = [
    60.0, 150.0, 400.0, 1_000.0, 2_500.0, 5_000.0, 8_000.0, 12_000.0,
];

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

pub const KIND: EffectKind = EffectKind::new("parametric-eq");

pub const DEF: EffectDef = EffectDef::new(
    KIND,
    "Parametric Equalizer",
    "Eight bands with per-band shape, frequency, gain and Q.",
    EffectCategory::Equalizer,
    ParametricEqEffect::boxed,
)
.with_response(response);

#[derive(Debug)]
pub struct ParametricEqEffect {
    parameters: ParamSet,
    coefficients: [BiquadCoefficients; BAND_COUNT],
    states: Vec<BiquadState>,
    channels: usize,
}

impl ParametricEqEffect {
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

    #[must_use]
    pub fn boxed() -> Box<dyn AudioEffect> {
        Box::new(Self::new())
    }

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

#[must_use]
pub fn response(parameters: &ParamSet, frequency: f32, sample_rate: f32) -> Option<f32> {
    let mut total = parameters.value_f32(PARAM_PREAMP);

    for band in 0..BAND_COUNT {
        let base = band_base(band);
        if !parameters.value_bool(base) {
            continue;
        }
        let shape = BandType::from_index(parameters.value_enum(base + 1));
        let gain_db = parameters.value_f32(base + 3);
        if shape.uses_gain() && gain_db.abs() < 1e-3 {
            continue;
        }
        let coefficients = shape.design(
            parameters.value_f32(base + 2),
            sample_rate,
            parameters.value_f32(base + 4),
            gain_db,
        );
        total += coefficients
            .gain_db_at(frequency, sample_rate)
            .clamp(-48.0, 48.0);
    }

    Some(total)
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
