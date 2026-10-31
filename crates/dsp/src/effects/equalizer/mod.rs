use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, math};

use crate::biquad::Biquad;
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

mod labels;
pub(crate) mod params;

pub use params::response_db;
pub use params::{BAND_COUNT, BAND_FREQUENCIES, BAND_IDS, BAND_LABELS, specs};

use params::{BandKnobs, CHANNEL_MODE_ID, DEFAULT_Q, RIGHT_BAND_IDS};

const SIGNATURE_LEN: usize = BAND_COUNT * 8 + 1;

pub struct Equalizer {
    bands: [[Biquad; BAND_COUNT]; MAX_CHANNELS],
    main: [BandKnobs; BAND_COUNT],
    right: [BandKnobs; BAND_COUNT],
    channel_mode: Knob,
    bandwidth: Knob,
    output_gain: Knob,
    signature: [f32; SIGNATURE_LEN],
    output_smoothed: math::Smoothed,
    rate: f32,
    dirty: bool,
}

impl Equalizer {
    pub fn new(params: ParamSet) -> Self {
        let main = std::array::from_fn(|band| {
            BandKnobs::new(
                &params,
                BAND_IDS[band],
                params::BAND_TYPE_IDS[band],
                params::BAND_FREQUENCY_IDS[band],
                params::BAND_Q_IDS[band],
            )
        });
        let right = std::array::from_fn(|band| {
            BandKnobs::new(
                &params,
                RIGHT_BAND_IDS[band],
                params::RIGHT_BAND_TYPE_IDS[band],
                params::RIGHT_BAND_FREQUENCY_IDS[band],
                params::RIGHT_BAND_Q_IDS[band],
            )
        });
        Self {
            bands: [[Biquad::default(); BAND_COUNT]; MAX_CHANNELS],
            main,
            right,
            channel_mode: Knob::new(&params, CHANNEL_MODE_ID, 0.0),
            bandwidth: Knob::new(&params, "bandwidth", DEFAULT_Q),
            output_gain: Knob::new(&params, "output_gain", 0.0),
            signature: [f32::NAN; SIGNATURE_LEN],
            output_smoothed: math::Smoothed::new(1.0),
            rate: 48_000.0,
            dirty: true,
        }
    }

    fn signature_now(&self) -> [f32; SIGNATURE_LEN] {
        let mut signature = [0.0f32; SIGNATURE_LEN];
        for band in 0..BAND_COUNT {
            let slot = band * 4;
            signature[slot] = self.main[band].gain.value();
            signature[slot + 1] = self.main[band].kind.value();
            signature[slot + 2] = self.main[band].frequency.value();
            signature[slot + 3] = self.main[band].q.value();
            let right = BAND_COUNT * 4 + slot;
            signature[right] = self.right[band].gain.value();
            signature[right + 1] = self.right[band].kind.value();
            signature[right + 2] = self.right[band].frequency.value();
            signature[right + 3] = self.right[band].q.value();
        }
        signature[BAND_COUNT * 8] = self.channel_mode.value();
        signature
    }

    fn refresh(&mut self) {
        let bandwidth = self.bandwidth.value();
        let split = self.channel_mode.index() == 1;
        for band in 0..BAND_COUNT {
            let main = self.main[band].coefficients(self.rate, bandwidth);
            let right = self.right[band].coefficients(self.rate, bandwidth);
            for channel in 0..MAX_CHANNELS {
                let coefficients = if split && channel == 1 { right } else { main };
                self.bands[channel][band].set_coefficients(coefficients);
            }
        }
    }
}

impl Effect for Equalizer {
    fn id(&self) -> &'static str {
        "equalizer"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate() as f32;
        self.output_smoothed.set_time_constant(20.0, format.rate());
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.bands {
            for band in channel {
                band.reset();
            }
        }
        self.output_smoothed
            .snap(math::db_to_gain(self.output_gain.value()));
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let signature = self.signature_now();
        if self.dirty || signature != self.signature {
            self.signature = signature;
            self.dirty = false;
            self.refresh();
        }
        let channels = buffer.channels().min(MAX_CHANNELS);
        for channel in 0..channels {
            let stages = &mut self.bands[channel];
            for sample in buffer.channel_mut(channel) {
                let mut value = *sample;
                for stage in stages.iter_mut() {
                    value = stage.process(value);
                }
                *sample = value;
            }
        }
        self.output_smoothed
            .set_target(math::db_to_gain(self.output_gain.value()));
        let gain = self.output_smoothed.advance();
        if (gain - 1.0).abs() > f32::EPSILON {
            for channel in buffer.channels_mut() {
                for sample in channel.iter_mut() {
                    *sample = math::sanitize(*sample * gain);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
