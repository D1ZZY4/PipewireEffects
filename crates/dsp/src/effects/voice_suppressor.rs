use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::fft::{Fft, hann, magnitude_db};
use crate::knob::Knob;

const FFT_SIZE: usize = 1024;
const HOP: usize = FFT_SIZE / 4;
const BINS: usize = FFT_SIZE / 2 + 1;
const WINDOW_SUM: f32 = 1.5;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("start", "Start", ParamUnit::Hertz)
            .range(40.0, 20_000.0, 200.0)
            .logarithmic()
            .group("Range")
            .build(),
        ParamSpec::float("end", "End", ParamUnit::Hertz)
            .range(40.0, 20_000.0, 8_000.0)
            .logarithmic()
            .group("Range")
            .build(),
        ParamSpec::float("correlation", "Correlation", ParamUnit::None)
            .range(0.0, 1.0, 0.5)
            .step(0.01)
            .group("Detection")
            .build(),
        ParamSpec::float("phase", "Phase difference", ParamUnit::None)
            .range(0.0, 180.0, 30.0)
            .step(1.0)
            .group("Detection")
            .build(),
        ParamSpec::boolean("inverted", "Inverted mode", false).group("Detection"),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
    ]
}

pub struct VoiceSuppressor {
    fft: Fft,
    window: Vec<f32>,
    start: Knob,
    end: Knob,
    correlation: Knob,
    phase: Knob,
    inverted: Knob,
    mix: Knob,
    rate: f32,
    input: [[f32; FFT_SIZE]; MAX_CHANNELS],
    output: [[f32; FFT_SIZE]; MAX_CHANNELS],
    re: [[f32; FFT_SIZE]; MAX_CHANNELS],
    im: [[f32; FFT_SIZE]; MAX_CHANNELS],
}

impl VoiceSuppressor {
    pub fn new(params: ParamSet) -> Self {
        Self {
            fft: Fft::new(FFT_SIZE),
            window: hann(FFT_SIZE),
            start: Knob::new(&params, "start", 200.0),
            end: Knob::new(&params, "end", 8_000.0),
            correlation: Knob::new(&params, "correlation", 0.5),
            phase: Knob::new(&params, "phase", 30.0),
            inverted: Knob::new(&params, "inverted", 0.0),
            mix: Knob::new(&params, "mix", 100.0),
            rate: 48_000.0,
            input: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            output: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            re: [[0.0; FFT_SIZE]; MAX_CHANNELS],
            im: [[0.0; FFT_SIZE]; MAX_CHANNELS],
        }
    }

    fn transform(&mut self, channels: usize) {
        let start = self.start.value();
        let end = self.end.value();
        let correlation = self.correlation.value();
        let phase_limit = self.phase.value().to_radians();
        let inverted = self.inverted.flag();
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let bin_hz = self.rate / FFT_SIZE as f32;
        for channel in 0..channels {
            for (index, slot) in self.re[channel].iter_mut().enumerate() {
                *slot = self.input[channel][index] * self.window[index];
            }
            self.im[channel].fill(0.0);
        }
        for channel in 0..channels {
            self.fft
                .forward(&mut self.re[channel], &mut self.im[channel]);
        }
        for bin in 1..BINS {
            let frequency = bin as f32 * bin_hz;
            if frequency < start || frequency > end {
                continue;
            }
            let (left_re, left_im) = if channels > 1 {
                (self.re[1][bin], self.im[1][bin])
            } else {
                (self.re[0][bin], self.im[0][bin])
            };
            let (right_re, right_im) = (self.re[0][bin], self.im[0][bin]);
            let left_power = left_re * left_re + left_im * left_im;
            let right_power = right_re * right_re + right_im * right_im;
            if magnitude_db(left_re, left_im).max(magnitude_db(right_re, right_im)) < -90.0 {
                continue;
            }
            let cross_re = left_re * right_re + left_im * right_im;
            let cross_im = left_im * right_re - left_re * right_im;
            let denominator = (left_power * right_power).sqrt();
            let coefficient = if denominator <= f32::MIN_POSITIVE {
                0.0
            } else {
                (cross_re / denominator).clamp(-1.0, 1.0)
            };
            let difference = cross_im.atan2(cross_re).abs();
            let matches = coefficient >= correlation && difference <= phase_limit;
            let voice = if inverted { !matches } else { matches };
            if !voice {
                continue;
            }
            let gain = 1.0 - mix;
            for channel in 0..channels {
                self.re[channel][bin] *= gain;
                self.im[channel][bin] *= gain;
            }
        }
        for channel in 0..channels {
            self.im[channel][0] = 0.0;
            self.im[channel][FFT_SIZE / 2] = 0.0;
            for bin in 1..BINS - 1 {
                self.re[channel][FFT_SIZE - bin] = self.re[channel][bin];
                self.im[channel][FFT_SIZE - bin] = -self.im[channel][bin];
            }
        }
    }

    fn synthesize(&mut self, channels: usize) {
        for channel in 0..channels {
            self.fft
                .inverse(&mut self.re[channel], &mut self.im[channel]);
            for (index, slot) in self.output[channel].iter_mut().enumerate() {
                *slot += self.re[channel][index] * self.window[index] / WINDOW_SUM;
            }
        }
    }

    fn advance(&mut self, channels: usize) {
        for channel in 0..channels {
            self.input[channel].copy_within(HOP.., 0);
            self.output[channel].copy_within(HOP.., 0);
            self.input[channel][FFT_SIZE - HOP..].fill(0.0);
            self.output[channel][FFT_SIZE - HOP..].fill(0.0);
        }
    }
}

impl Effect for VoiceSuppressor {
    fn id(&self) -> &'static str {
        "voice_suppressor"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate() as f32;
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.input {
            channel.fill(0.0);
        }
        for channel in &mut self.output {
            channel.fill(0.0);
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();
        let mut position = 0;
        while position < frames {
            let take = HOP.min(frames - position);
            for channel in 0..channels {
                let plan = buffer.channel(channel);
                self.input[channel][FFT_SIZE - HOP..FFT_SIZE - HOP + take]
                    .copy_from_slice(&plan[position..position + take]);
            }
            self.transform(channels);
            self.synthesize(channels);
            for channel in 0..channels {
                let plan = buffer.channel_mut(channel);
                for index in 0..take {
                    plan[position + index] = math::sanitize(self.output[channel][index]);
                }
            }
            self.advance(channels);
            position += take;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frequency: f32, frames: usize, offset: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let sign = if channel == 1 { -1.0 } else { 1.0 };
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample = sign
                    * (std::f32::consts::TAU * frequency * (offset + index) as f32 / 48_000.0)
                        .sin();
            }
        }
        buffer
    }

    fn rms(effect: &mut VoiceSuppressor, inverted_channels: bool) -> f32 {
        let frames = 512;
        let mut total = 0.0f64;
        let mut count = 0usize;
        for block in 0..48 {
            let mut buffer = tone(1_000.0, frames, block * frames);
            if !inverted_channels {
                let right = buffer.channel(0).to_vec();
                buffer.channel_mut(1).copy_from_slice(&right);
            }
            effect.process(&mut buffer);
            if block >= 16 {
                for channel in 0..2 {
                    for sample in buffer.channel(channel) {
                        total += f64::from(*sample) * f64::from(*sample);
                        count += 1;
                    }
                }
            }
        }
        (total / count as f64).sqrt() as f32
    }

    #[test]
    fn centred_voice_is_attenuated() {
        let params = ParamSet::new(specs());
        let mut effect = VoiceSuppressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let level = rms(&mut effect, false);
        assert!(level < 0.15, "the centre was not suppressed: {level}");
    }

    #[test]
    fn an_out_of_phase_signal_is_preserved() {
        let params = ParamSet::new(specs());
        let mut effect = VoiceSuppressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let level = rms(&mut effect, true);
        assert!(level > 0.5, "the side signal was suppressed: {level}");
    }

    #[test]
    fn zero_mix_is_transparent() {
        let params = ParamSet::new(specs());
        params.set_f32("mix", 0.0).unwrap();
        let mut effect = VoiceSuppressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let level = rms(&mut effect, false);
        assert!(level > 0.5, "the signal was altered at zero mix: {level}");
    }
}
