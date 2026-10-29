use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const MAX_FACTOR: u32 = 250;
const MODES: [&str; 2] = [mark("Linear"), mark("Logarithmic")];

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::choice("mode", "Mode", &MODES, 0).group("Crusher"),
        ParamSpec::float("bits", "Bit depth", ParamUnit::None)
            .range(1.0, 16.0, 4.0)
            .step(1.0)
            .group("Crusher")
            .build(),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 50.0)
            .step(1.0)
            .group("Crusher")
            .build(),
        ParamSpec::float("samples", "Sample reduction", ParamUnit::Samples)
            .range(1.0, MAX_FACTOR as f32, 1.0)
            .step(1.0)
            .group("Crusher")
            .build(),
        ParamSpec::float("anti_aliasing", "Anti-aliasing", ParamUnit::Percent)
            .range(0.0, 100.0, 50.0)
            .step(1.0)
            .group("Crusher")
            .build(),
        ParamSpec::float("dc", "DC offset", ParamUnit::Decibel)
            .range(-12.041, 12.041, 0.0)
            .step(0.001)
            .group("Output")
            .build(),
        ParamSpec::boolean("lfo_active", "Modulate the reduction", false).group("Modulation"),
        ParamSpec::float("lfo_depth", "Depth", ParamUnit::Samples)
            .range(1.0, MAX_FACTOR as f32, 20.0)
            .step(1.0)
            .group("Modulation")
            .build(),
        ParamSpec::float("lfo_rate", "Rate", ParamUnit::Hertz)
            .range(0.01, 200.0, 0.3)
            .logarithmic()
            .group("Modulation")
            .build(),
    ]
}

pub struct Crusher {
    mode: Knob,
    bits: Knob,
    mix: Knob,
    samples: Knob,
    anti_aliasing: Knob,
    dc: Knob,
    lfo_active: Knob,
    lfo_depth: Knob,
    lfo_rate: Knob,
    rate: u32,
    phase: f32,
    held: [f32; MAX_CHANNELS],
    smoothed: [f32; MAX_CHANNELS],
}

impl Crusher {
    pub fn new(params: ParamSet) -> Self {
        Self {
            mode: Knob::new(&params, "mode", 0.0),
            bits: Knob::new(&params, "bits", 4.0),
            mix: Knob::new(&params, "mix", 50.0),
            samples: Knob::new(&params, "samples", 1.0),
            anti_aliasing: Knob::new(&params, "anti_aliasing", 50.0),
            dc: Knob::new(&params, "dc", 0.0),
            lfo_active: Knob::new(&params, "lfo_active", 0.0),
            lfo_depth: Knob::new(&params, "lfo_depth", 20.0),
            lfo_rate: Knob::new(&params, "lfo_rate", 0.3),
            rate: 48_000,
            phase: 0.0,
            held: [0.0; MAX_CHANNELS],
            smoothed: [0.0; MAX_CHANNELS],
        }
    }

    fn factor(&self) -> usize {
        let base = self.samples.value();
        let factor = if self.lfo_active.flag() {
            let lfo = 0.5 - 0.5 * (std::f32::consts::TAU * self.phase).cos();
            base + self.lfo_depth.value() * lfo
        } else {
            base
        };
        (factor.round() as u32).clamp(1, MAX_FACTOR) as usize
    }
}

fn uniform(sample: f32, levels: f32) -> f32 {
    let step = 2.0 / levels;
    (sample / step).round() * step
}

fn logarithmic(sample: f32, levels: f32) -> f32 {
    const FLOOR_DB: f32 = -60.0;
    let floor = 10.0f32.powf(FLOOR_DB / 20.0);
    let log_step = (1.0 / floor).ln() / levels;
    let magnitude = sample.abs();
    if magnitude <= floor * 0.5 || log_step <= 0.0 {
        return 0.0;
    }
    let value = (magnitude.ln() / log_step).round() * log_step;
    let sign = if sample < 0.0 { -1.0 } else { 1.0 };
    sign * value.exp().min(1.0)
}

fn quantise(sample: f32, levels: f32, logarithmic_mode: bool) -> f32 {
    if !sample.is_finite() || levels < 2.0 {
        return 0.0;
    }
    if logarithmic_mode {
        logarithmic(sample, levels)
    } else {
        uniform(sample, levels)
    }
}

impl Effect for Crusher {
    fn id(&self) -> &'static str {
        "crusher"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.reset();
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.held = [0.0; MAX_CHANNELS];
        self.smoothed = [0.0; MAX_CHANNELS];
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let bits = self.bits.value().round().clamp(1.0, 16.0);
        let levels = 2.0f32.powf(bits);
        let logarithmic_mode = self.mode.index() == 1;
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let alpha = 1.0 - 0.9 * (self.anti_aliasing.value() / 100.0).clamp(0.0, 1.0);
        let offset = math::db_to_gain(self.dc.value()) - 1.0;
        let rate = self.rate.max(1) as f32;
        let lfo_step = (self.lfo_rate.value() / rate).clamp(0.0, 0.5);
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();
        let mut frame = 0;

        while frame < frames {
            let factor = self.factor();
            let end = (frame + factor).min(frames);
            for channel in 0..channels {
                let mut state = self.smoothed[channel];
                for index in frame..end {
                    let input = buffer.channel(channel)[index];
                    state = math::sanitize(state + (input - state) * alpha);
                    if index == frame {
                        self.held[channel] = state;
                    }
                }
                self.smoothed[channel] = state;
            }
            for index in frame..end {
                for channel in 0..channels {
                    let dry = self.held[channel];
                    let crushed = quantise(dry + offset, levels, logarithmic_mode);
                    buffer.channel_mut(channel)[index] =
                        math::sanitize(dry * (1.0 - mix) + crushed * mix);
                }
            }
            self.phase += lfo_step * factor as f32;
            if self.phase >= 1.0 {
                self.phase -= self.phase.floor();
            }
            frame = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(samples: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, samples);
        buffer.configure(2, samples).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample = -1.0 + 2.0 * index as f32 / samples as f32;
            }
        }
        buffer
    }

    fn constant(amplitude: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            buffer.channel_mut(channel).fill(amplitude);
        }
        buffer
    }

    fn effect(settings: &[(&str, f32)]) -> Crusher {
        let params = ParamSet::new(specs());
        for (key, value) in settings {
            params.set_f32(key, *value).unwrap();
        }
        let mut effect = Crusher::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        effect
    }

    #[test]
    fn sixteen_bits_at_full_rate_is_transparent() {
        let mut crusher = effect(&[
            ("bits", 16.0),
            ("mix", 100.0),
            ("samples", 1.0),
            ("anti_aliasing", 0.0),
        ]);
        let mut buffer = ramp(4096);
        let source: Vec<f32> = buffer.channel(0).to_vec();
        crusher.process(&mut buffer);
        for (processed, original) in buffer.channel(0).iter().zip(source) {
            assert!(
                (processed - original).abs() < 1.0e-4,
                "{processed} {original}"
            );
        }
    }

    #[test]
    fn a_zero_mix_keeps_the_original_signal() {
        let mut crusher = effect(&[("bits", 1.0), ("mix", 0.0), ("anti_aliasing", 0.0)]);
        let mut buffer = ramp(1024);
        let source: Vec<f32> = buffer.channel(0).to_vec();
        crusher.process(&mut buffer);
        for (processed, original) in buffer.channel(0).iter().zip(source) {
            assert!(
                (processed - original).abs() < 1.0e-6,
                "{processed} {original}"
            );
        }
    }

    #[test]
    fn two_bits_quantises_to_linear_steps() {
        let mut crusher = effect(&[
            ("bits", 2.0),
            ("mix", 100.0),
            ("samples", 1.0),
            ("anti_aliasing", 0.0),
        ]);
        let mut buffer = ramp(2048);
        crusher.process(&mut buffer);
        for sample in buffer.channel(0) {
            let steps = sample / 0.5;
            assert!((steps - steps.round()).abs() < 1.0e-4, "{sample}");
            assert!(sample.abs() <= 1.0 + 1.0e-4);
        }
    }

    #[test]
    fn the_logarithmic_mode_does_not_gate_low_levels() {
        let mut linear = effect(&[("bits", 4.0), ("mix", 100.0), ("anti_aliasing", 0.0)]);
        let mut logarithmic = effect(&[
            ("bits", 4.0),
            ("mix", 100.0),
            ("mode", 1.0),
            ("anti_aliasing", 0.0),
        ]);
        let mut quiet = constant(0.01, 64);
        let mut other = constant(0.01, 64);
        linear.process(&mut quiet);
        logarithmic.process(&mut other);
        assert_eq!(quiet.peak(), 0.0);
        assert!(other.peak() > 0.001, "{}", other.peak());
    }

    #[test]
    fn sample_reduction_holds_each_value_for_the_factor() {
        let mut crusher = effect(&[
            ("bits", 16.0),
            ("mix", 100.0),
            ("samples", 8.0),
            ("anti_aliasing", 0.0),
        ]);
        let mut buffer = ramp(2048);
        crusher.process(&mut buffer);
        let plan = buffer.channel(0);
        for block in plan.chunks(8) {
            assert!(
                block.iter().all(|value| (value - block[0]).abs() < 1.0e-6),
                "{block:?}"
            );
        }
    }

    #[test]
    fn modulation_moves_the_reduction_factor() {
        let mut crusher = effect(&[
            ("bits", 16.0),
            ("mix", 100.0),
            ("samples", 1.0),
            ("anti_aliasing", 0.0),
            ("lfo_active", 1.0),
            ("lfo_depth", 40.0),
            ("lfo_rate", 20.0),
        ]);
        let mut buffer = ramp(8192);
        crusher.process(&mut buffer);
        let repeats = buffer
            .channel(0)
            .windows(2)
            .filter(|pair| (pair[0] - pair[1]).abs() < 1.0e-9)
            .count();
        assert!(repeats > 0, "the modulated crusher never held a sample");
    }

    #[test]
    fn abusive_settings_stay_finite_and_bounded() {
        let mut crusher = effect(&[
            ("bits", 1.0),
            ("samples", 250.0),
            ("anti_aliasing", 100.0),
            ("dc", 12.041),
            ("mix", 100.0),
        ]);
        let mut buffer = ramp(4096);
        crusher.process(&mut buffer);
        for sample in buffer.channel(0) {
            assert!(sample.is_finite(), "{sample}");
            assert!(sample.abs() <= 5.0, "{sample}");
        }
    }
}
