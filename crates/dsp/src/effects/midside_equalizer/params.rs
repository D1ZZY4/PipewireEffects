use pwe_core::i18n::mark;
use pwe_core::{ParamSpec, ParamUnit};

use crate::effects::equalizer::params::{
    BAND_COUNT, BAND_FREQUENCIES, BAND_FREQUENCY_LABELS, BAND_LABELS, BAND_Q_LABELS,
    BAND_TYPE_LABELS, DEFAULT_Q, push_band,
};

macro_rules! midside_ids {
    ($prefix:literal, $suffix:literal; $($number:literal),* $(,)?) => {
        [$(concat!($prefix, $number, $suffix)),*]
    };
}

pub const MID_GAIN_IDS: [&str; BAND_COUNT] = midside_ids!("mid_band", "_gain"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const MID_TYPE_IDS: [&str; BAND_COUNT] = midside_ids!("mid_band", "_type"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const MID_FREQUENCY_IDS: [&str; BAND_COUNT] = midside_ids!("mid_band", "_frequency"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const MID_Q_IDS: [&str; BAND_COUNT] = midside_ids!("mid_band", "_q"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const SIDE_GAIN_IDS: [&str; BAND_COUNT] = midside_ids!("side_band", "_gain"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const SIDE_TYPE_IDS: [&str; BAND_COUNT] = midside_ids!("side_band", "_type"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const SIDE_FREQUENCY_IDS: [&str; BAND_COUNT] = midside_ids!("side_band", "_frequency"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);
pub const SIDE_Q_IDS: [&str; BAND_COUNT] = midside_ids!("side_band", "_q"; 11, 12, 1, 13, 14, 2, 15, 16, 3, 17, 18, 4, 19, 20, 5, 21, 22, 6, 23, 24, 7, 25, 26, 8, 27, 28, 9, 29, 30, 10);

pub const MID_GROUP: &str = mark("Mid bands");
pub const SIDE_GROUP: &str = mark("Side bands");

pub fn specs() -> Vec<ParamSpec> {
    let mut specs = Vec::with_capacity(BAND_COUNT * 8 + 4);
    for band in 0..BAND_COUNT {
        push_band(
            &mut specs,
            MID_GAIN_IDS[band],
            BAND_LABELS[band],
            MID_TYPE_IDS[band],
            BAND_TYPE_LABELS[band],
            MID_FREQUENCY_IDS[band],
            BAND_FREQUENCY_LABELS[band],
            MID_Q_IDS[band],
            BAND_Q_LABELS[band],
            BAND_FREQUENCIES[band],
            MID_GROUP,
        );
    }
    for band in 0..BAND_COUNT {
        push_band(
            &mut specs,
            SIDE_GAIN_IDS[band],
            BAND_LABELS[band],
            SIDE_TYPE_IDS[band],
            BAND_TYPE_LABELS[band],
            SIDE_FREQUENCY_IDS[band],
            BAND_FREQUENCY_LABELS[band],
            SIDE_Q_IDS[band],
            BAND_Q_LABELS[band],
            BAND_FREQUENCIES[band],
            SIDE_GROUP,
        );
    }
    specs.push(
        ParamSpec::float("bandwidth", "Default bandwidth", ParamUnit::None)
            .range(0.3, 6.0, DEFAULT_Q)
            .step(0.01)
            .group("Tone")
            .build(),
    );
    specs.push(
        ParamSpec::float("mid_gain", "Mid gain", ParamUnit::Decibel)
            .range(-24.0, 24.0, 0.0)
            .step(0.1)
            .group("Tone")
            .build(),
    );
    specs.push(
        ParamSpec::float("side_gain", "Side gain", ParamUnit::Decibel)
            .range(-24.0, 24.0, 0.0)
            .step(0.1)
            .group("Tone")
            .build(),
    );
    specs.push(
        ParamSpec::float("output_gain", "Output gain", ParamUnit::Decibel)
            .range(-24.0, 24.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
    );
    specs
}
