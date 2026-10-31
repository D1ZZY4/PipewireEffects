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
                    let original = buffer.channel(channel)[index];
                    let crushed = quantise(self.held[channel] + offset, levels, logarithmic_mode);
                    buffer.channel_mut(channel)[index] =
                        math::sanitize(original * (1.0 - mix) + crushed * mix);
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
#[path = "tests/tests.rs"]
mod tests;
