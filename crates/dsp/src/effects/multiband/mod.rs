use pwe_core::i18n::mark;
use pwe_core::{ParamSet, ParamSpec, ParamUnit};

use crate::effects::crossover::{Crossover, DEFAULT_BANDS, MAX_BANDS};
use crate::knob::Knob;

pub const DEFAULT_SPLITS: [f32; MAX_BANDS - 1] =
    [120.0, 800.0, 5_000.0, 8_000.0, 11_000.0, 14_000.0, 17_000.0];

const SPLIT_RANGES: [(f32, f32); MAX_BANDS - 1] = [
    (40.0, 1_000.0),
    (200.0, 8_000.0),
    (1_000.0, 16_000.0),
    (2_000.0, 18_000.0),
    (3_000.0, 20_000.0),
    (4_000.0, 20_000.0),
    (5_000.0, 20_000.0),
];

const BAND_GROUPS: [&str; MAX_BANDS] = [
    mark("Band 1"),
    mark("Band 2"),
    mark("Band 3"),
    mark("Band 4"),
    mark("Band 5"),
    mark("Band 6"),
    mark("Band 7"),
    mark("Band 8"),
];

pub fn band_group(band: usize) -> &'static str {
    BAND_GROUPS[band]
}

pub fn crossover_specs() -> Vec<ParamSpec> {
    let mut specs = Vec::with_capacity(1 + MAX_BANDS);
    specs.push(
        ParamSpec::float("bands", "Bands", ParamUnit::None)
            .range(2.0, MAX_BANDS as f32, DEFAULT_BANDS as f32)
            .step(1.0)
            .group("Crossovers")
            .build(),
    );
    for (index, (min, max)) in SPLIT_RANGES.iter().enumerate() {
        specs.push(
            ParamSpec::float(SPLIT_IDS[index], SPLIT_LABELS[index], ParamUnit::Hertz)
                .range(*min, *max, DEFAULT_SPLITS[index])
                .logarithmic()
                .group("Crossovers")
                .build(),
        );
    }
    specs
}

const SPLIT_IDS: [&str; MAX_BANDS - 1] = [
    "split1", "split2", "split3", "split4", "split5", "split6", "split7",
];

const SPLIT_LABELS: [&str; MAX_BANDS - 1] = [
    mark("Split 1"),
    mark("Split 2"),
    mark("Split 3"),
    mark("Split 4"),
    mark("Split 5"),
    mark("Split 6"),
    mark("Split 7"),
];

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

pub fn split_knobs(params: &ParamSet) -> [Knob; MAX_BANDS - 1] {
    std::array::from_fn(|index| Knob::new(params, SPLIT_IDS[index], DEFAULT_SPLITS[index]))
}

pub fn splits(knobs: &[Knob; MAX_BANDS - 1]) -> [f32; MAX_BANDS - 1] {
    std::array::from_fn(|index| knobs[index].value())
}

pub fn redesign(
    crossover: &mut Crossover,
    signature: &mut [f32; MAX_BANDS],
    dirty: &mut bool,
    count: usize,
    splits: [f32; MAX_BANDS - 1],
    rate: f32,
) {
    let mut current = [0.0f32; MAX_BANDS];
    current[0] = count as f32;
    current[1..].copy_from_slice(&splits);
    if *dirty || current != *signature {
        *signature = current;
        *dirty = false;
        crossover.design(count, rate, &splits);
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
