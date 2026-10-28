use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("input_gain", "Input gain", ParamUnit::Decibel)
            .range(-20.0, 20.0, 0.0)
            .step(0.1)
            .group("Input")
            .build(),
        ParamSpec::float("limit", "Limit", ParamUnit::Decibel)
            .range(-30.0, 0.0, -1.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::float("release", "Release", ParamUnit::Milliseconds)
            .range(5.0, 1000.0, 60.0)
            .logarithmic()
            .group("Timing")
            .build(),
    ]
}

pub struct Limiter {
    input_gain: Knob,
    limit: Knob,
    release: Knob,
    gain: f32,
    release_coefficient: f32,
    input_smoothed: math::Smoothed,
    rate: u32,
}

impl Limiter {
    pub fn new(params: ParamSet) -> Self {
        Self {
            input_gain: Knob::new(&params, "input_gain", 0.0),
            limit: Knob::new(&params, "limit", -1.0),
            release: Knob::new(&params, "release", 60.0),
            gain: 1.0,
            release_coefficient: 0.001,
            input_smoothed: math::Smoothed::new(1.0),
            rate: 48_000,
        }
    }

    fn update_coefficient(&mut self) {
        let seconds = (self.release.value().max(0.5) / 1000.0).max(1.0e-6);
        self.release_coefficient = 1.0 - (-1.0 / (seconds * self.rate.max(1) as f32)).exp();
        self.input_smoothed.set_time_constant(20.0, self.rate);
    }
}

impl Effect for Limiter {
    fn id(&self) -> &'static str {
        "limiter"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.update_coefficient();
        self.reset();
    }

    fn reset(&mut self) {
        self.gain = 1.0;
        self.input_smoothed
            .snap(math::db_to_gain(self.input_gain.value()));
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        self.update_coefficient();
        self.input_smoothed
            .set_target(math::db_to_gain(self.input_gain.value()));
        let input_gain = self.input_smoothed.advance();
        let ceiling = math::db_to_gain(self.limit.value());
        let ceiling = ceiling.max(1.0e-5);
        let release = self.release_coefficient;
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();

        for frame in 0..frames {
            let mut peak = 0.0f32;
            for channel in 0..channels {
                let sample = buffer.channel_mut(channel)[frame] * input_gain;
                peak = peak.max(sample.abs());
            }
            let required = if peak > ceiling { ceiling / peak } else { 1.0 };
            if required < self.gain {
                self.gain = required;
            } else {
                self.gain += (1.0 - self.gain) * release;
                if self.gain > 1.0 {
                    self.gain = 1.0;
                }
            }
            let gain = self.gain * input_gain;
            for channel in 0..channels {
                let plan = buffer.channel_mut(channel);
                let value = plan[frame] * gain;
                plan[frame] = if value > ceiling {
                    ceiling
                } else if value < -ceiling {
                    -ceiling
                } else {
                    math::sanitize(value)
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hot_tone(amplitude: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample =
                    amplitude * (std::f32::consts::TAU * 300.0 * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

    #[test]
    fn the_ceiling_is_never_exceeded() {
        let params = ParamSet::new(specs());
        params.set_f32("input_gain", 20.0).unwrap();
        params.set_f32("limit", -6.0).unwrap();
        let mut effect = Limiter::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let ceiling = math::db_to_gain(-6.0);
        for _ in 0..128 {
            let mut buffer = hot_tone(1.0, 1024);
            effect.process(&mut buffer);
            for channel in 0..2 {
                for sample in buffer.channel(channel) {
                    assert!(sample.abs() <= ceiling + 1.0e-6, "{sample} > {ceiling}");
                }
            }
        }
    }

    #[test]
    fn quiet_signal_passes_through_unity() {
        let params = ParamSet::new(specs());
        let mut effect = Limiter::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = hot_tone(0.1, 4096);
        effect.process(&mut buffer);
        assert!((buffer.peak() - 0.1).abs() < 0.01, "{}", buffer.peak());
    }

    #[test]
    fn the_gain_recovers_after_a_transient() {
        let params = ParamSet::new(specs());
        params.set_f32("limit", -12.0).unwrap();
        params.set_f32("release", 5.0).unwrap();
        let mut effect = Limiter::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut burst = hot_tone(1.0, 256);
        effect.process(&mut burst);
        let mut quiet = hot_tone(0.02, 4096);
        for _ in 0..64 {
            effect.process(&mut quiet);
        }
        assert!((quiet.peak() - 0.02).abs() < 0.002, "{}", quiet.peak());
    }
}
