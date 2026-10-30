use pwe_core::i18n::mark;
use pwe_core::{ParamSet, ParamSpec, ParamUnit};

use crate::effects::crossover::{Crossover, MAX_BANDS};
use crate::knob::Knob;

pub const DEFAULT_SPLITS: [f32; MAX_BANDS - 1] = [120.0, 800.0, 5_000.0];

const BAND_GROUPS: [&str; MAX_BANDS] = [
    mark("Band 1"),
    mark("Band 2"),
    mark("Band 3"),
    mark("Band 4"),
];

pub fn band_group(band: usize) -> &'static str {
    BAND_GROUPS[band]
}

pub fn crossover_specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("bands", "Bands", ParamUnit::None)
            .range(2.0, MAX_BANDS as f32, MAX_BANDS as f32)
            .step(1.0)
            .group("Crossovers")
            .build(),
        ParamSpec::float("split1", "Split 1", ParamUnit::Hertz)
            .range(40.0, 1_000.0, DEFAULT_SPLITS[0])
            .logarithmic()
            .group("Crossovers")
            .build(),
        ParamSpec::float("split2", "Split 2", ParamUnit::Hertz)
            .range(200.0, 8_000.0, DEFAULT_SPLITS[1])
            .logarithmic()
            .group("Crossovers")
            .build(),
        ParamSpec::float("split3", "Split 3", ParamUnit::Hertz)
            .range(1_000.0, 16_000.0, DEFAULT_SPLITS[2])
            .logarithmic()
            .group("Crossovers")
            .build(),
    ]
}

pub fn mix_spec() -> ParamSpec {
    ParamSpec::float("mix", "Mix", ParamUnit::Percent)
        .range(0.0, 100.0, 100.0)
        .step(1.0)
        .group("Output")
        .build()
}

pub fn band_knobs(
    params: &ParamSet,
    ids: &[[&'static str; 7]; MAX_BANDS],
    defaults: &[f32; 7],
) -> [[Knob; 7]; MAX_BANDS] {
    std::array::from_fn(|band| {
        std::array::from_fn(|param| Knob::new(params, ids[band][param], defaults[param]))
    })
}

pub fn band_count(control: &Knob) -> usize {
    (control.value().round() as usize).clamp(2, MAX_BANDS)
}

pub fn splits(split1: &Knob, split2: &Knob, split3: &Knob) -> [f32; MAX_BANDS - 1] {
    [split1.value(), split2.value(), split3.value()]
}

pub fn redesign(
    crossover: &mut Crossover,
    signature: &mut [f32; 4],
    dirty: &mut bool,
    count: usize,
    splits: [f32; MAX_BANDS - 1],
    rate: f32,
) {
    let current = [count as f32, splits[0], splits[1], splits[2]];
    if *dirty || current != *signature {
        *signature = current;
        *dirty = false;
        crossover.design(count, rate, &splits);
    }
}
