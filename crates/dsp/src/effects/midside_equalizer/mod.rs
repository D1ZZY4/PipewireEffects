use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, math};

use crate::biquad::Biquad;
use crate::knob::Knob;

use super::equalizer::params::{BAND_COUNT, BandKnobs, DEFAULT_Q};

mod params;

pub use params::specs;

use params::{
    MID_FREQUENCY_IDS, MID_GAIN_IDS, MID_Q_IDS, MID_TYPE_IDS, SIDE_FREQUENCY_IDS, SIDE_GAIN_IDS,
    SIDE_Q_IDS, SIDE_TYPE_IDS,
};

const SIGNATURE_LEN: usize = BAND_COUNT * 8 + 1;

pub struct MidSideEqualizer {
    mid: [BandKnobs; BAND_COUNT],
    side: [BandKnobs; BAND_COUNT],
    bandwidth: Knob,
    mid_gain: Knob,
    side_gain: Knob,
    output_gain: Knob,
    mid_bands: [Biquad; BAND_COUNT],
    side_bands: [Biquad; BAND_COUNT],
    signature: [f32; SIGNATURE_LEN],
    rate: f32,
    dirty: bool,
}

impl MidSideEqualizer {
    pub fn new(params: ParamSet) -> Self {
        let mid = std::array::from_fn(|band| {
            BandKnobs::new(
                &params,
                MID_GAIN_IDS[band],
                MID_TYPE_IDS[band],
                MID_FREQUENCY_IDS[band],
                MID_Q_IDS[band],
            )
        });
        let side = std::array::from_fn(|band| {
            BandKnobs::new(
                &params,
                SIDE_GAIN_IDS[band],
                SIDE_TYPE_IDS[band],
                SIDE_FREQUENCY_IDS[band],
                SIDE_Q_IDS[band],
            )
        });
        Self {
            mid,
            side,
            bandwidth: Knob::new(&params, "bandwidth", DEFAULT_Q),
            mid_gain: Knob::new(&params, "mid_gain", 0.0),
            side_gain: Knob::new(&params, "side_gain", 0.0),
            output_gain: Knob::new(&params, "output_gain", 0.0),
            mid_bands: [Biquad::default(); BAND_COUNT],
            side_bands: [Biquad::default(); BAND_COUNT],
            signature: [f32::NAN; SIGNATURE_LEN],
            rate: 48_000.0,
            dirty: true,
        }
    }

    fn signature_now(&self) -> [f32; SIGNATURE_LEN] {
        let mut signature = [0.0f32; SIGNATURE_LEN];
        for band in 0..BAND_COUNT {
            let slot = band * 4;
            signature[slot] = self.mid[band].gain.value();
            signature[slot + 1] = self.mid[band].kind.value();
            signature[slot + 2] = self.mid[band].frequency.value();
            signature[slot + 3] = self.mid[band].q.value();
            let side = BAND_COUNT * 4 + slot;
            signature[side] = self.side[band].gain.value();
            signature[side + 1] = self.side[band].kind.value();
            signature[side + 2] = self.side[band].frequency.value();
            signature[side + 3] = self.side[band].q.value();
        }
        signature[BAND_COUNT * 8] = self.bandwidth.value();
        signature
    }

    fn refresh(&mut self, bandwidth: f32) {
        for band in 0..BAND_COUNT {
            let mid = self.mid[band].coefficients(self.rate, bandwidth);
            self.mid_bands[band].set_coefficients(mid);
            let side = self.side[band].coefficients(self.rate, bandwidth);
            self.side_bands[band].set_coefficients(side);
        }
    }
}

impl Effect for MidSideEqualizer {
    fn id(&self) -> &'static str {
        "midside_equalizer"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate().max(1) as f32;
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        for stage in &mut self.mid_bands {
            stage.reset();
        }
        for stage in &mut self.side_bands {
            stage.reset();
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let signature = self.signature_now();
        if self.dirty || signature != self.signature {
            self.signature = signature;
            self.dirty = false;
            self.refresh(signature[BAND_COUNT * 8]);
        }
        if buffer.channels() < 2 {
            return;
        }
        let mid_trim = math::db_to_gain(self.mid_gain.value());
        let side_trim = math::db_to_gain(self.side_gain.value());
        let output = math::db_to_gain(self.output_gain.value());
        let frames = buffer.frames();

        for frame in 0..frames {
            let left = buffer.channel(0)[frame];
            let right = buffer.channel(1)[frame];
            let mut middle = 0.5 * (left + right);
            let mut side = 0.5 * (left - right);
            for stage in &mut self.mid_bands {
                middle = stage.process(middle);
            }
            for stage in &mut self.side_bands {
                side = stage.process(side);
            }
            middle *= mid_trim;
            side *= side_trim;
            buffer.channel_mut(0)[frame] = math::sanitize((middle + side) * output);
            buffer.channel_mut(1)[frame] = math::sanitize((middle - side) * output);
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
