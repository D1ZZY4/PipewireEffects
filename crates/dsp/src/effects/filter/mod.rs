use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::choice("mode", "Mode", &FilterKind::LABELS, 0).group("Filter"),
        ParamSpec::float("frequency", "Cutoff", ParamUnit::Hertz)
            .range(20.0, 20_000.0, 1_000.0)
            .logarithmic()
            .group("Filter")
            .build(),
        ParamSpec::float("q", "Resonance", ParamUnit::None)
            .range(0.1, 20.0, std::f32::consts::FRAC_1_SQRT_2)
            .step(0.01)
            .group("Filter")
            .build(),
        ParamSpec::float("gain", "Gain", ParamUnit::Decibel)
            .range(-24.0, 24.0, 0.0)
            .step(0.1)
            .group("Filter")
            .build(),
        ParamSpec::float("order", "Order", ParamUnit::None)
            .range(1.0, 4.0, 2.0)
            .step(1.0)
            .group("Filter")
            .build(),
    ]
}

pub struct Filter {
    mode: Knob,
    frequency: Knob,
    q: Knob,
    gain: Knob,
    order: Knob,
    sections: [[Biquad; 4]; MAX_CHANNELS],
    applied: Coefficients,
    applied_order: usize,
    rate: f32,
    dirty: bool,
}

impl Filter {
    pub fn new(params: ParamSet) -> Self {
        Self {
            mode: Knob::new(&params, "mode", 0.0),
            frequency: Knob::new(&params, "frequency", 1_000.0),
            q: Knob::new(&params, "q", std::f32::consts::FRAC_1_SQRT_2),
            gain: Knob::new(&params, "gain", 0.0),
            order: Knob::new(&params, "order", 2.0),
            sections: [[Biquad::default(); 4]; MAX_CHANNELS],
            applied: Coefficients::identity(),
            applied_order: 0,
            rate: 48_000.0,
            dirty: true,
        }
    }

    fn target(&self) -> (Coefficients, usize) {
        let kind = FilterKind::from_index(self.mode.index());
        let order = self.order.value().round().clamp(1.0, 4.0) as usize;
        let coefficients = Coefficients::design(
            kind,
            self.rate,
            self.frequency.value(),
            self.q.value(),
            self.gain.value(),
        );
        (coefficients, order)
    }

    fn refresh(&mut self, coefficients: Coefficients, order: usize) {
        for channel in 0..MAX_CHANNELS {
            for stage in 0..4 {
                self.sections[channel][stage].set_coefficients(if stage < order {
                    coefficients
                } else {
                    Coefficients::identity()
                });
            }
        }
        self.applied = coefficients;
        self.applied_order = order;
    }
}

impl Effect for Filter {
    fn id(&self) -> &'static str {
        "filter"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate() as f32;
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.sections {
            for biquad in channel {
                biquad.reset();
            }
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let (coefficients, order) = self.target();
        if self.dirty || coefficients != self.applied || order != self.applied_order {
            self.dirty = false;
            self.refresh(coefficients, order);
        }
        let channels = buffer.channels().min(MAX_CHANNELS);
        for channel in 0..channels {
            let stages = &mut self.sections[channel][..order];
            for sample in buffer.channel_mut(channel) {
                let mut value = *sample;
                for stage in stages.iter_mut() {
                    value = stage.process(value);
                }
                *sample = value;
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
