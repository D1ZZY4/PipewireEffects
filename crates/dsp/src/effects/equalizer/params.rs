use pwe_core::{ParamSet, ParamSpec, ParamUnit};

use crate::biquad::{Coefficients, FilterKind};
use crate::knob::Knob;

pub use super::labels::{
    BAND_COUNT, BAND_FREQUENCIES, BAND_FREQUENCY_IDS, BAND_FREQUENCY_LABELS, BAND_IDS, BAND_LABELS,
    BAND_Q_IDS, BAND_Q_LABELS, BAND_TYPE_IDS, BAND_TYPE_LABELS, CHANNEL_LABELS, CHANNEL_MODE_ID,
    DEFAULT_Q, PEAK_INDEX, RIGHT_BAND_FREQUENCY_IDS, RIGHT_BAND_IDS, RIGHT_BAND_Q_IDS,
    RIGHT_BAND_TYPE_IDS, band_group, right_group,
};

#[allow(clippy::too_many_arguments)]
pub fn push_band(
    specs: &mut Vec<ParamSpec>,
    gain_id: &'static str,
    gain_label: &'static str,
    type_id: &'static str,
    type_label: &'static str,
    frequency_id: &'static str,
    frequency_label: &'static str,
    q_id: &'static str,
    q_label: &'static str,
    frequency: f32,
    group: &'static str,
) {
    specs.push(
        ParamSpec::float(gain_id, gain_label, ParamUnit::Decibel)
            .range(-24.0, 24.0, 0.0)
            .step(0.1)
            .group(group)
            .build(),
    );
    specs.push(
        ParamSpec::choice(type_id, type_label, &FilterKind::LABELS, PEAK_INDEX as u16).group(group),
    );
    specs.push(
        ParamSpec::float(frequency_id, frequency_label, ParamUnit::Hertz)
            .range(20.0, 20_000.0, frequency)
            .logarithmic()
            .group(group)
            .build(),
    );
    specs.push(
        ParamSpec::float(q_id, q_label, ParamUnit::None)
            .range(0.1, 10.0, DEFAULT_Q)
            .step(0.01)
            .group(group)
            .build(),
    );
}

pub fn specs() -> Vec<ParamSpec> {
    let mut specs = Vec::with_capacity(BAND_COUNT * 8 + 4);
    specs.push(ParamSpec::choice(CHANNEL_MODE_ID, "Channels", &CHANNEL_LABELS, 0).group("Channel"));
    for band in 0..BAND_COUNT {
        push_band(
            &mut specs,
            BAND_IDS[band],
            BAND_LABELS[band],
            BAND_TYPE_IDS[band],
            BAND_TYPE_LABELS[band],
            BAND_FREQUENCY_IDS[band],
            BAND_FREQUENCY_LABELS[band],
            BAND_Q_IDS[band],
            BAND_Q_LABELS[band],
            BAND_FREQUENCIES[band],
            band_group(BAND_FREQUENCIES[band]),
        );
    }
    for band in 0..BAND_COUNT {
        push_band(
            &mut specs,
            RIGHT_BAND_IDS[band],
            BAND_LABELS[band],
            RIGHT_BAND_TYPE_IDS[band],
            BAND_TYPE_LABELS[band],
            RIGHT_BAND_FREQUENCY_IDS[band],
            BAND_FREQUENCY_LABELS[band],
            RIGHT_BAND_Q_IDS[band],
            BAND_Q_LABELS[band],
            BAND_FREQUENCIES[band],
            right_group(BAND_FREQUENCIES[band]),
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
        ParamSpec::float("output_gain", "Output gain", ParamUnit::Decibel)
            .range(-24.0, 24.0, 0.0)
            .step(0.1)
            .group("Tone")
            .build(),
    );
    specs
}

pub fn effective_q(q: f32, bandwidth: f32) -> f32 {
    if (q - DEFAULT_Q).abs() < 1.0e-6 {
        bandwidth.clamp(0.1, 10.0)
    } else {
        q
    }
}

pub struct BandKnobs {
    pub gain: Knob,
    pub kind: Knob,
    pub frequency: Knob,
    pub q: Knob,
}

impl BandKnobs {
    pub fn new(
        params: &ParamSet,
        gain_id: &'static str,
        type_id: &'static str,
        frequency_id: &'static str,
        q_id: &'static str,
    ) -> Self {
        Self {
            gain: Knob::new(params, gain_id, 0.0),
            kind: Knob::new(params, type_id, PEAK_INDEX as f32),
            frequency: Knob::new(params, frequency_id, 1_000.0),
            q: Knob::new(params, q_id, DEFAULT_Q),
        }
    }

    pub fn coefficients(&self, rate: f32, bandwidth: f32) -> Coefficients {
        Coefficients::design(
            FilterKind::from_index(self.kind.index()),
            rate,
            self.frequency.value(),
            effective_q(self.q.value(), bandwidth),
            self.gain.value(),
        )
    }
}

pub struct BandSetting {
    pub kind: FilterKind,
    pub frequency: f32,
    pub q: f32,
    pub gain_db: f32,
}

fn settings_for(
    params: &ParamSet,
    gain_ids: &[&str; BAND_COUNT],
    type_ids: &[&str; BAND_COUNT],
    frequency_ids: &[&str; BAND_COUNT],
    q_ids: &[&str; BAND_COUNT],
) -> [BandSetting; BAND_COUNT] {
    let bandwidth = params.get_f32("bandwidth").unwrap_or(DEFAULT_Q);
    std::array::from_fn(|band| BandSetting {
        kind: FilterKind::from_index(
            params.get_f32(type_ids[band]).unwrap_or(PEAK_INDEX as f32) as usize
        ),
        frequency: params
            .get_f32(frequency_ids[band])
            .unwrap_or(BAND_FREQUENCIES[band]),
        q: effective_q(params.get_f32(q_ids[band]).unwrap_or(DEFAULT_Q), bandwidth),
        gain_db: params.get_f32(gain_ids[band]).unwrap_or(0.0),
    })
}

pub fn main_settings(params: &ParamSet) -> [BandSetting; BAND_COUNT] {
    settings_for(
        params,
        &BAND_IDS,
        &BAND_TYPE_IDS,
        &BAND_FREQUENCY_IDS,
        &BAND_Q_IDS,
    )
}

pub fn right_settings(params: &ParamSet) -> [BandSetting; BAND_COUNT] {
    settings_for(
        params,
        &RIGHT_BAND_IDS,
        &RIGHT_BAND_TYPE_IDS,
        &RIGHT_BAND_FREQUENCY_IDS,
        &RIGHT_BAND_Q_IDS,
    )
}

pub fn channel_split(params: &ParamSet) -> bool {
    params.get_f32(CHANNEL_MODE_ID).unwrap_or(0.0) >= 0.5
}

pub fn response_db(rate: f32, params: &ParamSet, right: bool, frequency: f32) -> f32 {
    let settings = if right && channel_split(params) {
        right_settings(params)
    } else {
        main_settings(params)
    };
    response_of(rate, &settings, frequency)
}

pub fn response_of(rate: f32, settings: &[BandSetting; BAND_COUNT], frequency: f32) -> f32 {
    let mut total = 0.0f32;
    for band in settings {
        let coefficients =
            Coefficients::design(band.kind, rate, band.frequency, band.q, band.gain_db);
        let magnitude = coefficients.magnitude_at(rate, frequency).max(1.0e-9);
        total += 20.0 * magnitude.log10();
    }
    total
}
