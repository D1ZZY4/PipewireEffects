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
mod tests {
    use super::*;

    fn tone(start: usize, frequency: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                let phase = std::f32::consts::TAU * frequency * (start + index) as f32 / 48_000.0;
                *sample = phase.sin();
            }
        }
        buffer
    }

    fn measure(mode: u16, cutoff: f32, order: f32, probe: f32) -> f32 {
        let params = ParamSet::new(specs());
        params.set_choice("mode", mode).unwrap();
        params.set_f32("frequency", cutoff).unwrap();
        params.set_f32("order", order).unwrap();
        params
            .set_f32("q", std::f32::consts::FRAC_1_SQRT_2)
            .unwrap();
        let mut effect = Filter::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut start = 0usize;
        let mut sum = 0.0f64;
        let mut count = 0usize;
        for block in 0..64 {
            let mut buffer = tone(start, probe, 1024);
            start += 1024;
            effect.process(&mut buffer);
            if block >= 60 {
                for sample in buffer.channel(0) {
                    sum += f64::from(*sample) * f64::from(*sample);
                    count += 1;
                }
            }
        }
        ((sum / count as f64).sqrt() / std::f64::consts::FRAC_1_SQRT_2) as f32
    }

    #[test]
    fn low_pass_keeps_the_passband_and_cuts_the_stopband() {
        assert!(measure(0, 4_000.0, 2.0, 500.0) > 0.99);
        assert!(measure(0, 1_000.0, 2.0, 8_000.0) < 0.01);
    }

    #[test]
    fn high_pass_cuts_below_the_cutoff() {
        assert!(measure(1, 4_000.0, 2.0, 100.0) < 0.01);
        assert!(measure(1, 1_000.0, 2.0, 10_000.0) > 0.99);
    }

    #[test]
    fn a_higher_order_adds_more_slope() {
        let second = measure(0, 1_000.0, 2.0, 4_000.0);
        let fourth = measure(0, 1_000.0, 4.0, 4_000.0);
        assert!(fourth < second, "{fourth} !< {second}");
        assert!(
            (fourth - second * second).abs() < 1.0e-3,
            "{fourth} vs {}",
            second * second
        );
    }

    #[test]
    fn band_pass_only_passes_the_band() {
        assert!(measure(2, 1_000.0, 2.0, 1_000.0) > 0.99);
        assert!(measure(2, 1_000.0, 2.0, 50.0) < 0.01);
        assert!(measure(2, 1_000.0, 2.0, 18_000.0) < 0.05);
    }

    #[test]
    fn notch_rejects_the_centre_frequency() {
        assert!(measure(3, 1_000.0, 2.0, 1_000.0) < 0.01);
        assert!(measure(3, 1_000.0, 2.0, 50.0) > 0.99);
    }

    #[test]
    fn shelves_leave_the_far_end_alone() {
        assert!(measure(6, 1_000.0, 2.0, 16_000.0) > 0.99);
        assert!(measure(7, 1_000.0, 2.0, 40.0) > 0.95);
    }

    #[test]
    fn extreme_settings_stay_finite() {
        let params = ParamSet::new(specs());
        params.set_f32("q", 20.0).unwrap();
        params.set_f32("frequency", 20.0).unwrap();
        params.set_f32("gain", 24.0).unwrap();
        params.set_choice("mode", 5).unwrap();
        params.set_f32("order", 4.0).unwrap();
        let mut effect = Filter::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut start = 0usize;
        for _ in 0..32 {
            let mut buffer = tone(start, 20.0, 4096);
            start += 4096;
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(sample.is_finite() && sample.abs() < 1.0e6, "{sample}");
                }
            }
        }
    }

    #[test]
    fn changing_the_mode_reconfigures_the_filter() {
        let params = ParamSet::new(specs());
        let mut effect = Filter::new(params.clone());
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0, 100.0, 1024);
        effect.process(&mut buffer);
        let low_pass_peak = buffer.peak();
        params.set_choice("mode", 1).unwrap();
        let mut buffer = tone(0, 100.0, 1024);
        effect.process(&mut buffer);
        assert!(
            buffer.peak() < low_pass_peak,
            "{} !< {low_pass_peak}",
            buffer.peak()
        );
    }
}
