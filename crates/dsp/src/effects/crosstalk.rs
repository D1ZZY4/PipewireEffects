use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::biquad::{Biquad, Coefficients, FilterKind};
use crate::knob::Knob;

const CAPACITY: usize = 512;

const CORRECTION: [(FilterKind, f32, f32, f32); 6] = [
    (FilterKind::HighPass, 250.0, 0.710, 0.0),
    (FilterKind::LowPass, 3_245.0, 0.710, 0.0),
    (FilterKind::Peaking, 688.0, 1.000, -2.9),
    (FilterKind::Peaking, 1_066.0, 3.352, -6.4),
    (FilterKind::Peaking, 2_190.0, 2.037, -5.6),
    (FilterKind::Peaking, 3_792.0, 3.232, -4.5),
];

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("delay", "Delay", ParamUnit::Microseconds)
            .range(200.0, 500.0, 313.0)
            .step(1.0)
            .group("Crosstalk")
            .build(),
        ParamSpec::float("decay", "Decay", ParamUnit::Decibel)
            .range(-6.0, 0.0, -2.0)
            .step(0.1)
            .group("Crosstalk")
            .build(),
        ParamSpec::boolean("phantom_center", "Phantom centre only", false).group("Crosstalk"),
    ]
}

struct CorrectionLine {
    data: [f32; CAPACITY],
    index: usize,
    active: usize,
    stages: [Biquad; 6],
}

impl CorrectionLine {
    fn new() -> Self {
        Self {
            data: [0.0; CAPACITY],
            index: 0,
            active: 0,
            stages: [Biquad::default(); 6],
        }
    }

    fn configure(&mut self, rate: f32, delay_us: f32) {
        let samples = (delay_us * 1.0e-6 * rate).round();
        self.active = if samples.is_finite() && samples >= 1.0 {
            (samples as usize).min(CAPACITY)
        } else {
            0
        };
        self.index = 0;
        for (stage, (kind, frequency, q, gain)) in self.stages.iter_mut().zip(CORRECTION) {
            *stage = Biquad::new(Coefficients::design(kind, rate, frequency, q, gain));
        }
    }

    fn reset(&mut self) {
        self.data = [0.0; CAPACITY];
        self.index = 0;
        for stage in &mut self.stages {
            stage.reset();
        }
    }

    fn read(&self) -> f32 {
        if self.active == 0 {
            0.0
        } else {
            self.data[self.index]
        }
    }

    fn write(&mut self, sample: f32) {
        let mut value = sample;
        for stage in &mut self.stages {
            value = stage.process(value);
        }
        if self.active == 0 {
            return;
        }
        self.data[self.index] = value;
        self.index += 1;
        if self.index >= self.active {
            self.index = 0;
        }
    }
}

pub struct Crosstalk {
    delay: Knob,
    decay: Knob,
    phantom_center: Knob,
    left: CorrectionLine,
    right: CorrectionLine,
    rate: f32,
    signature: (f32, f32),
}

impl Crosstalk {
    pub fn new(params: ParamSet) -> Self {
        Self {
            delay: Knob::new(&params, "delay", 313.0),
            decay: Knob::new(&params, "decay", -2.0),
            phantom_center: Knob::new(&params, "phantom_center", 0.0),
            left: CorrectionLine::new(),
            right: CorrectionLine::new(),
            rate: 48_000.0,
            signature: (f32::NAN, f32::NAN),
        }
    }

    fn refresh(&mut self, delay_us: f32) {
        if self.signature == (self.rate, delay_us) {
            return;
        }
        self.signature = (self.rate, delay_us);
        self.left.configure(self.rate, delay_us);
        self.right.configure(self.rate, delay_us);
    }
}

impl Effect for Crosstalk {
    fn id(&self) -> &'static str {
        "crosstalk"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate().max(1) as f32;
        self.signature = (f32::NAN, f32::NAN);
        self.reset();
    }

    fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        let channels = buffer.channels();
        if channels < 2 {
            return;
        }
        self.refresh(self.delay.value());
        let decay = math::db_to_gain(self.decay.value());
        let phantom = self.phantom_center.flag();
        let frames = buffer.frames();

        for frame in 0..frames {
            let input_left = buffer.channel(0)[frame];
            let input_right = buffer.channel(1)[frame];
            if phantom {
                let middle = input_left + input_right;
                let side = input_left - input_right;
                let corrected = middle - decay * self.left.read();
                buffer.channel_mut(0)[frame] = math::sanitize(0.5 * (corrected + side));
                buffer.channel_mut(1)[frame] = math::sanitize(0.5 * (corrected - side));
                self.left.write(corrected);
            } else {
                let left_out = input_left - decay * self.right.read();
                let right_out = input_right - decay * self.left.read();
                buffer.channel_mut(0)[frame] = math::sanitize(left_out);
                buffer.channel_mut(1)[frame] = math::sanitize(right_out);
                self.left.write(left_out);
                self.right.write(right_out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(delay: f32, decay: f32, phantom: f32) -> Crosstalk {
        let params = ParamSet::new(specs());
        params.set_f32("delay", delay).unwrap();
        params.set_f32("decay", decay).unwrap();
        params.set_f32("phantom_center", phantom).unwrap();
        let mut effect = Crosstalk::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        effect
    }

    fn one_sided(amplitude: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for index in 0..frames {
            let value = amplitude * (std::f32::consts::TAU * 300.0 * index as f32 / 48_000.0).sin();
            buffer.channel_mut(1)[index] = value;
        }
        buffer
    }

    #[test]
    fn a_mono_signal_keeps_both_channels_equal() {
        let mut effect = effect(313.0, -2.0, 0.0);
        let mut buffer = one_sided(0.4, 8_192);
        for index in 0..buffer.frames() {
            let value = buffer.channel(1)[index];
            buffer.channel_mut(0)[index] = value;
        }
        effect.process(&mut buffer);
        for index in 0..buffer.frames() {
            let left = buffer.channel(0)[index];
            let right = buffer.channel(1)[index];
            assert!((left - right).abs() < 1.0e-5, "{left} {right} at {index}");
        }
    }

    #[test]
    fn silence_stays_silent() {
        let mut effect = effect(313.0, 0.0, 0.0);
        let mut buffer = AudioBuffer::new(2, 4_096);
        buffer.configure(2, 4_096).unwrap();
        effect.process(&mut buffer);
        assert_eq!(buffer.peak(), 0.0);
    }

    #[test]
    fn a_signal_on_one_side_leaks_a_delayed_copy_to_the_other() {
        let mut effect = effect(313.0, 0.0, 0.0);
        let mut buffer = one_sided(0.5, 9_600);
        effect.process(&mut buffer);
        let leaked = buffer
            .channel(0)
            .iter()
            .fold(0.0f32, |peak, value| peak.max(value.abs()));
        assert!(leaked > 0.02, "no crossfeed appeared: {leaked}");
    }

    #[test]
    fn the_leak_is_the_configured_delay_late() {
        let mut effect = effect(400.0, 0.0, 0.0);
        let mut buffer = AudioBuffer::new(2, 2048);
        buffer.configure(2, 2048).unwrap();
        buffer.channel_mut(1)[0] = 1.0;
        effect.process(&mut buffer);
        let expected = (400.0e-6f32 * 48_000.0).round() as usize;
        let peak_index = buffer
            .channel(0)
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .map(|(index, _)| index)
            .unwrap();
        assert!(
            peak_index.abs_diff(expected) < 4,
            "{peak_index} against {expected}"
        );
    }

    #[test]
    fn the_phantom_centre_mode_keeps_a_difference_signal_intact() {
        let mut effect = effect(313.0, -2.0, 1.0);
        let mut buffer = one_sided(0.5, 4096);
        effect.process(&mut buffer);
        for sample in buffer.channel(0) {
            assert!(sample.is_finite() && sample.abs() < 2.0, "{sample}");
        }
    }

    #[test]
    fn extreme_settings_stay_bounded() {
        let mut effect = effect(500.0, 0.0, 0.0);
        let mut buffer = one_sided(0.9, 8192);
        effect.process(&mut buffer);
        for channel in 0..2 {
            for sample in buffer.channel(channel) {
                assert!(sample.is_finite() && sample.abs() < 4.0, "{sample}");
            }
        }
    }
}
