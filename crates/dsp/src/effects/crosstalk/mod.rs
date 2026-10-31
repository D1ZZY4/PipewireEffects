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
        ParamSpec::float("input_gain", "Input gain", ParamUnit::Decibel)
            .range(-36.0, 36.0, 0.0)
            .step(0.1)
            .group("Input")
            .build(),
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
        ParamSpec::float("output_gain", "Output gain", ParamUnit::Decibel)
            .range(-36.0, 36.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
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
    input_gain: Knob,
    delay: Knob,
    decay: Knob,
    phantom_center: Knob,
    output_gain: Knob,
    left: CorrectionLine,
    right: CorrectionLine,
    rate: f32,
    signature: (f32, f32),
}

impl Crosstalk {
    pub fn new(params: ParamSet) -> Self {
        Self {
            input_gain: Knob::new(&params, "input_gain", 0.0),
            delay: Knob::new(&params, "delay", 313.0),
            decay: Knob::new(&params, "decay", -2.0),
            phantom_center: Knob::new(&params, "phantom_center", 0.0),
            output_gain: Knob::new(&params, "output_gain", 0.0),
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
        let input_gain = math::db_to_gain(self.input_gain.value());
        let output_gain = math::db_to_gain(self.output_gain.value());
        let decay = math::db_to_gain(self.decay.value());
        let phantom = self.phantom_center.flag();
        let frames = buffer.frames();

        for frame in 0..frames {
            let input_left = buffer.channel(0)[frame] * input_gain;
            let input_right = buffer.channel(1)[frame] * input_gain;
            if phantom {
                let middle = input_left + input_right;
                let side = input_left - input_right;
                let corrected = middle - decay * self.left.read();
                buffer.channel_mut(0)[frame] =
                    math::sanitize(0.5 * (corrected + side) * output_gain);
                buffer.channel_mut(1)[frame] =
                    math::sanitize(0.5 * (corrected - side) * output_gain);
                self.left.write(corrected);
            } else {
                let left_out = input_left - decay * self.right.read();
                let right_out = input_right - decay * self.left.read();
                buffer.channel_mut(0)[frame] = math::sanitize(left_out * output_gain);
                buffer.channel_mut(1)[frame] = math::sanitize(right_out * output_gain);
                self.left.write(left_out);
                self.right.write(right_out);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
