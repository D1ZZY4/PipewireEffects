use pwe_core::i18n::mark;
use pwe_core::{AudioBuffer, AudioFormat, Effect, ParamSet, ParamSpec, ParamUnit, math};

use crate::effects::dynamics::{Detector, blend, soft_knee_reduction_db};
use crate::effects::util::MAX_CHANNELS;
use crate::knob::Knob;

const DETECTORS: [&str; 2] = [mark("Peak"), mark("RMS")];

pub fn specs() -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("threshold", "Threshold", ParamUnit::Decibel)
            .range(-60.0, 0.0, -18.0)
            .step(0.5)
            .group("Detector")
            .build(),
        ParamSpec::choice("detector", "Detector", &DETECTORS, 0).group("Detector"),
        ParamSpec::float("ratio", "Ratio", ParamUnit::Ratio)
            .range(1.0, 20.0, 4.0)
            .step(0.1)
            .group("Detector")
            .build(),
        ParamSpec::float("knee", "Knee", ParamUnit::Decibel)
            .range(0.0, 24.0, 3.0)
            .step(0.5)
            .group("Detector")
            .build(),
        ParamSpec::float("attack", "Attack", ParamUnit::Milliseconds)
            .range(0.1, 200.0, 20.0)
            .logarithmic()
            .group("Timing")
            .build(),
        ParamSpec::float("release", "Release", ParamUnit::Milliseconds)
            .range(5.0, 2000.0, 200.0)
            .logarithmic()
            .group("Timing")
            .build(),
        ParamSpec::float("makeup", "Makeup", ParamUnit::Decibel)
            .range(-12.0, 24.0, 0.0)
            .step(0.1)
            .group("Output")
            .build(),
        ParamSpec::float("mix", "Mix", ParamUnit::Percent)
            .range(0.0, 100.0, 100.0)
            .step(1.0)
            .group("Output")
            .build(),
    ]
}

pub struct Compressor {
    threshold: Knob,
    detector: Knob,
    ratio: Knob,
    knee: Knob,
    attack: Knob,
    release: Knob,
    makeup: Knob,
    mix: Knob,
    state: Detector,
    rate: u32,
}

impl Compressor {
    pub fn new(params: ParamSet) -> Self {
        Self {
            threshold: Knob::new(&params, "threshold", -18.0),
            detector: Knob::new(&params, "detector", 0.0),
            ratio: Knob::new(&params, "ratio", 4.0),
            knee: Knob::new(&params, "knee", 3.0),
            attack: Knob::new(&params, "attack", 20.0),
            release: Knob::new(&params, "release", 200.0),
            makeup: Knob::new(&params, "makeup", 0.0),
            mix: Knob::new(&params, "mix", 100.0),
            state: Detector::default(),
            rate: 48_000,
        }
    }
}

impl Effect for Compressor {
    fn id(&self) -> &'static str {
        "compressor"
    }

    fn prepare(&mut self, format: AudioFormat) {
        self.rate = format.rate();
        self.state.reset();
    }

    fn reset(&mut self) {
        self.state.reset();
    }

    fn process(&mut self, buffer: &mut AudioBuffer) {
        self.state
            .set_ballistics(self.attack.value(), self.release.value(), self.rate);
        let threshold = self.threshold.value();
        let ratio = self.ratio.value();
        let knee = self.knee.value();
        let makeup = math::db_to_gain(self.makeup.value());
        let mix = (self.mix.value() / 100.0).clamp(0.0, 1.0);
        let use_rms = self.detector.index() == 1;
        let channels = buffer.channels().min(MAX_CHANNELS);
        let frames = buffer.frames();

        for frame in 0..frames {
            let level = self.state.track(buffer, channels, frame, use_rms);
            let reduction = soft_knee_reduction_db(math::gain_to_db(level), threshold, knee, ratio);
            let gain = math::db_to_gain(reduction) * makeup;
            for channel in 0..channels {
                let plan = buffer.channel_mut(channel);
                let dry = plan[frame];
                plan[frame] = math::sanitize(blend(dry, dry * gain, mix));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(amplitude: f32, frames: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(2, frames);
        buffer.configure(2, frames).unwrap();
        for channel in 0..2 {
            let plan = buffer.channel_mut(channel);
            for (index, sample) in plan.iter_mut().enumerate() {
                *sample =
                    amplitude * (std::f32::consts::TAU * 400.0 * index as f32 / 48_000.0).sin();
            }
        }
        buffer
    }

    #[test]
    fn signal_below_the_threshold_is_untouched() {
        let params = ParamSet::new(specs());
        let mut effect = Compressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0.05, 4096);
        for _ in 0..16 {
            effect.process(&mut buffer);
        }
        assert!(buffer.peak() <= 0.0501, "{}", buffer.peak());
    }

    #[test]
    fn signal_above_the_threshold_is_reduced() {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -24.0).unwrap();
        params.set_f32("ratio", 8.0).unwrap();
        params.set_f32("knee", 0.0).unwrap();
        params.set_f32("attack", 1.0).unwrap();
        let mut effect = Compressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0.9, 1024);
        for _ in 0..256 {
            buffer = tone(0.9, 1024);
            effect.process(&mut buffer);
        }
        let output_db = math::gain_to_db(buffer.peak());
        assert!(output_db < -18.0, "not enough reduction: {output_db}");
        assert!(output_db > -24.0, "too much reduction: {output_db}");
    }

    #[test]
    fn a_higher_ratio_compresses_harder() {
        let measure = |ratio: f32| {
            let params = ParamSet::new(specs());
            params.set_f32("threshold", -30.0).unwrap();
            params.set_f32("ratio", ratio).unwrap();
            params.set_f32("knee", 0.0).unwrap();
            params.set_f32("attack", 1.0).unwrap();
            let mut effect = Compressor::new(params);
            effect.prepare(AudioFormat::new(48_000, 2));
            let mut buffer = tone(0.9, 1024);
            for _ in 0..256 {
                buffer = tone(0.9, 1024);
                effect.process(&mut buffer);
            }
            buffer.peak()
        };
        let gentle = measure(2.0);
        let aggressive = measure(16.0);
        assert!(aggressive < gentle, "{aggressive} !< {gentle}");
    }

    #[test]
    fn makeup_gain_is_added_back() {
        let params = ParamSet::new(specs());
        params.set_f32("makeup", 6.0).unwrap();
        let mut effect = Compressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0.1, 4096);
        effect.process(&mut buffer);
        assert!((buffer.peak() - 0.1 * math::db_to_gain(6.0)).abs() < 0.01);
    }

    #[test]
    fn half_mix_blends_dry_and_wet() {
        let params = ParamSet::new(specs());
        params.set_f32("threshold", -24.0).unwrap();
        params.set_f32("mix", 0.0).unwrap();
        let mut effect = Compressor::new(params);
        effect.prepare(AudioFormat::new(48_000, 2));
        let mut buffer = tone(0.9, 4096);
        effect.process(&mut buffer);
        assert!((buffer.peak() - 0.9).abs() < 0.01, "{}", buffer.peak());
    }
}
