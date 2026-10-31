use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

mod parts;

use parts::{AllPass, Comb};

const COMB_TUNING: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING: [usize; 4] = [556, 441, 341, 225];
const MAX_SCALE: f32 = 6.0;
const MAX_PRE_DELAY: usize = 38_400;
const CHARACTERS: &[&str] = &[
    mark("Small"),
    mark("Medium"),
    mark("Large"),
    mark("Tunnel-like"),
    mark("Large/smooth"),
    mark("Experimental"),
];
const CHARACTER_SHAPES: [(f32, f32, usize); 6] = [
    (0.50, 0.50, 11),
    (0.75, 0.75, 17),
    (1.00, 1.00, 23),
    (1.30, 0.60, 31),
    (1.15, 1.15, 23),
    (0.90, 1.40, 47),
];
const MAX_SPREAD: usize = 47;

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("room_size", "Room size", ParamUnit::Percent)
            .range(0.0, 100.0, 60.0)
            .step(1.0)
            .group("Room")
            .build(),
        ParamSpec::float("damping", "Damping", ParamUnit::Percent)
            .range(0.0, 100.0, 50.0)
            .step(1.0)
            .group("Room")
            .build(),
        ParamSpec::float("diffusion", "Diffusion", ParamUnit::Percent)
            .range(0.0, 100.0, 50.0)
            .step(1.0)
            .group("Room")
            .build(),
        ParamSpec::float("tank_mix", "Tank mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Room")
            .build(),
        ParamSpec::float("pre_delay", "Pre-delay", ParamUnit::Milliseconds)
            .range(0.0, 200.0, 20.0)
            .step(1.0)
            .group("Room")
            .build(),
        ParamSpec::choice("character", "Character", CHARACTERS, 2).group("Room"),
        ParamSpec::float("bass_cut", "Bass cut", ParamUnit::Hertz)
            .range(20.0, 1000.0, 300.0)
            .logarithmic()
            .group("Tone")
            .build(),
        ParamSpec::float("treble_cut", "Treble cut", ParamUnit::Hertz)
            .range(1000.0, 20_000.0, 5000.0)
            .logarithmic()
            .group("Tone")
            .build(),
        ParamSpec::float("width", "Width", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
        ParamSpec::float("wet", "Wet", ParamUnit::Decibel)
            .range(-60.0, 0.0, -6.0)
            .step(0.5)
            .group("Output")
            .build(),
        ParamSpec::float("dry", "Dry", ParamUnit::Decibel)
            .range(-60.0, 0.0, 0.0)
            .step(0.5)
            .group("Output")
            .build(),
    ]
}

pub struct Reverb {
    room_size: Knob,
    character: Knob,
    damping: Knob,
    diffusion: Knob,
    tank_mix: Knob,
    pre_delay: Knob,
    bass_cut: Knob,
    treble_cut: Knob,
    width: Knob,
    wet: Knob,
    dry: Knob,
    combs: [[Comb; 8]; 2],
    allpasses: [[AllPass; 4]; 2],
    tone: [[Biquad; 2]; 2],
    tone_signature: [u32; 4],
    pre_delay_buffer: Vec<f32>,
    pre_delay_index: usize,
    rate: u32,
    shape: [u32; 2],
    dirty: bool,
}

impl Reverb {
    pub fn new(params: ParamSet) -> Self {
        let combs = std::array::from_fn(|channel| {
            std::array::from_fn(|band| {
                let spread = if channel == 1 { MAX_SPREAD } else { 0 };
                Comb::new(((COMB_TUNING[band] + spread) as f32 * MAX_SCALE) as usize)
            })
        });
        let allpasses = std::array::from_fn(|channel| {
            std::array::from_fn(|band| {
                let spread = if channel == 1 { MAX_SPREAD } else { 0 };
                AllPass::new(((ALLPASS_TUNING[band] + spread) as f32 * MAX_SCALE) as usize)
            })
        });
        Self {
            room_size: Knob::new(&params, "room_size", 60.0),
            character: Knob::new(&params, "character", 2.0),
            damping: Knob::new(&params, "damping", 50.0),
            diffusion: Knob::new(&params, "diffusion", 50.0),
            tank_mix: Knob::new(&params, "tank_mix", 100.0),
            pre_delay: Knob::new(&params, "pre_delay", 20.0),
            bass_cut: Knob::new(&params, "bass_cut", 300.0),
            treble_cut: Knob::new(&params, "treble_cut", 5000.0),
            width: Knob::new(&params, "width", 100.0),
            wet: Knob::new(&params, "wet", -6.0),
            dry: Knob::new(&params, "dry", 0.0),
            combs,
            allpasses,
            tone: std::array::from_fn(|_| {
                std::array::from_fn(|_| {
                    Biquad::new(Coefficients::design(
                        FilterKind::LowPass,
                        48_000.0,
                        20_000.0,
                        0.707,
                        0.0,
                    ))
                })
            }),
            tone_signature: [0; 4],
            pre_delay_buffer: vec![0.0; MAX_PRE_DELAY * 2],
            pre_delay_index: 0,
            rate: 48_000,
            shape: [0; 2],
            dirty: true,
        }
    }

    fn tune_tone(&mut self, bass_cut: f32, treble_cut: f32) {
        let wanted = [bass_cut.to_bits(), treble_cut.to_bits(), self.rate, 0];
        if wanted == self.tone_signature {
            return;
        }
        self.tone_signature = wanted;
        for channel in &mut self.tone {
            channel[0] = Biquad::new(Coefficients::design(
                FilterKind::HighPass,
                self.rate as f32,
                bass_cut,
                0.707,
                0.0,
            ));
            channel[1] = Biquad::new(Coefficients::design(
                FilterKind::LowPass,
                self.rate as f32,
                treble_cut,
                0.707,
                0.0,
            ));
        }
    }

    fn tune(&mut self) {
        let (comb_scale, allpass_scale, spread_size) =
            CHARACTER_SHAPES[self.character.index().min(CHARACTER_SHAPES.len() - 1)];
        let scale = self.rate.max(1) as f32 / 44_100.0;
        for channel in 0..2 {
            let spread = if channel == 1 { spread_size } else { 0 };
            for (band, tuning) in COMB_TUNING.iter().enumerate() {
                self.combs[channel][band]
                    .set_length(((*tuning as f32 * comb_scale + spread as f32) * scale) as usize);
            }
            for (band, tuning) in ALLPASS_TUNING.iter().enumerate() {
                self.allpasses[channel][band].set_length(
                    ((*tuning as f32 * allpass_scale + spread as f32) * scale) as usize,
                );
            }
        }
    }
}

impl Effect for Reverb {
    fn id(&self) -> &'static str {
        "reverb"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.dirty = true;
        self.reset();
    }

    fn reset(&mut self) {
        for channel in &mut self.combs {
            for comb in channel {
                comb.reset();
            }
        }
        for channel in &mut self.allpasses {
            for allpass in channel {
                allpass.reset();
            }
        }
        for channel in &mut self.tone {
            for filter in channel {
                filter.reset();
            }
        }
        self.pre_delay_buffer.fill(0.0);
        self.pre_delay_index = 0;
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let shape = [self.rate, self.character.index() as u32];
        if self.dirty || shape != self.shape {
            self.shape = shape;
            self.dirty = false;
            self.tune();
        }
        let feedback = 0.7 + 0.28 * (self.room_size.value() / 100.0).clamp(0.0, 1.0);
        let damping = (self.damping.value() / 100.0).clamp(0.0, 0.99) * 0.4;
        let diffusion = 0.2 + 0.5 * (self.diffusion.value() / 100.0).clamp(0.0, 1.0);
        self.tune_tone(self.bass_cut.value(), self.treble_cut.value());
        let width = (self.width.value() / 100.0).clamp(0.0, 1.0);
        let wet = math::db_to_gain(self.wet.value());
        let dry = math::db_to_gain(self.dry.value());
        let tank_mix = (self.tank_mix.value() / 100.0).clamp(0.0, 1.0);
        let pre_delay = ((self.pre_delay.value() / 1000.0) * self.rate.max(1) as f32)
            .clamp(0.0, MAX_PRE_DELAY as f32 - 1.0) as usize;
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();

        for channel in 0..2 {
            for comb in self.combs[channel].iter_mut() {
                comb.feedback = feedback;
                comb.damping = damping;
            }
            for allpass in self.allpasses[channel].iter_mut() {
                allpass.feedback = diffusion;
            }
        }

        for frame in 0..frames {
            let mut wet_signal = [0.0f32; 2];
            for (channel, slot) in wet_signal.iter_mut().enumerate().take(channels.min(2)) {
                let input = buffer.channel_mut(channel)[frame];
                let delayed = if pre_delay > 0 {
                    let base = channel * MAX_PRE_DELAY;
                    let read =
                        base + (self.pre_delay_index + MAX_PRE_DELAY - pre_delay) % MAX_PRE_DELAY;
                    let value = self.pre_delay_buffer[read];
                    self.pre_delay_buffer[base + self.pre_delay_index] = input;
                    value
                } else {
                    input
                };
                let mut sum = 0.0f32;
                for comb in self.combs[channel].iter_mut() {
                    sum += comb.process(delayed * 0.015);
                }
                let comb_sum = sum;
                for allpass in self.allpasses[channel].iter_mut() {
                    sum = allpass.process(sum);
                }
                sum = comb_sum * (1.0 - tank_mix) + sum * tank_mix;
                for filter in self.tone[channel].iter_mut() {
                    sum = filter.process(sum);
                }
                *slot = sum;
            }
            if pre_delay > 0 {
                self.pre_delay_index = (self.pre_delay_index + 1) % MAX_PRE_DELAY;
            }
            for (channel, local) in wet_signal.iter().enumerate().take(channels.min(2)) {
                let input = buffer.channel_mut(channel)[frame];
                let mixed = *local * width + wet_signal[1 - channel] * (1.0 - width);
                buffer.channel_mut(channel)[frame] = math::sanitize(input * dry + mixed * wet);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
