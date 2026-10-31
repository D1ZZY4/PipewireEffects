use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

mod channel;
mod dither;
mod oversampled;
mod resample;

use channel::Channel;
use dither::Dither;
use resample::Resampler;

const MAX_LOOKAHEAD_RATE: usize = 192_000;
const MAX_LOOKAHEAD_MS: usize = 20;
const MAX_LOOKAHEAD_FRAMES: usize = MAX_LOOKAHEAD_RATE * MAX_LOOKAHEAD_MS / 1000;
const MAX_OVERSAMPLE: usize = 4;
const DELAY_LENGTH: usize = MAX_LOOKAHEAD_FRAMES * MAX_OVERSAMPLE + 1;
const OVERSAMPLING: [&str; 3] = [mark("1x"), mark("2x"), mark("4x")];
const FACTORS: [usize; 3] = [1, 2, 4];
const DITHER_MODES: [&str; 3] = [mark("Off"), mark("Rectangular"), mark("Triangular")];
const RELEASE_MODES: [&str; 2] = [mark("Standard"), mark("ALR")];
const DITHER_LSB: f32 = 1.0 / 32_768.0;
const MAX_BOOST: f32 = 4.0;

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
        ParamSpec::boolean("gain_boost", "Gain boost", false).group("Output"),
        ParamSpec::float("lookahead", "Lookahead", ParamUnit::Milliseconds)
            .range(0.1, MAX_LOOKAHEAD_MS as f32, 5.0)
            .step(0.1)
            .group("Timing")
            .build(),
        ParamSpec::float("attack", "Attack", ParamUnit::Milliseconds)
            .range(0.1, MAX_LOOKAHEAD_MS as f32, MAX_LOOKAHEAD_MS as f32)
            .step(0.1)
            .group("Timing")
            .build(),
        ParamSpec::float("release", "Release", ParamUnit::Milliseconds)
            .range(5.0, 1000.0, 60.0)
            .logarithmic()
            .group("Timing")
            .build(),
        ParamSpec::choice("release_mode", "Release mode", &RELEASE_MODES, 0).group("Timing"),
        ParamSpec::float("stereo_link", "Stereo link", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Timing")
            .build(),
        ParamSpec::choice("oversampling", "Oversampling", &OVERSAMPLING, 0).group("Quality"),
        ParamSpec::choice("dithering", "Dithering", &DITHER_MODES, 0).group("Quality"),
    ]
}

pub struct Limiter {
    channels: Vec<Channel>,
    input_gain: Knob,
    limit: Knob,
    lookahead: Knob,
    attack: Knob,
    release: Knob,
    link: Knob,
    gain_boost: Knob,
    release_mode: Knob,
    oversampling: Knob,
    dithering: Knob,
    resampler: Resampler,
    dither: Dither,
    frames: usize,
    ramp: usize,
    factor: usize,
    forget: f32,
    release_coefficient: f32,
    input_smoothed: math::Smoothed,
    boost_reference: f32,
    boost_attack: f32,
    boost_release: f32,
    rate: u32,
}

impl Limiter {
    pub fn new(params: ParamSet) -> Self {
        Self {
            channels: (0..MAX_CHANNELS).map(|_| Channel::new()).collect(),
            input_gain: Knob::new(&params, "input_gain", 0.0),
            limit: Knob::new(&params, "limit", -1.0),
            lookahead: Knob::new(&params, "lookahead", 5.0),
            attack: Knob::new(&params, "attack", MAX_LOOKAHEAD_MS as f32),
            release: Knob::new(&params, "release", 60.0),
            link: Knob::new(&params, "stereo_link", 100.0),
            gain_boost: Knob::new(&params, "gain_boost", 0.0),
            release_mode: Knob::new(&params, "release_mode", 0.0),
            oversampling: Knob::new(&params, "oversampling", 0.0),
            dithering: Knob::new(&params, "dithering", 0.0),
            resampler: Resampler::new(),
            dither: Dither::new(),
            frames: 1,
            ramp: 1,
            factor: 1,
            forget: 0.2,
            release_coefficient: 0.001,
            input_smoothed: math::Smoothed::new(1.0),
            boost_reference: 1.0,
            boost_attack: 0.001,
            boost_release: 0.0001,
            rate: 48_000,
        }
    }

    fn oversampling_factor(&self) -> usize {
        FACTORS[self.oversampling.index().min(FACTORS.len() - 1)]
    }

    fn update_coefficient(&mut self) {
        let factor = self.oversampling_factor().min(MAX_OVERSAMPLE);
        if factor != self.factor {
            self.factor = factor;
            self.resampler.set_factor(factor);
            for channel in &mut self.channels {
                channel.reset();
            }
        }
        let high_rate = self.rate.saturating_mul(factor as u32).max(1);
        let seconds = (self.release.value().max(0.5) / 1000.0).max(1.0e-6);
        self.release_coefficient = 1.0 - (-1.0 / (seconds * high_rate as f32)).exp();
        self.input_smoothed.set_time_constant(20.0, self.rate);
        self.boost_attack = math::one_pole_alpha(20.0, self.rate);
        self.boost_release = math::one_pole_alpha(500.0, self.rate);
        let frames = (self.lookahead.value() / 1000.0 * self.rate as f32).round() as usize;
        self.frames = frames.clamp(1, MAX_LOOKAHEAD_FRAMES);
        let lookahead = self.frames * factor;
        self.forget = 1.0 - (-1.0 / lookahead as f32).exp();
        let attack = (self.attack.value() / 1000.0 * high_rate as f32).round() as usize;
        self.ramp = attack.clamp(1, lookahead);
        for channel in &mut self.channels {
            channel.window = lookahead;
        }
    }

    fn boost_makeup(&self) -> f32 {
        if self.gain_boost.flag() {
            (1.0 / self.boost_reference.max(1.0 / MAX_BOOST)).clamp(1.0, MAX_BOOST)
        } else {
            1.0
        }
    }

    fn required(peak: f32, ceiling: f32) -> f32 {
        let peak = peak * peak;
        let ceiling_squared = ceiling * ceiling;
        if peak > ceiling_squared {
            (ceiling_squared / peak).sqrt()
        } else {
            1.0
        }
    }

    fn process_direct(&mut self, buffer: &mut AudioBuffer) {
        let input_gain = self.input_smoothed.advance() * self.boost_makeup();
        let ceiling = math::db_to_gain(self.limit.value()).max(1.0e-5);
        let release = self.release_coefficient;
        let forget = self.forget;
        let link = (self.link.value() / 100.0).clamp(0.0, 1.0);
        let alr = self.release_mode.index() == 1;
        let dither_mode = self.dithering.index();
        let ramp = self.ramp;
        let channels = buffer.channels().min(self.channels.len());
        let frames = buffer.frames();
        let lookahead = self.frames;
        let mut delayed = [0.0f32; MAX_CHANNELS];
        let mut need = [1.0f32; MAX_CHANNELS];
        let mut gains = [1.0f32; MAX_CHANNELS];

        for frame in 0..frames {
            for (index, channel) in self.channels.iter_mut().enumerate().take(channels) {
                let length = channel.delay.len();
                let read = (channel.head + length - lookahead) % length;
                let incoming = buffer.channel(index)[frame] * input_gain;
                delayed[index] = channel.delay[read];
                channel.delay[channel.head] = incoming;
                channel.head = (channel.head + 1) % length;
                need[index] = Self::required(incoming, ceiling);
            }

            let shared = need
                .iter()
                .take(channels)
                .fold(1.0f32, |acc, value| acc.min(*value));
            let coefficient = if shared < self.boost_reference {
                self.boost_attack
            } else {
                self.boost_release
            };
            self.boost_reference += (shared - self.boost_reference) * coefficient;
            if !self.boost_reference.is_finite() {
                self.boost_reference = 1.0;
            }
            for (value, channel) in need.iter().zip(self.channels.iter_mut()).take(channels) {
                channel.hold = if *value < channel.hold {
                    *value
                } else {
                    channel.hold + (1.0 - channel.hold) * forget
                };
            }
            for ((delayed_value, channel), gain) in delayed
                .iter()
                .enumerate()
                .zip(self.channels.iter_mut())
                .zip(gains.iter_mut())
                .take(channels)
            {
                let index = delayed_value.0;
                let delayed_value = delayed_value.1;
                let own = channel.hold;
                let target = (shared * link + own * (1.0 - link)).min(own);
                let exact = Self::required(delayed_value * input_gain, ceiling);
                channel.advance(need[index], target, ramp, release, exact, alr);
                *gain = channel.envelope;
            }
            for (index, gain) in gains.iter().enumerate().take(channels) {
                let noise = self.dither.sample(dither_mode, DITHER_LSB);
                let sample = (delayed[index] * gain * input_gain + noise).clamp(-ceiling, ceiling);
                buffer.channel_mut(index)[frame] = math::sanitize(sample);
            }
        }
    }
}

impl Effect for Limiter {
    fn id(&self) -> &'static str {
        "limiter"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.factor = 0;
        self.update_coefficient();
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
        self.resampler.reset();
        self.boost_reference = 1.0;
        self.input_smoothed
            .snap(math::db_to_gain(self.input_gain.value()));
    }

    fn latency_frames(&self) -> usize {
        if self.factor > 1 {
            self.frames + self.resampler.latency()
        } else {
            self.frames
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        self.update_coefficient();
        self.input_smoothed
            .set_target(math::db_to_gain(self.input_gain.value()));
        if self.factor > 1 {
            self.process_oversampled(buffer);
        } else {
            self.process_direct(buffer);
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
